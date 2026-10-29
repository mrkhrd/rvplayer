//! GPS data inside MP4/MOV files (`TGpsInfoManager.LoadMOVUdat` / `LoadMOVTrak`), in the
//! original's order:
//! 1. text in `moov/udta` (unless its first entry is camera identification), read as a log;
//! 2. GPS blocks: `free` atoms tagged "GPS " in the media data, found through the Novatek
//!    `gps ` index in `moov` (offset/size pairs; not known to the original) or, like the
//!    original's DOD reader, right after the runs of samples of a track;
//! 3. samples of text/data tracks (`tx3g`, `text`, `mp4s`, `RVMI`), one record each.
//!
//! Records get their video time; the result's times are seconds of video time.
//! Media data is never read as a whole: only box headers, the index, blocks (at most 64 KiB)
//! and text samples (at most 1 MiB each).

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::format::RawItem;
use super::formats::dod::LINE_PREFIX;
use super::manager::{Env, load_items, split_to_items};
use super::util::{SEC, be_u32, le_u32, pos};
use super::{GpsError, GpsLog, LoadOptions, MAX_LOG_SIZE, file_times, finish, formats, local_tz_offset_days};
use crate::mp4::{self, Movie};

/// Largest GPS block payload read (`SplitMOV` buffer).
const MAX_BLOCK: u32 = 0x10000;
/// Text samples larger than this are not GPS records.
const MAX_TEXT_SAMPLE: u32 = 1 << 20;
/// `gps ` index entries beyond this are ignored.
const MAX_INDEX_ENTRIES: u64 = 1 << 20;
/// Sample runs checked per track before giving up when no block follows them (DOD: 6).
const RUNS_WITHOUT_BLOCKS: usize = 6;
/// Codecs of the tracks read as text records (`formatType` 3).
const TEXT_CODECS: [&[u8; 4]; 4] = [b"TX3G", b"TEXT", b"RVMI", b"MP4S"];
/// Encrypted ("XOR-coded") Azdome-style blocks have this at payload offset 2.
const XOR_MARK: [u8; 8] = [0xaa, 0xaa, 0xf2, 0xe1, 0xf0, 0xee, 0x54, 0x54];

/// Reads `len` bytes at `offset`.
fn read_at<R: Read + Seek>(r: &mut R, offset: u64, len: usize) -> Option<Vec<u8>> {
    r.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// Child boxes of [start, end): (type, body start, end).
fn boxes<R: Read + Seek>(r: &mut R, start: u64, end: u64) -> Vec<([u8; 4], u64, u64)> {
    let mut out = Vec::new();
    let mut p = start;
    while p.saturating_add(8) <= end && out.len() < 100_000 {
        let Some(h) = read_at(r, p, 8) else { break };
        let mut size = u64::from(be_u32(&h, 0).unwrap_or(0));
        let kind: [u8; 4] = [h[4], h[5], h[6], h[7]];
        let mut header = 8;
        if size == 1 {
            let Some(ext) = read_at(r, p + 8, 8) else { break };
            size = u64::from_be_bytes(ext.try_into().unwrap_or([0; 8]));
            header = 16;
        } else if size == 0 {
            size = end - p;
        }
        if size < header || p.saturating_add(size) > end {
            break;
        }
        out.push((kind, p + header, p + size));
        p += size;
    }
    out
}

/// Body of the first `moov` child of the given type, if not larger than `limit`.
fn moov_child<R: Read + Seek>(r: &mut R, file_len: u64, kind: &[u8; 4], limit: u64) -> Option<Vec<u8>> {
    let (_, mb, me) = boxes(r, 0, file_len).into_iter().find(|b| &b.0 == b"moov")?;
    let (_, b, e) = boxes(r, mb, me).into_iter().find(|c| &c.0 == kind)?;
    if e - b > limit {
        return None;
    }
    read_at(r, b, (e - b) as usize)
}

/// `ReadUDAT`: udta text, unless it starts with camera identification.
fn udta_text<R: Read + Seek>(r: &mut R, file_len: u64) -> Option<Vec<u8>> {
    let body = moov_child(r, file_len, b"udta", MAX_LOG_SIZE)?;
    let id = body.get(4..8)?;
    let camera = [&b"AMBA"[..], b"\xa9inf", b"\xa9enc", b"cprt"].contains(&id);
    (!camera && body.len() > 10).then_some(body)
}

/// What a GPS block holds.
enum Block {
    Gps(Vec<u8>),
    Encrypted,
}

/// The `free` atom at `offset`: its GPS payload (the atom from byte 16) if it is tagged
/// "GPS ", and its size for walking to the next atom. `None` when there is no `free` atom.
fn read_block<R: Read + Seek>(r: &mut R, offset: u64, file_len: u64) -> Option<(Option<Block>, u64)> {
    if offset.saturating_add(16) > file_len {
        return None;
    }
    let h = read_at(r, offset, 16)?;
    if &h[4..8] != b"free" {
        return None;
    }
    let size = u64::from(be_u32(&h, 0)?);
    if &h[8..12] != b"GPS " {
        return Some((None, size));
    }
    let declared = le_u32(&h, 12)?;
    let room = file_len - offset - 16;
    let len = if declared == 0 || declared > MAX_BLOCK { size.saturating_sub(16) } else { u64::from(declared) };
    let len = len.min(u64::from(MAX_BLOCK)).min(room);
    let payload = read_at(r, offset + 16, len as usize)?;
    let encrypted = pos(b"LIGOGPSINFO", &payload).is_some() || payload.get(2..10) == Some(&XOR_MARK[..]);
    Some((Some(if encrypted { Block::Encrypted } else { Block::Gps(payload) }), size))
}

fn block_item(frame_time: f64, payload: &[u8]) -> RawItem {
    RawItem::timed(frame_time, [LINE_PREFIX, payload].concat())
}

/// Video times taken from where the blocks are stored are only useful when the blocks are
/// spread over the media data; when most share a time (stored together, e.g. at the end),
/// the fixes keep their own GPS times instead.
fn spread_or_untimed(mut items: Vec<RawItem>) -> Vec<RawItem> {
    let mut times: Vec<f64> = items.iter().map(|i| i.frame_time).collect();
    times.sort_by(f64::total_cmp);
    times.dedup();
    if items.len() > 1 && times.len() * 2 < items.len() {
        for i in &mut items {
            i.frame_time = 0.0;
        }
    }
    items
}

/// Records from the Novatek `gps ` index (8 header bytes, then big-endian offset/size pairs);
/// a block's video time is that of the last sample stored before it.
fn indexed_blocks<R: Read + Seek>(r: &mut R, movie: &Movie, file_len: u64, encrypted: &mut usize) -> Vec<RawItem> {
    let Some(index) = moov_child(r, file_len, b"gps ", 8 * MAX_INDEX_ENTRIES + 8) else { return vec![] };
    let mut ends: Vec<(u64, f64)> = movie
        .tracks
        .iter()
        .flat_map(|t| t.samples())
        .map(|s| (s.offset.saturating_add(u64::from(s.size)), s.time))
        .collect();
    ends.sort_by_key(|e| e.0);
    let mut out = Vec::new();
    for e in index.get(8..).unwrap_or_default().chunks_exact(8) {
        let Some(offset) = be_u32(e, 0).map(u64::from).filter(|&o| o > 0) else { continue };
        let k = ends.partition_point(|&(end, _)| end <= offset);
        let frame_time = k.checked_sub(1).map_or(0.0, |k| ends[k].1.max(0.0)) * SEC;
        match read_block(r, offset, file_len) {
            Some((Some(Block::Gps(p)), _)) => out.push(block_item(frame_time, &p)),
            Some((Some(Block::Encrypted), _)) => *encrypted += 1,
            _ => {}
        }
    }
    out
}

/// `TGpsFormat_DOD.SplitMOV`: blocks stored right after a run of contiguous samples. The
/// original looks only behind the second track (audio); the other tracks are tried after it.
/// A track is given up after [`RUNS_WITHOUT_BLOCKS`] runs without a block.
fn scanned_blocks<R: Read + Seek>(r: &mut R, movie: &Movie, file_len: u64, encrypted: &mut usize) -> Vec<RawItem> {
    let n = movie.tracks.len();
    for ti in (1..n).chain(0..n.min(1)) {
        let samples = movie.tracks[ti].samples();
        let mut out = Vec::new();
        let (mut runs, mut found) = (0usize, 0usize);
        let mut i = 0usize;
        while i < samples.len() {
            while i + 1 < samples.len()
                && samples[i + 1].offset == samples[i].offset.saturating_add(u64::from(samples[i].size))
            {
                i += 1;
            }
            if runs >= RUNS_WITHOUT_BLOCKS && found == 0 {
                break;
            }
            runs += 1;
            let s = samples[i];
            let mut po = s.offset.saturating_add(u64::from(s.size));
            while let Some((block, size)) = read_block(r, po, file_len) {
                match block {
                    Some(Block::Gps(p)) => {
                        out.push(block_item(s.time.max(0.0) * SEC, &p));
                        found += 1;
                    }
                    Some(Block::Encrypted) => {
                        *encrypted += 1;
                        found += 1;
                    }
                    None => {}
                }
                if size < 16 {
                    break;
                }
                po = po.saturating_add(size);
            }
            i += 1;
        }
        if !out.is_empty() {
            return out;
        }
    }
    vec![]
}

/// Records of the text/data tracks: the 16-bit length prefix of text samples and trailing
/// line breaks are removed.
fn text_track_items<R: Read + Seek>(r: &mut R, movie: &Movie, skip_rvmi: bool) -> Vec<RawItem> {
    let mut out = Vec::new();
    for t in &movie.tracks {
        let codec = t.codec.to_ascii_uppercase();
        if !TEXT_CODECS.iter().any(|c| c[..] == codec[..]) || (skip_rvmi && codec == b"RVMI") {
            continue;
        }
        for s in t.samples() {
            if s.size > MAX_TEXT_SAMPLE {
                continue;
            }
            let Some(mut d) = read_at(r, s.offset, s.size as usize) else { continue };
            if d.len() > 2 && d.len() == usize::from(d[0]) * 256 + usize::from(d[1]) + 2 {
                d.drain(..2);
            }
            while matches!(d.last(), Some(b'\n' | b'\r')) {
                d.pop();
            }
            out.push(RawItem::timed(s.time * SEC, d));
        }
    }
    out
}

/// GPS data embedded in a video. `skip_rvmi` leaves out the RVMI track (read by
/// [`crate::rvmi`] instead). `NoData` when there is none; `Unsupported` when the only GPS
/// blocks are encrypted.
pub fn read_embedded<R: Read + Seek>(
    r: &mut R,
    movie: &Movie,
    name: &str,
    opts: &LoadOptions,
    skip_rvmi: bool,
) -> Result<GpsLog, GpsError> {
    let file_len = r.seek(SeekFrom::End(0)).map_err(|e| GpsError::Io(e.to_string()))?;
    let duration = movie.duration_secs() * SEC;
    let companion = |_: &str| None;
    let env = Env { name: name.to_string(), file_times: opts.file_times, tz: opts.tz, companion: &companion };
    if let Some(text) = udta_text(r, file_len) {
        let mut f = formats::all();
        let (items, frame0) = split_to_items(&mut f, &text);
        if let Some(l) = load_items(&mut f, Some(text), items, frame0, "udat", duration, &env)
            && let Ok(log) = finish(l, opts.file_times)
        {
            return Ok(log);
        }
    }
    let mut encrypted = 0;
    let mut items = indexed_blocks(r, movie, file_len, &mut encrypted);
    if items.is_empty() {
        items = scanned_blocks(r, movie, file_len, &mut encrypted);
    }
    items = spread_or_untimed(items);
    if items.is_empty() {
        items = text_track_items(r, movie, skip_rvmi);
    }
    if !items.is_empty() {
        let mut f = formats::all();
        if let Some(l) = load_items(&mut f, None, items, 0.0, "mov", duration, &env)
            && let Ok(log) = finish(l, opts.file_times)
        {
            return Ok(log);
        }
    }
    if encrypted > 0 {
        return Err(GpsError::Unsupported(format!("{encrypted} encrypted GPS blocks (LIGOGPSINFO or XOR-coded)")));
    }
    Err(GpsError::NoData)
}

/// [`read_embedded`] on a file, including the RVMI track.
pub fn load_video_path(path: &Path) -> Result<GpsLog, GpsError> {
    let io = |e: std::io::Error| GpsError::Io(e.to_string());
    let file = std::fs::File::open(path).map_err(io)?;
    let meta = file.metadata().map_err(io)?;
    let mut r = std::io::BufReader::new(file);
    let movie = mp4::read_movie(&mut r).map_err(io)?;
    let opts = LoadOptions { tz: local_tz_offset_days(), file_times: file_times(&meta) };
    read_embedded(&mut r, &movie, &path.to_string_lossy(), &opts, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::formats::dod::tests::payload_d;
    use crate::gps::formats::novatek::tests::atom;
    use crate::testutil::{be, mk, movie_with_track};
    use std::io::Cursor;

    const OPTS: LoadOptions = LoadOptions { tz: 0.0, file_times: (0.0, 0.0) };

    /// A one-track (video) movie of `secs` one-second samples with `blocks` appended after
    /// `mdat`, and a `gps ` index in `moov` listing them when `indexed`.
    fn novatek_movie(secs: usize, blocks: &[Vec<u8>], indexed: bool) -> Vec<u8> {
        let base = movie_with_track(b"vide", b"avc1", 1, &vec![vec![0u8; 16]; secs]);
        let mut entries = Vec::new();
        let mut off = base.len() as u32;
        for b in blocks {
            entries.push(off);
            entries.push(b.len() as u32);
            off += b.len() as u32;
        }
        if !indexed {
            return [base, blocks.concat()].concat();
        }
        // insert `gps ` into moov: rebuild by appending the atom to moov and fixing sizes
        let moov_at = base.windows(4).position(|w| w == b"moov").unwrap() - 4;
        let moov_size = u32::from_be_bytes(base[moov_at..moov_at + 4].try_into().unwrap()) as usize;
        let gps = mk(b"gps ", &[be(&[0x0101, blocks.len() as u32]), be(&entries)].concat());
        let shift = gps.len() as u32;
        let mut out = base[..moov_at + moov_size].to_vec();
        out.extend_from_slice(&gps);
        out[moov_at..moov_at + 4].copy_from_slice(&((moov_size as u32) + shift).to_be_bytes());
        out.extend_from_slice(&base[moov_at + moov_size..]);
        // chunk offsets and index entries moved by the inserted atom
        let stco = out.windows(4).position(|w| w == b"stco").unwrap();
        let co = u32::from_be_bytes(out[stco + 12..stco + 16].try_into().unwrap()) + shift;
        out[stco + 12..stco + 16].copy_from_slice(&co.to_be_bytes());
        let gps_at = out.windows(4).position(|w| w == b"gps ").unwrap() + 4 + 8;
        for k in 0..blocks.len() {
            let p = gps_at + 8 * k;
            let v = u32::from_be_bytes(out[p..p + 4].try_into().unwrap()) + shift;
            out[p..p + 4].copy_from_slice(&v.to_be_bytes());
        }
        out.extend_from_slice(&blocks.concat());
        out
    }

    fn load(file: &[u8]) -> Result<GpsLog, GpsError> {
        let mut r = Cursor::new(file);
        let movie = mp4::read_movie(&mut r).unwrap();
        read_embedded(&mut r, &movie, "a.mp4", &OPTS, false)
    }

    #[test]
    fn novatek_blocks_through_the_gps_index() {
        let blocks: Vec<Vec<u8>> = (0..5)
            .map(|k| atom(13, 6, 26 + k, b'A', 6003.8176 - k as f32 * 0.02, b'N', 3019.6213, b'E', 20.0, 180.0))
            .collect();
        let file = novatek_movie(5, &blocks, true);
        let log = load(&file).unwrap();
        assert_eq!(log.format, "movNovatek");
        let t = log.track;
        assert_eq!(t.gps.len(), 5);
        // all blocks are stored after the media: the fixes keep their GPS times
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert!((p.lat - 60.063_626).abs() < 1e-5 && (p.lon - 30.327_021).abs() < 1e-5);
        assert!((p.speed_kmh - 20.0 * 1.85325).abs() < 1e-6 && p.course == 180.0);
        assert_eq!(t.gps[4].time, 4.0);
    }

    /// Video samples of one second each with a GPS block stored after every sample (each
    /// sample its own chunk), optionally listed in a `gps ` index.
    fn interleaved_movie(blocks: &[Vec<u8>], indexed: bool) -> Vec<u8> {
        let n = blocks.len() as u32;
        let build = |mdat_at: u32| {
            let mut offsets = Vec::new();
            let mut entries = Vec::new();
            let mut p = mdat_at + 8;
            for b in blocks {
                offsets.push(p);
                entries.push(p + 16);
                entries.push(b.len() as u32);
                p += 16 + b.len() as u32;
            }
            let mut stsd = be(&[0, 1, 16]);
            stsd.extend_from_slice(b"avc1");
            stsd.extend_from_slice(&[0; 8]);
            let stbl = [
                mk(b"stsd", &stsd),
                mk(b"stts", &be(&[0, 1, n, 1])),
                mk(b"stsc", &be(&[0, 1, 1, 1, 1])),
                mk(b"stsz", &be(&[0, 16, n])),
                mk(b"stco", &[be(&[0, n]), be(&offsets)].concat()),
            ]
            .concat();
            let mut hdlr = be(&[0, 0]);
            hdlr.extend_from_slice(b"vide");
            hdlr.extend_from_slice(&[0; 12]);
            let minf = mk(b"minf", &mk(b"stbl", &stbl));
            let mdia = mk(b"mdia", &[mk(b"mdhd", &be(&[0, 0, 0, 1, n, 0])), mk(b"hdlr", &hdlr), minf].concat());
            let trak = mk(b"trak", &[mk(b"tkhd", &[0u8; 84]), mdia].concat());
            let mvhd = [be(&[0, 0, 0, 1, n]), vec![0; 80]].concat();
            let gps = if indexed { mk(b"gps ", &[be(&[0x0101, n]), be(&entries)].concat()) } else { vec![] };
            let moov = mk(b"moov", &[mk(b"mvhd", &mvhd), trak, gps].concat());
            [mk(b"ftyp", b"isom    isom"), moov].concat()
        };
        let head = build(0);
        let mut file = build(head.len() as u32);
        let body: Vec<u8> = blocks.iter().flat_map(|b| [vec![0u8; 16], b.clone()].concat()).collect();
        file.extend(mk(b"mdat", &body));
        file
    }

    #[test]
    fn interleaved_blocks_get_video_times() {
        // GPS clock 13:06:26.. while the video sample before each block is at 0, 1, 2, 3 s
        let blocks: Vec<Vec<u8>> = (0..4)
            .map(|k| atom(13, 6, 26 + k, b'A', 6003.8176 - k as f32 * 0.02, b'N', 3019.6213, b'E', 20.0, 180.0))
            .collect();
        for indexed in [true, false] {
            let log = load(&interleaved_movie(&blocks, indexed)).unwrap();
            assert_eq!(log.format, "movNovatek");
            let t = log.track;
            assert_eq!(t.gps.len(), 4, "indexed {indexed}");
            assert_eq!(t.start.unwrap().format("%H:%M:%S").to_string(), "13:06:26");
            let times: Vec<f64> = t.gps.iter().map(|p| p.time).collect();
            assert_eq!(times, [0.0, 1.0, 2.0, 3.0]);
        }
    }

    #[test]
    fn mai70_blocks_give_one_point_per_fix_at_its_first_frame() {
        use crate::gps::formats::mai70::tests::block;
        let fix = |k: i32, speed: u32| Some((600_695_419 + k * 1000, 312_231_780 + k * 1000, 90, speed));
        // One block a second: no fix, the same fix for 12 s while moving (repeated old fix),
        // a new fix, then a fix held while standing.
        let blocks: Vec<Vec<u8>> = std::iter::once(None)
            .chain([fix(0, 100); 12])
            .chain([fix(1, 100)])
            .chain([fix(2, 0); 12])
            .map(block)
            .collect();
        let file = interleaved_movie(&blocks, true);
        let mut r = Cursor::new(&file);
        let movie = mp4::read_movie(&mut r).unwrap();
        let opts = LoadOptions { tz: 0.0, file_times: (45_000.0, 45_000.0 + 26.0 * SEC) };
        let log = read_embedded(&mut r, &movie, "NO20261024-154056-012644F.MP4", &opts, false).unwrap();
        assert_eq!(log.format, "mov70mai");
        let t = log.track;
        let got: Vec<(f64, u8, f64)> = t.gps.iter().map(|p| (p.time, p.warning, p.speed_kmh)).collect();
        assert_eq!(got, [(1.0, 2, 100.0), (13.0, 0, 100.0), (14.0, 0, 0.0)]);
        assert!((t.gps[1].lat - (60.0 + 6.96419 / 60.0)).abs() < 1e-9);
    }

    #[test]
    fn blocks_found_by_scanning_after_samples() {
        let blocks: Vec<Vec<u8>> = (0..3)
            .map(|k| atom(13, 6, 26 + k, b'A', 6003.8176, b'N', 3019.6213 + k as f32 * 0.05, b'E', 0.0, 90.0))
            .collect();
        let file = novatek_movie(3, &blocks, false);
        let t = load(&file).unwrap().track;
        assert_eq!(t.gps.len(), 3);
    }

    #[test]
    fn dod_layout_blocks() {
        let blocks: Vec<Vec<u8>> = (0..3)
            .map(|k| {
                let p = payload_d(13, 6, 26 + k, 6003.8176, 3019.6213 + k as f32 * 0.05, 10.0, b'A');
                let mut a = ((16 + p.len()) as u32).to_be_bytes().to_vec();
                a.extend_from_slice(b"freeGPS ");
                a.extend_from_slice(&(p.len() as u32).to_le_bytes());
                a.extend_from_slice(&p);
                a
            })
            .collect();
        let log = load(&novatek_movie(3, &blocks, true)).unwrap();
        assert_eq!(log.format, "movDOD_D");
        assert_eq!(log.track.gps.len(), 3);
    }

    #[test]
    fn encrypted_blocks_are_reported() {
        let mut a = atom(13, 6, 26, b'A', 6003.8176, b'N', 3019.6213, b'E', 0.0, 0.0);
        a[20..31].copy_from_slice(b"LIGOGPSINFO");
        match load(&novatek_movie(2, &[a.clone(), a], true)) {
            Err(GpsError::Unsupported(m)) => assert!(m.contains("LIGOGPSINFO"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn text_track_samples_are_records() {
        let lines = [
            "$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,10.0,90.0,160516,,,A*00",
            "$GPRMC,130627.000,A,6003.8176,N,03019.6413,E,10.0,90.0,160516,,,A*00",
        ];
        let fixed = crate::gps::test_support::fix_checksums(&lines.join("\n"), &[0, 1]);
        let payloads: Vec<Vec<u8>> = fixed
            .split('\n')
            .map(|l| {
                let mut v = (l.len() as u16).to_be_bytes().to_vec();
                v.extend_from_slice(l.as_bytes());
                v.extend_from_slice(b"\r\n");
                v
            })
            .collect();
        // the length prefix does not cover the line break here: it is kept, then trimmed
        let file = movie_with_track(b"sbtl", b"tx3g", 1, &payloads);
        let log = load(&file).unwrap();
        assert_eq!(log.format, "movNMEA");
        assert_eq!(log.track.gps.len(), 2);
        assert_eq!(log.track.gps[1].time - log.track.gps[0].time, 1.0);
    }

    #[test]
    fn garbage_videos_do_not_panic() {
        let blocks: Vec<Vec<u8>> =
            (0..3).map(|k| atom(13, 6, 26 + k, b'A', 6003.8, b'N', 3019.6, b'E', 1.0, 2.0)).collect();
        let file = novatek_movie(3, &blocks, true);
        for seed in 0..60u64 {
            let mut f = file.clone();
            let noise = crate::gps::test_support::noise(seed, 40);
            let at = (seed as usize * 37) % (f.len() - 40);
            f[at..at + 40].copy_from_slice(&noise);
            let mut r = Cursor::new(&f);
            if let Ok(movie) = mp4::read_movie(&mut r) {
                let _ = read_embedded(&mut r, &movie, "a.mp4", &OPTS, false);
            }
            let cut = &file[..(seed as usize * 53) % file.len()];
            let mut r = Cursor::new(cut);
            if let Ok(movie) = mp4::read_movie(&mut r) {
                let _ = read_embedded(&mut r, &movie, "a.mp4", &OPTS, false);
            }
        }
    }
}
