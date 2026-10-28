//! Minimal MP4/QuickTime structure reader: movie header, tracks with sample tables and user data.
//!
//! Only box headers and the small index boxes are read; media data (`mdat`) is never loaded.

use std::io::{self, Read, Seek, SeekFrom};

pub type FourCC = [u8; 4];

/// Index boxes larger than this are treated as corrupt.
const MAX_INDEX_BOX: u64 = 64 << 20;
/// User data entries larger than this are not kept.
const MAX_USER_DATA: u64 = 4096;

#[derive(Debug, Clone, Default)]
pub struct Movie {
    pub brand: Option<FourCC>,
    pub timescale: u32,
    pub duration: u64,
    /// Seconds since 1904-01-01 (QuickTime epoch); 0 when unset.
    pub creation_time: u64,
    pub tracks: Vec<Track>,
    /// Children of `moov/udta` (camera identification and similar), small entries only.
    pub user_data: Vec<(FourCC, Vec<u8>)>,
}

#[derive(Debug, Clone, Default)]
pub struct Track {
    pub id: u32,
    pub handler: FourCC,
    pub codec: FourCC,
    pub timescale: u32,
    pub duration: u64,
    pub width: f32,
    pub height: f32,
    /// Decoder configuration record of a video sample entry (`avcC` / `hvcC` body).
    pub codec_config: Option<Vec<u8>>,
    /// `stss` sync (key) sample numbers, 1-based; `None` when every sample is a sync sample.
    sync_samples: Option<Vec<u32>>,
    sample_sizes: Vec<u32>,
    chunk_offsets: Vec<u64>,
    /// `stsc` runs: (first chunk, 1-based; samples per chunk).
    sample_to_chunk: Vec<(u32, u32)>,
    /// `stts` runs: (sample count, sample delta).
    time_to_sample: Vec<(u32, u32)>,
    /// `ctts` runs: (sample count, composition offset).
    composition_offsets: Vec<(u32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub offset: u64,
    pub size: u32,
    /// Presentation time in seconds from the start of the track (decode time + `ctts` offset).
    pub time: f64,
    /// Sync (key) sample: decoding can start here.
    pub sync: bool,
}

impl Movie {
    pub fn duration_secs(&self) -> f64 {
        if self.timescale == 0 {
            0.0
        } else {
            self.duration as f64 / self.timescale as f64
        }
    }

    pub fn track_by_handler(&self, handler: &FourCC) -> Option<&Track> {
        self.tracks.iter().find(|t| &t.handler == handler)
    }

    pub fn track_by_codec(&self, codec: &FourCC) -> Option<&Track> {
        self.tracks.iter().find(|t| &t.codec == codec)
    }

    /// Camera identification from QuickTime user data (`©mod`, `©fmt`, `©inf`, `©mak`).
    pub fn camera_model(&self) -> Option<String> {
        let keys: [&[u8]; 4] = [b"\xa9mod", b"\xa9fmt", b"\xa9inf", b"\xa9mak"];
        keys.iter()
            .filter_map(|k| self.user_data.iter().find(|(t, _)| t == *k))
            .map(|(_, body)| printable(body))
            .find(|s| !s.is_empty())
    }
}

impl Track {
    pub fn duration_secs(&self) -> f64 {
        if self.timescale == 0 {
            0.0
        } else {
            self.duration as f64 / self.timescale as f64
        }
    }

    pub fn sample_count(&self) -> usize {
        self.sample_sizes.len()
    }

    /// All samples in decoding order with file offsets and times.
    pub fn samples(&self) -> Vec<Sample> {
        let mut out = Vec::with_capacity(self.sample_sizes.len());
        let mut deltas = self
            .time_to_sample
            .iter()
            .flat_map(|&(n, d)| std::iter::repeat_n(d, n as usize));
        let mut cts = self
            .composition_offsets
            .iter()
            .flat_map(|&(n, o)| std::iter::repeat_n(o, n as usize));
        let mut ticks = 0u64;
        let mut index = 0usize;
        for (chunk, &chunk_offset) in self.chunk_offsets.iter().enumerate() {
            let chunk_no = chunk as u32 + 1;
            let per_chunk = self
                .sample_to_chunk
                .iter()
                .rev()
                .find(|&&(first, _)| first <= chunk_no)
                .map_or(0, |&(_, n)| n);
            let mut offset = chunk_offset;
            for _ in 0..per_chunk {
                let Some(&size) = self.sample_sizes.get(index) else {
                    return out;
                };
                let presentation = ticks as i64 + i64::from(cts.next().unwrap_or(0));
                let time = if self.timescale == 0 {
                    0.0
                } else {
                    presentation as f64 / self.timescale as f64
                };
                let sync = self
                    .sync_samples
                    .as_ref()
                    .is_none_or(|s| s.binary_search(&(index as u32 + 1)).is_ok());
                out.push(Sample {
                    offset,
                    size,
                    time,
                    sync,
                });
                offset += u64::from(size);
                ticks += u64::from(deltas.next().unwrap_or(0));
                index += 1;
            }
        }
        out
    }
}

pub fn read_movie<R: Read + Seek>(r: &mut R) -> io::Result<Movie> {
    let end = r.seek(SeekFrom::End(0))?;
    let mut movie = Movie::default();
    let mut found_moov = false;
    for b in boxes(r, 0, end)? {
        match &b.kind {
            b"ftyp" => {
                movie.brand = read_body(r, &b, 64)?
                    .get(..4)
                    .map(|s| s.try_into().unwrap())
            }
            b"moov" => {
                found_moov = true;
                read_moov(r, &b, &mut movie)?;
            }
            _ => {}
        }
    }
    if !found_moov {
        return Err(invalid("no moov box (file truncated or not MP4/MOV)"));
    }
    Ok(movie)
}

struct BoxRef {
    kind: FourCC,
    body: u64,
    end: u64,
}

impl BoxRef {
    fn body_len(&self) -> u64 {
        self.end - self.body
    }
}

fn boxes<R: Read + Seek>(r: &mut R, start: u64, end: u64) -> io::Result<Vec<BoxRef>> {
    let mut out = Vec::new();
    let mut pos = start;
    while pos + 8 <= end {
        r.seek(SeekFrom::Start(pos))?;
        let mut h = [0u8; 8];
        r.read_exact(&mut h)?;
        let mut size = u64::from(u32::from_be_bytes(h[..4].try_into().unwrap()));
        let kind: FourCC = h[4..].try_into().unwrap();
        let mut header = 8;
        if size == 1 {
            let mut ext = [0u8; 8];
            r.read_exact(&mut ext)?;
            size = u64::from_be_bytes(ext);
            header = 16;
        } else if size == 0 {
            size = end - pos;
        }
        if size < header || pos + size > end {
            break; // truncated tail: keep what was read so far
        }
        out.push(BoxRef {
            kind,
            body: pos + header,
            end: pos + size,
        });
        pos += size;
    }
    Ok(out)
}

fn read_body<R: Read + Seek>(r: &mut R, b: &BoxRef, limit: u64) -> io::Result<Vec<u8>> {
    if b.body_len() > limit {
        return Err(invalid("index box too large"));
    }
    r.seek(SeekFrom::Start(b.body))?;
    let mut buf = vec![0u8; b.body_len() as usize];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_moov<R: Read + Seek>(r: &mut R, moov: &BoxRef, movie: &mut Movie) -> io::Result<()> {
    for b in boxes(r, moov.body, moov.end)? {
        match &b.kind {
            b"mvhd" => {
                let d = read_body(r, &b, 1024)?;
                let mut c = Cursor::new(&d);
                let version = c.u8()?;
                c.skip(3)?;
                if version == 1 {
                    movie.creation_time = c.u64()?;
                    c.skip(8)?;
                    movie.timescale = c.u32()?;
                    movie.duration = c.u64()?;
                } else {
                    movie.creation_time = u64::from(c.u32()?);
                    c.skip(4)?;
                    movie.timescale = c.u32()?;
                    movie.duration = u64::from(c.u32()?);
                }
            }
            b"trak" => {
                let mut track = Track::default();
                read_trak(r, &b, &mut track)?;
                movie.tracks.push(track);
            }
            b"udta" => {
                for u in boxes(r, b.body, b.end)? {
                    if u.body_len() <= MAX_USER_DATA {
                        let body = read_body(r, &u, MAX_USER_DATA)?;
                        movie.user_data.push((u.kind, body));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn read_trak<R: Read + Seek>(r: &mut R, parent: &BoxRef, t: &mut Track) -> io::Result<()> {
    for b in boxes(r, parent.body, parent.end)? {
        match &b.kind {
            b"mdia" | b"minf" | b"stbl" => read_trak(r, &b, t)?,
            b"tkhd" => {
                let d = read_body(r, &b, 1024)?;
                let version = d.first().copied().unwrap_or(0);
                let (id_at, size_at) = if version == 1 { (20, 88) } else { (12, 76) };
                let mut c = Cursor::new(&d);
                c.skip(id_at)?;
                t.id = c.u32()?;
                let mut c = Cursor::new(&d);
                c.skip(size_at)?;
                t.width = c.u32()? as f32 / 65536.0;
                t.height = c.u32()? as f32 / 65536.0;
            }
            b"mdhd" => {
                let d = read_body(r, &b, 1024)?;
                let mut c = Cursor::new(&d);
                let version = c.u8()?;
                c.skip(3)?;
                if version == 1 {
                    c.skip(16)?;
                    t.timescale = c.u32()?;
                    t.duration = c.u64()?;
                } else {
                    c.skip(8)?;
                    t.timescale = c.u32()?;
                    t.duration = u64::from(c.u32()?);
                }
            }
            // The media handler comes first; `minf` may contain a second (data reference) handler.
            b"hdlr" if t.handler == [0; 4] => {
                let d = read_body(r, &b, 1024)?;
                t.handler = d.get(8..12).map_or([0; 4], |s| s.try_into().unwrap());
            }
            b"stsd" => {
                let d = read_body(r, &b, MAX_INDEX_BOX)?;
                t.codec = d.get(12..16).map_or([0; 4], |s| s.try_into().unwrap());
                t.codec_config = video_codec_config(&d);
            }
            b"stss" => {
                let d = read_body(r, &b, MAX_INDEX_BOX)?;
                t.sync_samples = Some(table(&d, |c| c.u32())?);
            }
            b"ctts" => {
                let d = read_body(r, &b, MAX_INDEX_BOX)?;
                // Version 0 offsets are unsigned but writers store negative values the same way.
                t.composition_offsets = table(&d, |c| Ok((c.u32()?, c.u32()? as i32)))?;
            }
            b"stts" => {
                let d = read_body(r, &b, MAX_INDEX_BOX)?;
                t.time_to_sample = table(&d, |c| Ok((c.u32()?, c.u32()?)))?;
            }
            b"stsc" => {
                let d = read_body(r, &b, MAX_INDEX_BOX)?;
                t.sample_to_chunk = table(&d, |c| {
                    let first = c.u32()?;
                    let per_chunk = c.u32()?;
                    c.skip(4)?; // sample description index
                    Ok((first, per_chunk))
                })?;
            }
            b"stsz" => {
                let d = read_body(r, &b, MAX_INDEX_BOX)?;
                let mut c = Cursor::new(&d);
                c.skip(4)?;
                let fixed = c.u32()?;
                let count = c.u32()? as usize;
                t.sample_sizes = if fixed != 0 {
                    vec![fixed; count]
                } else {
                    (0..count).map(|_| c.u32()).collect::<io::Result<_>>()?
                };
            }
            b"stco" => {
                let d = read_body(r, &b, MAX_INDEX_BOX)?;
                t.chunk_offsets = table(&d, |c| Ok(u64::from(c.u32()?)))?;
            }
            b"co64" => {
                let d = read_body(r, &b, MAX_INDEX_BOX)?;
                t.chunk_offsets = table(&d, |c| c.u64())?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// `avcC` / `hvcC` body from the first entry of an `stsd` body, for H.264 / H.265 sample entries.
fn video_codec_config(stsd: &[u8]) -> Option<Vec<u8>> {
    // stsd: version/flags(4) count(4), entry: size(4) type(4) SampleEntry(8) VisualSampleEntry(70).
    const ENTRY: usize = 8;
    const CHILDREN: usize = ENTRY + 8 + 78;
    let entry_size = u32::from_be_bytes(stsd.get(ENTRY..ENTRY + 4)?.try_into().ok()?) as usize;
    let wanted: &[u8] = match stsd.get(ENTRY + 4..ENTRY + 8)? {
        b"avc1" | b"avc3" => b"avcC",
        b"hvc1" | b"hev1" => b"hvcC",
        _ => return None,
    };
    let end = (ENTRY + entry_size).min(stsd.len());
    let mut pos = CHILDREN;
    while pos + 8 <= end {
        let size = u32::from_be_bytes(stsd[pos..pos + 4].try_into().ok()?) as usize;
        if size < 8 || pos + size > end {
            return None;
        }
        if &stsd[pos + 4..pos + 8] == wanted {
            return Some(stsd[pos + 8..pos + size].to_vec());
        }
        pos += size;
    }
    None
}

/// Full-box table: version/flags, entry count, entries.
fn table<T>(d: &[u8], mut entry: impl FnMut(&mut Cursor) -> io::Result<T>) -> io::Result<Vec<T>> {
    let mut c = Cursor::new(d);
    c.skip(4)?;
    let count = c.u32()? as usize;
    let mut out = Vec::with_capacity(count.min(d.len() / 4));
    for _ in 0..count {
        out.push(entry(&mut c)?);
    }
    Ok(out)
}

/// Big-endian reader over a byte slice.
pub(crate) struct Cursor<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(d: &'a [u8]) -> Self {
        Self { d, pos: 0 }
    }

    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let s = self
            .d
            .get(self.pos..self.pos + n)
            .ok_or_else(|| invalid("box shorter than expected"))?;
        self.pos += n;
        Ok(s)
    }

    pub(crate) fn skip(&mut self, n: usize) -> io::Result<()> {
        self.take(n).map(|_| ())
    }

    pub(crate) fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(crate) fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Longest printable ASCII run in a user-data payload (skips QuickTime string headers and padding).
fn printable(body: &[u8]) -> String {
    body.split(|c| !(0x20..0x7f).contains(c))
        .max_by_key(|s| s.len())
        .map(|s| String::from_utf8_lossy(s).trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a box: size + type + body.
    pub(crate) fn mk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn be(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    /// A one-track movie whose samples (given as payloads) live in an `mdat` after `moov`.
    pub(crate) fn movie_with_track(
        handler: &[u8; 4],
        codec: &[u8; 4],
        timescale: u32,
        payloads: &[Vec<u8>],
    ) -> Vec<u8> {
        let build = |mdat_body_offset: u32| {
            let mut stsd = be(&[0, 1, 16]);
            stsd.extend_from_slice(codec);
            stsd.extend_from_slice(&[0; 8]);
            let stbl = [
                mk(b"stsd", &stsd),
                mk(b"stts", &be(&[0, 1, payloads.len() as u32, timescale])),
                mk(b"stsc", &be(&[0, 1, 1, payloads.len() as u32, 1])),
                mk(
                    b"stsz",
                    &[
                        be(&[0, 0, payloads.len() as u32]),
                        be(&payloads.iter().map(|p| p.len() as u32).collect::<Vec<_>>()),
                    ]
                    .concat(),
                ),
                mk(b"stco", &be(&[0, 1, mdat_body_offset])),
            ]
            .concat();
            let mut hdlr = be(&[0, 0]);
            hdlr.extend_from_slice(handler);
            hdlr.extend_from_slice(&[0; 12]);
            let mdhd = be(&[0, 0, 0, timescale, timescale * payloads.len() as u32, 0]);
            let mdia = mk(
                b"mdia",
                &[
                    mk(b"mdhd", &mdhd),
                    mk(b"hdlr", &hdlr),
                    mk(b"minf", &mk(b"stbl", &stbl)),
                ]
                .concat(),
            );
            let mut tkhd = vec![0u8; 84];
            tkhd[12..16].copy_from_slice(&1u32.to_be_bytes());
            tkhd[76..80].copy_from_slice(&(1920u32 << 16).to_be_bytes());
            tkhd[80..84].copy_from_slice(&(1080u32 << 16).to_be_bytes());
            let trak = mk(b"trak", &[mk(b"tkhd", &tkhd), mdia].concat());
            let mvhd = [
                be(&[0, 3_000_000_000, 0, 1000, 1000 * payloads.len() as u32]),
                vec![0; 80],
            ]
            .concat();
            let udta = mk(b"udta", &mk(b"\xa9inf", b"AMBARELLA A7L\0\0"));
            let moov = mk(b"moov", &[mk(b"mvhd", &mvhd), udta, trak].concat());
            [mk(b"ftyp", b"qt  \0\0\0\0qt  "), moov].concat()
        };
        let head_len = build(0).len() as u32;
        let mut file = build(head_len + 8);
        file.extend(mk(b"mdat", &payloads.concat()));
        file
    }

    #[test]
    fn reads_movie_structure_and_samples() {
        let payloads = vec![b"aaaa".to_vec(), b"bbbbbb".to_vec(), b"c".to_vec()];
        let file = movie_with_track(b"data", b"RVMI", 50, &payloads);
        let movie = read_movie(&mut io::Cursor::new(&file)).unwrap();
        assert_eq!(movie.brand, Some(*b"qt  "));
        assert_eq!(movie.duration_secs(), 3.0);
        assert_eq!(movie.camera_model().as_deref(), Some("AMBARELLA A7L"));
        let t = movie.track_by_codec(b"RVMI").unwrap();
        assert_eq!((t.handler, t.width, t.height), (*b"data", 1920.0, 1080.0));
        let samples = t.samples();
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[2].time, 2.0);
        let s = samples[1];
        assert_eq!(
            &file[s.offset as usize..s.offset as usize + s.size as usize],
            b"bbbbbb"
        );
    }

    #[test]
    fn extracts_avcc_from_video_sample_entry() {
        let avcc = mk(b"avcC", &[1, 0x64, 0, 0x28, 0xff]);
        let mut entry = vec![0u8; 78];
        entry.extend_from_slice(&avcc);
        let entry = mk(b"avc1", &entry);
        let stsd = [be(&[0, 1]), entry].concat();
        assert_eq!(
            video_codec_config(&stsd),
            Some(vec![1, 0x64, 0, 0x28, 0xff])
        );
        let other = [be(&[0, 1]), mk(b"mp4a", &[0; 40])].concat();
        assert_eq!(video_codec_config(&other), None);
    }

    #[test]
    fn missing_moov_is_an_error() {
        let file = mk(b"mdat", &[0; 32]);
        assert!(read_movie(&mut io::Cursor::new(&file)).is_err());
    }
}
