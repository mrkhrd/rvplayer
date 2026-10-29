//! The reference file's header (the original's `TFileRecoverHeaderThread.ReadRef`): the raw `moov`
//! that the new header is copied from, per-track sample templates, the median audio frame size
//! used to split audio runs, and the shape of the camera's video frames.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use rv_core::mp4;

use crate::Error;
use crate::boxes::{FourCC, atoms, be16, be32, be64, child, path};
use crate::nal::{self, LENGTH_SIZE, Observed, VideoStyle};

/// A `moov` larger than this is treated as corrupt.
const MAX_MOOV: u64 = 64 << 20;
/// Reference frames examined to learn the frame style.
const LEARN_FRAMES: usize = 64;

/// Timing fields of one reference sample, reused for the repaired sample at the same index
/// (the original copies the whole 40-byte sample record).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Template {
    /// `stts` delta, raw 32 bits (a negative step is stored as its two's complement).
    pub duration: u32,
    /// `ctts` offset, raw 32 bits.
    pub ctts: u32,
    /// `sdtp` byte.
    pub sdtp: u8,
}

#[derive(Debug, Clone)]
pub(crate) struct RefTrack {
    pub id: u32,
    pub handler: FourCC,
    pub timescale: u32,
    /// Number of samples (`stsz` count).
    pub sample_count: usize,
    /// One template per sample; for constant-size tracks only the first sample's.
    pub templates: Vec<Template>,
    /// Sample sizes; empty for constant-size tracks.
    pub sizes: Vec<u32>,
    /// `stsz` constant sample size; 0 when every sample has its own size.
    pub fixed_size: u32,
    /// For constant-size (PCM) tracks: bytes per packet and `stsz` samples per packet.
    pub packet: (u32, u32),
    /// Template period for samples past the reference's count: its GOP length (distance between
    /// the first two sync samples), 1 without them.
    pub period: usize,
    /// Media time of the first non-empty edit, when the track has an edit list.
    pub edit_media_time: Option<i64>,
}

#[derive(Debug, Clone)]
pub(crate) struct Reference {
    /// Body of the reference `moov`.
    pub moov: Vec<u8>,
    pub tracks: Vec<RefTrack>,
    /// Index into `tracks` of the video track that is rebuilt (the first `vide` track).
    pub video: usize,
    /// Index of the audio track that is rebuilt (the first `soun` track), if any.
    pub audio: Option<usize>,
    /// The original's `avgASize`: median audio sample size; 0 disables splitting audio runs.
    pub avg_audio_size: u32,
    pub style: VideoStyle,
}

impl Reference {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let reference = |e: &dyn std::fmt::Display| Error::Reference(e.to_string());
        let mut file = BufReader::new(File::open(path).map_err(|e| reference(&e))?);
        // The original requires the reference to open in its own container parser.
        let movie = mp4::read_movie(&mut file).map_err(|e| reference(&e))?;
        let moov = read_moov(&mut file)
            .map_err(|e| reference(&e))?
            .ok_or_else(|| reference(&"no complete moov box"))?;
        let tracks: Vec<RefTrack> = atoms(&moov)
            .filter(|a| &a.kind == b"trak")
            .filter_map(|a| parse_trak(a.body))
            .collect();

        let video = tracks
            .iter()
            .position(|t| &t.handler == b"vide")
            .ok_or_else(|| reference(&"no video track"))?;
        let core = movie
            .tracks
            .iter()
            .find(|t| t.id == tracks[video].id && &t.handler == b"vide")
            .ok_or_else(|| reference(&"no video track"))?;
        let hevc = match &core.codec {
            b"avc1" | b"avc3" => false,
            b"hvc1" | b"hev1" => true,
            other => {
                return Err(Error::Unsupported(format!(
                    "video codec {} (only H.264 and H.265 can be recovered)",
                    String::from_utf8_lossy(other)
                )));
            }
        };
        let length_size = core.codec_config.as_deref().and_then(|c| {
            let at = if hevc { 21 } else { 4 };
            c.get(at).map(|b| usize::from(b & 3) + 1)
        });
        if length_size.is_some_and(|n| n != LENGTH_SIZE) {
            return Err(Error::Unsupported(
                "video NAL units without 4-byte length prefixes".into(),
            ));
        }
        if tracks[video].templates.is_empty() {
            return Err(reference(&"the video track has no samples"));
        }

        let samples = core.samples();
        let seen = observe(&mut file, hevc, &samples[..samples.len().min(LEARN_FRAMES)])
            .map_err(|e| reference(&e))?;
        let style = nal::learn(hevc, &seen);

        let audio = tracks
            .iter()
            .position(|t| &t.handler == b"soun" && t.sample_count > 0);
        let avg_audio_size = audio
            .map(|i| &tracks[i])
            .filter(|t| t.fixed_size == 0)
            .map_or(0, |t| median(&t.sizes));
        Ok(Self {
            moov,
            tracks,
            video,
            audio,
            avg_audio_size,
            style,
        })
    }
}

/// Finds the top-level `moov` and returns its body (the original walks top-level boxes the same
/// way; it gives up on a box shorter than its header).
fn read_moov<R: Read + Seek>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let len = r.seek(SeekFrom::End(0))?;
    let mut pos = 0u64;
    while pos + 8 <= len {
        r.seek(SeekFrom::Start(pos))?;
        let mut h = [0u8; 8];
        r.read_exact(&mut h)?;
        let (size, header) = match u32::from_be_bytes(h[..4].try_into().unwrap()) {
            0 => (len - pos, 8),
            1 => {
                let mut ext = [0u8; 8];
                r.read_exact(&mut ext)?;
                (u64::from_be_bytes(ext), 16)
            }
            n => (u64::from(n), 8),
        };
        if size < header {
            return Ok(None);
        }
        if &h[4..] == b"moov" {
            if pos + size > len || size - header > MAX_MOOV {
                return Ok(None);
            }
            let mut body = vec![0u8; (size - header) as usize];
            r.read_exact(&mut body)?;
            return Ok(Some(body));
        }
        pos += size;
    }
    Ok(None)
}

fn parse_trak(trak: &[u8]) -> Option<RefTrack> {
    let tkhd = child(trak, b"tkhd")?;
    let id = be32(tkhd, if tkhd.first() == Some(&1) { 20 } else { 12 })?;
    let mdia = child(trak, b"mdia")?;
    let mdhd = child(mdia, b"mdhd")?;
    let timescale = be32(mdhd, if mdhd.first() == Some(&1) { 20 } else { 12 })?;
    let handler: FourCC = child(mdia, b"hdlr")?.get(8..12)?.try_into().ok()?;
    let stbl = path(mdia, &[b"minf", b"stbl"])?;

    let (fixed_size, sample_count, sizes) = match child(stbl, b"stsz") {
        Some(d) => {
            let fixed = be32(d, 4)?;
            let count = be32(d, 8)? as usize;
            if fixed != 0 {
                (fixed, count, Vec::new())
            } else {
                let sizes: Vec<u32> = (0..count).map_while(|i| be32(d, 12 + 4 * i)).collect();
                (0, sizes.len(), sizes)
            }
        }
        None => (0, 0, Vec::new()),
    };
    // Constant-size (PCM) tracks are rebuilt from their first `stts` delta only.
    let count = if fixed_size != 0 {
        sample_count.min(1)
    } else {
        sample_count
    };
    let durations = runs(child(stbl, b"stts"), count, |d, at| be32(d, at + 4));
    let ctts = runs(child(stbl, b"ctts"), count, |d, at| be32(d, at + 4));
    let sdtp = child(stbl, b"sdtp").map_or(&[][..], |d| d.get(4..).unwrap_or(&[]));
    let templates = (0..count)
        .map(|i| Template {
            duration: durations[i],
            ctts: ctts[i],
            sdtp: sdtp.get(i).copied().unwrap_or(0),
        })
        .collect();

    let period = child(stbl, b"stss")
        .filter(|d| be32(d, 4).is_some_and(|n| n >= 2))
        .and_then(|d| Some(be32(d, 12)?.checked_sub(be32(d, 8)?)? as usize))
        .filter(|&p| p > 0)
        .unwrap_or(1);
    let packet = child(stbl, b"stsd").map_or((fixed_size.max(1), 1), |d| packet(d, fixed_size));
    let edit_media_time = path(trak, &[b"edts", b"elst"]).and_then(first_media_time);
    Some(RefTrack {
        id,
        handler,
        timescale,
        sample_count,
        templates,
        sizes,
        fixed_size,
        packet,
        period,
        edit_media_time,
    })
}

/// Expands a run-length table (`stts` / `ctts`: count, value) to `count` values; missing entries
/// repeat the last value (0 without any).
fn runs(
    table: Option<&[u8]>,
    count: usize,
    value: impl Fn(&[u8], usize) -> Option<u32>,
) -> Vec<u32> {
    let mut out = Vec::with_capacity(count);
    if let Some(d) = table {
        let entries = be32(d, 4).unwrap_or(0) as usize;
        for e in 0..entries {
            let at = 8 + 8 * e;
            let (Some(n), Some(v)) = (be32(d, at), value(d, at)) else {
                break;
            };
            let n = (n as usize).min(count - out.len());
            out.extend(std::iter::repeat_n(v, n));
            if out.len() == count {
                break;
            }
        }
    }
    let last = out.last().copied().unwrap_or(0);
    out.resize(count, last);
    out
}

/// Bytes per packet and `stsz` samples per packet of a constant-size sound track. ISO files state
/// the byte size of a sample in `stsz`; QuickTime PCM uses size 1 per audio frame and describes the
/// frame in the sound sample description.
fn packet(stsd: &[u8], fixed: u32) -> (u32, u32) {
    if fixed > 1 {
        return (fixed, 1);
    }
    let Some(e) = stsd.get(8..) else {
        return (1, 1);
    };
    // SampleEntry(16) + version(2) revision(2) vendor(4) channels(2) bits(2) ... ; v1 adds
    // samples_per_packet(4) bytes_per_packet(4) bytes_per_frame(4) bytes_per_sample(4) at 36.
    let version = be16(e, 16).unwrap_or(0);
    if version == 1
        && let (Some(spp), Some(bpf)) = (be32(e, 36), be32(e, 44))
        && spp > 0
        && bpf > 0
    {
        return (bpf, spp);
    }
    let channels = u32::from(be16(e, 24).unwrap_or(0));
    let bits = u32::from(be16(e, 26).unwrap_or(0));
    let bytes = channels * bits / 8;
    if bytes > 0 { (bytes, 1) } else { (1, 1) }
}

/// Media time of the first non-empty edit of an `elst` body.
fn first_media_time(elst: &[u8]) -> Option<i64> {
    let v1 = elst.first() == Some(&1);
    let count = be32(elst, 4)? as usize;
    (0..count)
        .map_while(|i| {
            if v1 {
                be64(elst, 8 + 20 * i + 8).map(|t| t as i64)
            } else {
                be32(elst, 8 + 12 * i + 4).map(|t| i64::from(t as i32))
            }
        })
        .find(|&t| t >= 0)
}

/// Median as the original's `GetMedian` picks it: the element at index n/2 in sorted order.
pub(crate) fn median(values: &[u32]) -> u32 {
    if values.is_empty() {
        return 0;
    }
    let mut v = values.to_vec();
    let k = v.len() / 2;
    *v.select_nth_unstable(k).1
}

/// Reads the NAL headers of the reference's first frames.
fn observe<R: Read + Seek>(r: &mut R, hevc: bool, samples: &[mp4::Sample]) -> io::Result<Observed> {
    let style = VideoStyle {
        hevc,
        ..VideoStyle::original()
    };
    let mut seen = Observed::default();
    for s in samples {
        let mut nals = Vec::new();
        let mut at = 0u64;
        let size = u64::from(s.size);
        while at + (LENGTH_SIZE + style.header_len()) as u64 <= size {
            r.seek(SeekFrom::Start(s.offset + at))?;
            let mut p = [0u8; 8];
            r.read_exact(&mut p)?;
            let len = u32::from_be_bytes(p[..4].try_into().unwrap());
            if len == 0 || at + LENGTH_SIZE as u64 + u64::from(len) > size {
                break;
            }
            nals.push((len, style.header(&p)));
            at += LENGTH_SIZE as u64 + u64::from(len);
        }
        seen.frame(&style, &nals);
    }
    Ok(seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_takes_upper_middle() {
        assert_eq!(median(&[]), 0);
        assert_eq!(median(&[5]), 5);
        assert_eq!(median(&[4, 1, 3, 2]), 3);
        assert_eq!(median(&[9, 1, 5]), 5);
    }

    #[test]
    fn expands_run_tables() {
        let stts = [
            0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 10, 0, 0, 0, 1, 0, 0, 0, 20,
        ];
        let v = runs(Some(&stts), 5, |d, at| be32(d, at + 4));
        assert_eq!(v, [10, 10, 20, 20, 20]);
        assert_eq!(runs(None, 2, |d, at| be32(d, at + 4)), [0, 0]);
    }

    #[test]
    fn pcm_packet_from_sound_description() {
        let mut stsd = vec![0u8; 8 + 36];
        stsd[8 + 25] = 2; // channels
        stsd[8 + 27] = 16; // bits
        assert_eq!(packet(&stsd, 1), (4, 1));
        assert_eq!(packet(&stsd, 2), (2, 1));
        let mut v1 = vec![0u8; 8 + 52];
        v1[8 + 17] = 1;
        v1[8 + 39] = 64; // samples per packet
        v1[8 + 47] = 34; // bytes per frame
        assert_eq!(packet(&v1, 1), (34, 64));
    }
}
