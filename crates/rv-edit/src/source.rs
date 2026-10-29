//! Source files: box structure, track parameters and per-frame tables.
//!
//! Only the box headers, `ftyp` and `moov` are read; media data stays on disk and is copied
//! sample by sample when the output is written.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub(crate) type FourCC = [u8; 4];

/// `moov` boxes larger than this are treated as corrupt.
const MAX_MOOV: u64 = 256 << 20;

/// Track kinds the exporter copies (`formatType` of the original: 1 video, 2 audio).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Video,
    Audio,
    Other,
}

/// One sample of a source track. Times and durations are in track ticks, as doubles like the
/// original (`TFrameItem`), so speed changes and rescaling keep fractions until they are written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Frame {
    pub offset: u64,
    pub size: u32,
    /// Decode time.
    pub time: f64,
    pub dur: f64,
    /// Composition offset (`ctts`).
    pub ctts: i32,
    pub key: bool,
    /// `sdtp` flags byte (0 when the track has none).
    pub sdtp: u8,
    /// Picture type for key-frame-only speed-up: 0 unknown, 1 I, 2 P, 3 B.
    pub ftype: u8,
}

#[derive(Debug, Clone)]
pub(crate) struct SrcTrack {
    pub id: u32,
    pub kind: Kind,
    pub fourcc: FourCC,
    /// Video: width, height. Audio: channels, sample size, sample rate. Compared when joining
    /// (the 8 bytes at +0x44 of the original's track object).
    pub format: [u32; 3],
    /// Decoder specific info: `avcC` / `hvcC` body for video, the AAC AudioSpecificConfig for audio.
    pub dsi: Vec<u8>,
    pub scale: u32,
    /// `mdhd` duration in ticks.
    pub duration: u64,
    pub frames: Vec<Frame>,
}

impl SrcTrack {
    pub fn is_h264(&self) -> bool {
        matches!(&self.fourcc, b"avc1" | b"avc3")
    }

    pub fn is_hevc(&self) -> bool {
        matches!(&self.fourcc, b"hvc1" | b"hev1")
    }

    /// Track duration in milliseconds (`durationMS`).
    pub fn duration_ms(&self) -> i64 {
        if self.scale == 0 {
            0
        } else {
            round(self.duration as f64 * 1000.0 / f64::from(self.scale))
        }
    }

    /// Uncompressed PCM is stored as tiny fixed-size samples grouped in chunks; the frame model
    /// here (one packet per sample) does not apply to it.
    pub fn is_pcm(&self) -> bool {
        matches!(
            &self.fourcc,
            b"sowt"
                | b"twos"
                | b"raw "
                | b"lpcm"
                | b"in24"
                | b"in32"
                | b"fl32"
                | b"fl64"
                | b"ulaw"
                | b"alaw"
                | b"NONE"
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Source {
    pub path: PathBuf,
    /// `ftyp` body, if the file has one.
    pub ftyp: Option<Vec<u8>>,
    /// `moov` body (used as the template of the output header when this is the first source).
    pub moov: Vec<u8>,
    /// Container duration in milliseconds (`mvhd`); crop fractions refer to it.
    pub duration_ms: i64,
    pub tracks: Vec<SrcTrack>,
}

impl Source {
    pub fn open(path: &Path) -> io::Result<(Source, BufReader<File>)> {
        let mut r = BufReader::with_capacity(1 << 16, File::open(path)?);
        let end = r.seek(SeekFrom::End(0))?;
        let mut ftyp = None;
        let mut moov = None;
        let mut pos = 0u64;
        while pos + 8 <= end {
            r.seek(SeekFrom::Start(pos))?;
            let mut h = [0u8; 8];
            r.read_exact(&mut h)?;
            let mut size = u64::from(be32(&h, 0));
            let kind: FourCC = h[4..8].try_into().unwrap();
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
                break;
            }
            let body_len = size - header;
            match &kind {
                b"ftyp" if ftyp.is_none() && body_len <= 4096 => {
                    let mut b = vec![0u8; body_len as usize];
                    r.read_exact(&mut b)?;
                    ftyp = Some(b);
                }
                b"moov" if moov.is_none() => {
                    if body_len > MAX_MOOV {
                        return Err(invalid("moov box too large"));
                    }
                    let mut b = vec![0u8; body_len as usize];
                    r.read_exact(&mut b)?;
                    moov = Some(b);
                }
                _ => {}
            }
            pos += size;
        }
        let moov = moov.ok_or_else(|| invalid("no moov box"))?;
        let mut src = Source {
            path: path.to_path_buf(),
            ftyp,
            moov: Vec::new(),
            duration_ms: 0,
            tracks: Vec::new(),
        };
        parse_moov(&moov, &mut src)?;
        src.moov = moov;
        Ok((src, r))
    }

    /// First video track (`GetVideoTrack`).
    pub fn video_track(&self) -> Option<usize> {
        self.tracks.iter().position(|t| t.kind == Kind::Video)
    }
}

/// Delphi `Round`: to nearest, ties to even (the FPU default).
pub(crate) fn round(x: f64) -> i64 {
    x.round_ties_even() as i64
}

pub(crate) fn be16(d: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(d[at..at + 2].try_into().unwrap())
}

pub(crate) fn be32(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(d[at..at + 4].try_into().unwrap())
}

pub(crate) fn be64(d: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(d[at..at + 8].try_into().unwrap())
}

pub(crate) fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

/// A box inside an in-memory buffer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoxRef {
    pub kind: FourCC,
    pub body: usize,
    pub end: usize,
}

/// Iterates over the boxes in `d` (stops at the first malformed header).
pub(crate) fn boxes(d: &[u8]) -> impl Iterator<Item = BoxRef> + '_ {
    let mut pos = 0usize;
    std::iter::from_fn(move || {
        if pos + 8 > d.len() {
            return None;
        }
        let mut size = be32(d, pos) as usize;
        let kind: FourCC = d[pos + 4..pos + 8].try_into().unwrap();
        let mut header = 8;
        if size == 1 {
            if pos + 16 > d.len() {
                return None;
            }
            size = usize::try_from(be64(d, pos + 8)).ok()?;
            header = 16;
        } else if size == 0 {
            size = d.len() - pos;
        }
        if size < header || pos.checked_add(size)? > d.len() {
            return None;
        }
        let b = BoxRef {
            kind,
            body: pos + header,
            end: pos + size,
        };
        pos += size;
        Some(b)
    })
}

fn child<'a>(d: &'a [u8], kind: &FourCC) -> Option<&'a [u8]> {
    boxes(d)
        .find(|b| &b.kind == kind)
        .map(|b| &d[b.body..b.end])
}

fn parse_moov(moov: &[u8], src: &mut Source) -> io::Result<()> {
    for b in boxes(moov) {
        let body = &moov[b.body..b.end];
        match &b.kind {
            b"mvhd" if body.len() >= 20 => {
                let (scale, duration) = if body[0] == 1 {
                    if body.len() < 32 {
                        continue;
                    }
                    (be32(body, 20), be64(body, 24))
                } else {
                    (be32(body, 12), u64::from(be32(body, 16)))
                };
                if scale > 0 {
                    src.duration_ms = round(duration as f64 * 1000.0 / f64::from(scale));
                }
            }
            b"trak" => {
                if let Some(t) = parse_trak(body)? {
                    src.tracks.push(t);
                }
            }
            _ => {}
        }
    }
    if src.duration_ms == 0 {
        src.duration_ms = src
            .tracks
            .iter()
            .map(SrcTrack::duration_ms)
            .max()
            .unwrap_or(0);
    }
    Ok(())
}

#[derive(Default)]
struct Tables<'a> {
    stsd: Option<&'a [u8]>,
    stts: Option<&'a [u8]>,
    ctts: Option<&'a [u8]>,
    stss: Option<&'a [u8]>,
    stsz: Option<&'a [u8]>,
    stsc: Option<&'a [u8]>,
    stco: Option<&'a [u8]>,
    co64: Option<&'a [u8]>,
    sdtp: Option<&'a [u8]>,
}

fn parse_trak(trak: &[u8]) -> io::Result<Option<SrcTrack>> {
    let Some(tkhd) = child(trak, b"tkhd") else {
        return Ok(None);
    };
    let Some(mdia) = child(trak, b"mdia") else {
        return Ok(None);
    };
    let (id, tk_w, tk_h) = if tkhd.first() == Some(&1) && tkhd.len() >= 96 {
        (be32(tkhd, 20), be32(tkhd, 88) >> 16, be32(tkhd, 92) >> 16)
    } else if tkhd.len() >= 84 {
        (be32(tkhd, 12), be32(tkhd, 76) >> 16, be32(tkhd, 80) >> 16)
    } else {
        return Ok(None);
    };
    let Some(mdhd) = child(mdia, b"mdhd") else {
        return Ok(None);
    };
    let (scale, duration) = if mdhd.first() == Some(&1) && mdhd.len() >= 32 {
        (be32(mdhd, 20), be64(mdhd, 24))
    } else if mdhd.len() >= 20 {
        (be32(mdhd, 12), u64::from(be32(mdhd, 16)))
    } else {
        return Ok(None);
    };
    let handler: FourCC = child(mdia, b"hdlr")
        .and_then(|h| h.get(8..12))
        .map_or([0; 4], |s| s.try_into().unwrap());
    let kind = match &handler {
        b"vide" => Kind::Video,
        b"soun" => Kind::Audio,
        _ => Kind::Other,
    };
    let mut tables = Tables::default();
    if let Some(stbl) = child(mdia, b"minf").and_then(|m| child(m, b"stbl")) {
        for b in boxes(stbl) {
            let body = Some(&stbl[b.body..b.end]);
            match &b.kind {
                b"stsd" => tables.stsd = body,
                b"stts" => tables.stts = body,
                b"ctts" => tables.ctts = body,
                b"stss" => tables.stss = body,
                b"stsz" => tables.stsz = body,
                b"stsc" => tables.stsc = body,
                b"stco" => tables.stco = body,
                b"co64" => tables.co64 = body,
                b"sdtp" => tables.sdtp = body,
                _ => {}
            }
        }
    }
    let mut t = SrcTrack {
        id,
        kind,
        fourcc: [0; 4],
        format: [0; 3],
        dsi: Vec::new(),
        scale,
        duration,
        frames: Vec::new(),
    };
    if let Some(stsd) = tables.stsd {
        sample_description(stsd, &mut t);
    }
    if t.kind == Kind::Video && (t.format[0] == 0 || t.format[1] == 0) {
        t.format = [tk_w, tk_h, 0];
    }
    t.frames = frames(&tables)?;
    Ok(Some(t))
}

/// Codec, format parameters and decoder specific info from the first sample entry.
fn sample_description(stsd: &[u8], t: &mut SrcTrack) {
    // version/flags(4) count(4), entry: size(4) type(4) reserved(6) data_ref_index(2) ...
    const ENTRY: usize = 8;
    if stsd.len() < ENTRY + 8 {
        return;
    }
    let size = be32(stsd, ENTRY) as usize;
    let end = (ENTRY + size).min(stsd.len());
    let entry = &stsd[ENTRY..end];
    t.fourcc = entry[4..8].try_into().unwrap();
    match t.kind {
        Kind::Video if entry.len() >= 86 => {
            t.format = [u32::from(be16(entry, 32)), u32::from(be16(entry, 34)), 0];
            let wanted: &FourCC = if t.is_hevc() { b"hvcC" } else { b"avcC" };
            if let Some(c) = child(&entry[86..], wanted) {
                t.dsi = c.to_vec();
            }
        }
        Kind::Audio if entry.len() >= 36 => {
            t.format = [
                u32::from(be16(entry, 24)),
                u32::from(be16(entry, 26)),
                be32(entry, 32) >> 16,
            ];
            // QuickTime sound description versions 1 and 2 have 16 / 36 more bytes.
            let children = match be16(entry, 16) {
                1 => 52,
                2 => 72,
                _ => 36,
            };
            if let Some(esds) = entry.get(children..).and_then(find_esds) {
                t.dsi = audio_specific_config(esds).unwrap_or(esds).to_vec();
            }
        }
        _ => {}
    }
}

/// `esds` among the children of an audio sample entry, also inside a QuickTime `wave` box.
fn find_esds(d: &[u8]) -> Option<&[u8]> {
    for b in boxes(d) {
        match &b.kind {
            b"esds" => return Some(&d[b.body..b.end]),
            b"wave" => {
                if let Some(e) = find_esds(&d[b.body..b.end]) {
                    return Some(e);
                }
            }
            _ => {}
        }
    }
    None
}

/// DecoderSpecificInfo (tag 5) of an `esds` body: the AudioSpecificConfig for AAC.
fn audio_specific_config(esds: &[u8]) -> Option<&[u8]> {
    let mut d = esds.get(4..)?; // version/flags
    let (tag, body) = descriptor(&mut d)?;
    if tag != 3 {
        return None;
    }
    let mut es = body;
    let flags = *es.get(2)?;
    let mut skip = 3;
    if flags & 0x80 != 0 {
        skip += 2;
    }
    if flags & 0x40 != 0 {
        skip += 1 + usize::from(*es.get(skip)?);
    }
    if flags & 0x20 != 0 {
        skip += 2;
    }
    es = es.get(skip..)?;
    while !es.is_empty() {
        let (tag, body) = descriptor(&mut es)?;
        if tag == 4 {
            let mut dc = body.get(13..)?;
            while !dc.is_empty() {
                let (tag, body) = descriptor(&mut dc)?;
                if tag == 5 {
                    return Some(body);
                }
            }
        }
    }
    None
}

/// One MPEG-4 descriptor: tag, variable-length size, body. Advances `d` past it.
fn descriptor<'a>(d: &mut &'a [u8]) -> Option<(u8, &'a [u8])> {
    let tag = *d.first()?;
    let mut len = 0usize;
    let mut i = 1;
    loop {
        let b = *d.get(i)?;
        len = (len << 7) | usize::from(b & 0x7f);
        i += 1;
        if b & 0x80 == 0 || i > 4 {
            break;
        }
    }
    let body = d.get(i..i + len)?;
    *d = &d[i + len..];
    Some((tag, body))
}

/// Full-box table entries after version/flags and the entry count.
fn entries(d: &[u8], width: usize) -> impl Iterator<Item = &[u8]> {
    let count = if d.len() >= 8 { be32(d, 4) as usize } else { 0 };
    let avail = d.len().saturating_sub(8) / width;
    d.get(8..)
        .unwrap_or_default()
        .chunks_exact(width)
        .take(count.min(avail))
}

fn frames(t: &Tables) -> io::Result<Vec<Frame>> {
    let sizes: Vec<u32> = match t.stsz {
        Some(d) if d.len() >= 12 => {
            let fixed = be32(d, 4);
            let count = be32(d, 8) as usize;
            if fixed != 0 {
                vec![fixed; count]
            } else {
                let avail = (d.len() - 12) / 4;
                (0..count.min(avail)).map(|i| be32(d, 12 + 4 * i)).collect()
            }
        }
        _ => Vec::new(),
    };
    let chunks: Vec<u64> = match (t.stco, t.co64) {
        (_, Some(d)) => entries(d, 8).map(|e| be64(e, 0)).collect(),
        (Some(d), _) => entries(d, 4).map(|e| u64::from(be32(e, 0))).collect(),
        _ => Vec::new(),
    };
    let stsc: Vec<(u32, u32)> = t
        .stsc
        .map(|d| entries(d, 12).map(|e| (be32(e, 0), be32(e, 4))).collect())
        .unwrap_or_default();
    let mut deltas = t
        .stts
        .map(|d| {
            entries(d, 8)
                .map(|e| (be32(e, 0), be32(e, 4) as i32))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .flat_map(|(n, d)| std::iter::repeat_n(d, n as usize));
    let mut offsets = t
        .ctts
        .map(|d| {
            entries(d, 8)
                .map(|e| (be32(e, 0), be32(e, 4) as i32))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .flat_map(|(n, o)| std::iter::repeat_n(o, n as usize));
    let sync: Option<Vec<u32>> = t.stss.map(|d| entries(d, 4).map(|e| be32(e, 0)).collect());
    let sdtp = t.sdtp.and_then(|d| d.get(4..)).unwrap_or_default();

    let mut out = Vec::with_capacity(sizes.len());
    let mut ticks = 0i64;
    'chunks: for (c, &chunk_offset) in chunks.iter().enumerate() {
        let chunk_no = c as u32 + 1;
        let per_chunk = stsc
            .iter()
            .rev()
            .find(|&&(first, _)| first <= chunk_no)
            .map_or(0, |&(_, n)| n);
        let mut offset = chunk_offset;
        for _ in 0..per_chunk {
            let index = out.len();
            let Some(&size) = sizes.get(index) else {
                break 'chunks;
            };
            let delta = deltas.next().unwrap_or(0);
            let key = sync
                .as_ref()
                .is_none_or(|s| s.binary_search(&(index as u32 + 1)).is_ok());
            out.push(Frame {
                offset,
                size,
                time: ticks as f64,
                dur: f64::from(delta),
                ctts: offsets.next().unwrap_or(0),
                key,
                sdtp: sdtp.get(index).copied().unwrap_or(0),
                ftype: 0,
            });
            offset += u64::from(size);
            ticks += i64::from(delta);
        }
    }
    Ok(out)
}

/// `ExtractPPS` of the original: the `avcC` tail from the PPS count on (the SPS may differ).
fn extract_pps(s: &[u8]) -> &[u8] {
    if s.len() <= 10 {
        return s;
    }
    let sps_len = usize::from(be16(s, 6));
    if sps_len + 9 >= s.len() {
        return s;
    }
    &s[sps_len + 8..]
}

/// `IsSameDSI`: equal configuration, or for H.264 equal PPS (the SPS may differ). Audio and
/// H.265 configurations must match exactly.
pub(crate) fn same_dsi(t: &SrcTrack, a: &[u8], b: &[u8]) -> bool {
    a == b || (t.kind == Kind::Video && t.is_h264() && extract_pps(a) == extract_pps(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_asc_from_esds() {
        // From "Fragment of 05161259_0066": ES(3) > DecoderConfig(4) > DecSpecificInfo(5) = 11 90
        let esds = [
            0, 0, 0, 0, 0x03, 0x80, 0x80, 0x22, 0x00, 0x00, 0x00, 0x04, 0x80, 0x80, 0x16, 0x40,
            0x15, 0x00, 0x20, 0x00, 0x00, 0x01, 0xf4, 0x00, 0x00, 0x01, 0xf4, 0x00, 0x05, 0x80,
            0x80, 0x05, 0x11, 0x90, 0x00, 0x00, 0x00, 0x06, 0x80, 0x80, 0x01, 0x02,
        ];
        assert_eq!(
            audio_specific_config(&esds),
            Some(&[0x11, 0x90, 0, 0, 0][..])
        );
    }

    #[test]
    fn pps_comparison_ignores_sps() {
        let a = [
            1, 0x4d, 0, 0x32, 0xff, 0xe1, 0, 2, 0x67, 0xaa, 1, 0, 2, 0x68, 0xce,
        ];
        let mut b = a;
        b[9] = 0xbb; // different SPS byte
        let t = SrcTrack {
            id: 1,
            kind: Kind::Video,
            fourcc: *b"avc1",
            format: [0; 3],
            dsi: Vec::new(),
            scale: 1,
            duration: 0,
            frames: Vec::new(),
        };
        assert!(same_dsi(&t, &a, &b));
        let mut c = a;
        c[14] = 0xcf; // different PPS
        assert!(!same_dsi(&t, &a, &c));
    }
}
