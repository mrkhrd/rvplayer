//! Builder of small multi-track MOV files for the export tests: an H.264 / H.265 video track
//! (sample entry with `avcC` / `hvcC`, optional `ctts`, `sdtp`, `edts`), an AAC audio track
//! (`esds`), an optional RVMI data track and camera user data. Samples identify themselves:
//! video payloads carry `tag` and the frame index at bytes 6..11, audio payloads at 1..6.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use rv_core::testutil::{be, mk};

#[derive(Clone)]
pub struct Video {
    pub fourcc: [u8; 4],
    pub config: Vec<u8>,
    pub scale: u32,
    pub dur: u32,
    pub frames: usize,
    /// Key frame every `gop` frames.
    pub gop: usize,
    pub width: u16,
    pub height: u16,
    pub ctts: bool,
    pub sdtp: bool,
    pub edts: bool,
    /// Picture types of the frames after each key frame, written as H.264 access unit
    /// delimiters ('P' / 'B'); empty = no delimiters.
    pub aud: Vec<u8>,
}

#[derive(Clone)]
pub struct Audio {
    pub asc: Vec<u8>,
    pub scale: u32,
    pub frames: usize,
}

#[derive(Clone)]
pub struct Spec {
    pub tag: u8,
    pub video: Option<Video>,
    pub audio: Option<Audio>,
    /// RVMI records (time in 1/50 s, bytes).
    pub rvmi: Vec<(u32, Vec<u8>)>,
    pub model: &'static str,
}

pub const AVCC: [u8; 23] = [
    1, 0x4d, 0x00, 0x28, 0xff, 0xe1, 0, 8, 0x67, 0x4d, 0x00, 0x28, 0x9a, 0x64, 0x01, 0x40, 1, 0, 4,
    0x68, 0xee, 0x3c, 0x80,
];
pub const ASC: [u8; 2] = [0x11, 0x90];

pub fn video() -> Video {
    Video {
        fourcc: *b"avc1",
        config: AVCC.to_vec(),
        scale: 30000,
        dur: 1000,
        frames: 300,
        gop: 30,
        width: 1920,
        height: 1080,
        ctts: true,
        sdtp: true,
        edts: true,
        aud: Vec::new(),
    }
}

pub fn audio() -> Audio {
    Audio {
        asc: ASC.to_vec(),
        scale: 48000,
        frames: 469,
    }
}

pub fn spec(tag: u8) -> Spec {
    Spec {
        tag,
        video: Some(video()),
        audio: Some(audio()),
        rvmi: Vec::new(),
        model: "TESTCAM X1",
    }
}

pub fn ctts_of(i: usize, v: &Video) -> u32 {
    if i.is_multiple_of(v.gop) {
        v.dur
    } else {
        v.dur * (1 + (i % 3) as u32)
    }
}

pub fn video_payload(spec: &Spec, v: &Video, i: usize) -> Vec<u8> {
    let key = i.is_multiple_of(v.gop);
    let mut p = if !v.aud.is_empty() && !key {
        let kind = v.aud[(i % v.gop - 1) % v.aud.len()];
        vec![0, 0, 0, 2, 9, if kind == b'B' { 0x50 } else { 0x30 }]
    } else {
        vec![0, 0, 0, 2, 0x0c, 0]
    };
    p.push(spec.tag);
    p.extend_from_slice(&(i as u32).to_le_bytes());
    p.resize(if key { 600 } else { 100 + (i % 7) * 10 }, 0xaa);
    p
}

pub fn audio_payload(spec: &Spec, i: usize) -> Vec<u8> {
    let mut p = vec![b'A', spec.tag];
    p.extend_from_slice(&(i as u32).to_le_bytes());
    p.resize(200 + i % 3, 0x55);
    p
}

/// (tag, index) of an output sample of the given handler.
pub fn ident(payload: &[u8], video: bool) -> (u8, u32) {
    let at = if video { 6 } else { 1 };
    (
        payload[at],
        u32::from_le_bytes(payload[at + 1..at + 5].try_into().unwrap()),
    )
}

/// Builds a `trak` box given its chunk offsets.
type TrakBuilder = Box<dyn Fn(&[u32]) -> Vec<u8>>;

struct TrackData {
    trak_without_stco: TrakBuilder,
    samples: Vec<Vec<u8>>,
}

const CHUNK: usize = 5;

/// stsc for chunks of `CHUNK` samples.
fn stsc(n: usize) -> Vec<u8> {
    let full = n / CHUNK;
    let rem = n % CHUNK;
    let mut runs = vec![];
    if full > 0 {
        runs.push((1u32, CHUNK as u32));
    }
    if rem > 0 {
        runs.push((full as u32 + 1, rem as u32));
    }
    let mut b = be(&[0, runs.len() as u32]);
    for (first, per) in runs {
        b.extend(be(&[first, per, 1]));
    }
    mk(b"stsc", &b)
}

fn stsz(samples: &[Vec<u8>]) -> Vec<u8> {
    let mut b = be(&[0, 0, samples.len() as u32]);
    for s in samples {
        b.extend(be(&[s.len() as u32]));
    }
    mk(b"stsz", &b)
}

fn stco(offsets: &[u32]) -> Vec<u8> {
    let mut b = be(&[0, offsets.len() as u32]);
    b.extend(be(offsets));
    mk(b"stco", &b)
}

fn tkhd(id: u32, duration: u32, w: u16, h: u16, volume: u16) -> Vec<u8> {
    let mut t = vec![0u8; 84];
    t[3] = 3; // enabled, in movie
    t[12..16].copy_from_slice(&id.to_be_bytes());
    t[20..24].copy_from_slice(&duration.to_be_bytes());
    t[36..38].copy_from_slice(&volume.to_be_bytes());
    t[40..44].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    t[56..60].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    t[72..76].copy_from_slice(&0x4000_0000u32.to_be_bytes());
    t[76..80].copy_from_slice(&(u32::from(w) << 16).to_be_bytes());
    t[80..84].copy_from_slice(&(u32::from(h) << 16).to_be_bytes());
    mk(b"tkhd", &t)
}

fn mdhd(scale: u32, duration: u32) -> Vec<u8> {
    mk(b"mdhd", &be(&[0, 0, 0, scale, duration, 0x55c4_0000]))
}

fn hdlr(handler: &[u8; 4], name: &str) -> Vec<u8> {
    let mut h = be(&[0, 0]);
    h.extend_from_slice(handler);
    h.extend_from_slice(&[0; 12]);
    h.push(name.len() as u8);
    h.extend_from_slice(name.as_bytes());
    mk(b"hdlr", &h)
}

fn dinf() -> Vec<u8> {
    mk(
        b"dinf",
        &mk(b"dref", &[be(&[0, 1]), mk(b"url ", &be(&[1]))].concat()),
    )
}

fn video_trak(spec: &Spec, v: &Video) -> TrackData {
    let samples: Vec<Vec<u8>> = (0..v.frames).map(|i| video_payload(spec, v, i)).collect();
    let mut entry = vec![0u8; 6];
    entry.extend_from_slice(&[0, 1]); // data reference index
    entry.extend_from_slice(&[0; 16]);
    entry.extend_from_slice(&v.width.to_be_bytes());
    entry.extend_from_slice(&v.height.to_be_bytes());
    entry.extend(be(&[0x0048_0000, 0x0048_0000, 0]));
    entry.extend_from_slice(&[0, 1]);
    entry.extend_from_slice(&[0; 32]);
    entry.extend_from_slice(&[0, 0x18, 0xff, 0xff]);
    let cfg = if &v.fourcc == b"hvc1" || &v.fourcc == b"hev1" {
        b"hvcC"
    } else {
        b"avcC"
    };
    entry.extend(mk(cfg, &v.config));
    entry.extend(mk(b"pasp", &be(&[1, 1])));
    let stsd = mk(b"stsd", &[be(&[0, 1]), mk(&v.fourcc, &entry)].concat());
    let total = v.dur * v.frames as u32;
    let mut tables = vec![stsd, mk(b"stts", &be(&[0, 1, v.frames as u32, v.dur]))];
    if v.ctts {
        let mut b = be(&[0, v.frames as u32]);
        for i in 0..v.frames {
            b.extend(be(&[1, ctts_of(i, v)]));
        }
        tables.push(mk(b"ctts", &b));
    }
    let keys: Vec<u32> = (0..v.frames).step_by(v.gop).map(|i| i as u32 + 1).collect();
    tables.push(mk(
        b"stss",
        &[be(&[0, keys.len() as u32]), be(&keys)].concat(),
    ));
    tables.push(stsc(v.frames));
    tables.push(stsz(&samples));
    let sdtp = v.sdtp.then(|| {
        let mut b = vec![0u8; 4];
        b.extend((0..v.frames).map(|i| if i.is_multiple_of(v.gop) { 0x20 } else { 0x10 }));
        mk(b"sdtp", &b)
    });
    let edts = v.edts.then(|| {
        mk(
            b"edts",
            &mk(b"elst", &be(&[0, 1, total / 30, v.dur, 0x10000])),
        )
    });
    let (w, h, scale) = (v.width, v.height, v.scale);
    TrackData {
        trak_without_stco: Box::new(move |offsets: &[u32]| {
            let mut stbl = tables.concat();
            stbl.extend(stco(offsets));
            if let Some(s) = &sdtp {
                stbl.extend_from_slice(s);
            }
            let minf = [
                mk(b"vmhd", &be(&[1, 0, 0])),
                hdlr(b"alis", "DataHandler"),
                dinf(),
                mk(b"stbl", &stbl),
            ]
            .concat();
            let mdia = [
                mdhd(scale, total),
                hdlr(b"vide", "VideoHandler"),
                mk(b"minf", &minf),
            ]
            .concat();
            let mut trak = tkhd(1, total / 30, w, h, 0);
            if let Some(e) = &edts {
                trak.extend_from_slice(e);
            }
            trak.extend(mk(b"mdia", &mdia));
            mk(b"trak", &trak)
        }),
        samples,
    }
}

fn audio_trak(spec: &Spec, a: &Audio, id: u32) -> TrackData {
    let samples: Vec<Vec<u8>> = (0..a.frames).map(|i| audio_payload(spec, i)).collect();
    let mut entry = vec![0u8; 6];
    entry.extend_from_slice(&[0, 1]);
    entry.extend_from_slice(&[0; 8]);
    entry.extend_from_slice(&[0, 2, 0, 16, 0, 0, 0, 0]);
    entry.extend(be(&[a.scale << 16]));
    let mut dec_cfg = vec![0x40, 0x15, 0, 0x20, 0, 0, 1, 0xf4, 0, 0, 1, 0xf4, 0];
    dec_cfg.extend_from_slice(&[0x05, a.asc.len() as u8]);
    dec_cfg.extend_from_slice(&a.asc);
    let mut es = vec![0, 0, 0, 0x04, dec_cfg.len() as u8];
    es.extend(dec_cfg);
    es.extend_from_slice(&[0x06, 1, 2]);
    let mut esds = vec![0, 0, 0, 0, 0x03, es.len() as u8];
    esds.extend(es);
    entry.extend(mk(b"esds", &esds));
    let stsd = mk(b"stsd", &[be(&[0, 1]), mk(b"mp4a", &entry)].concat());
    let total = 1024 * a.frames as u32;
    let tables = [
        stsd,
        mk(b"stts", &be(&[0, 1, a.frames as u32, 1024])),
        stsc(a.frames),
        stsz(&samples),
    ]
    .concat();
    let scale = a.scale;
    TrackData {
        trak_without_stco: Box::new(move |offsets: &[u32]| {
            let mut stbl = tables.clone();
            stbl.extend(stco(offsets));
            let minf = [
                mk(b"smhd", &be(&[0, 0])),
                hdlr(b"alis", "DataHandler"),
                dinf(),
                mk(b"stbl", &stbl),
            ]
            .concat();
            let mdia = [
                mdhd(scale, total),
                hdlr(b"soun", "SoundHandler"),
                mk(b"minf", &minf),
            ]
            .concat();
            let movie_dur = (u64::from(total) * 1000 / u64::from(scale)) as u32;
            mk(
                b"trak",
                &[tkhd(id, movie_dur, 0, 0, 0x0100), mk(b"mdia", &mdia)].concat(),
            )
        }),
        samples,
    }
}

fn rvmi_trak(records: &[(u32, Vec<u8>)], id: u32) -> TrackData {
    let samples: Vec<Vec<u8>> = records.iter().map(|r| r.1.clone()).collect();
    let mut stts = vec![];
    for i in 0..records.len() {
        let d = records
            .get(i + 1)
            .map_or(0, |n| n.0.wrapping_sub(records[i].0));
        stts.extend(be(&[1, d]));
    }
    let n = records.len() as u32;
    let tables = [
        mk(b"stsd", &[be(&[0, 1]), mk(b"RVMI", &[])].concat()),
        mk(b"stts", &[be(&[0, n]), stts].concat()),
        stsc(records.len()),
        stsz(&samples),
    ]
    .concat();
    let last = records.last().map_or(0, |r| r.0);
    TrackData {
        trak_without_stco: Box::new(move |offsets: &[u32]| {
            let mut stbl = tables.clone();
            stbl.extend(stco(offsets));
            let mdia = [
                mdhd(50, last),
                mk(
                    b"hdlr",
                    &[&[0u8; 8][..], b"data", &[0; 12], b" GPS and G-Sensor"].concat(),
                ),
                mk(b"minf", &mk(b"stbl", &stbl)),
            ]
            .concat();
            mk(
                b"trak",
                &[tkhd(id, last * 20, 0, 0, 0), mk(b"mdia", &mdia)].concat(),
            )
        }),
        samples,
    }
}

pub fn build(spec: &Spec) -> Vec<u8> {
    let mut tracks = vec![];
    let mut duration_ms = 0u32;
    if let Some(v) = &spec.video {
        tracks.push(video_trak(spec, v));
        duration_ms = (u64::from(v.dur) * v.frames as u64 * 1000 / u64::from(v.scale)) as u32;
    }
    if let Some(a) = &spec.audio {
        tracks.push(audio_trak(spec, a, tracks.len() as u32 + 1));
        if spec.video.is_none() {
            duration_ms = (1024 * a.frames as u64 * 1000 / u64::from(a.scale)) as u32;
        }
    }
    if !spec.rvmi.is_empty() {
        tracks.push(rvmi_trak(&spec.rvmi, tracks.len() as u32 + 1));
    }
    // chunk layout: chunks of every track in turn
    let chunked: Vec<Vec<&[Vec<u8>]>> = tracks
        .iter()
        .map(|t| t.samples.chunks(CHUNK).collect())
        .collect();
    let max_chunks = chunked.iter().map(Vec::len).max().unwrap_or(0);
    let header = |offsets: &[Vec<u32>]| {
        let mvhd = [
            be(&[0, 0xd35f_760e, 0xd35f_760e, 1000, duration_ms]),
            be(&[0x0001_0000]),
            vec![1, 0],
            vec![0; 10],
            be(&[0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x4000_0000]),
            vec![0; 24],
            be(&[tracks.len() as u32 + 1]),
        ]
        .concat();
        let mut name = vec![0, spec.model.len() as u8, 0, 0];
        name.extend_from_slice(spec.model.as_bytes());
        let udta = mk(b"udta", &mk(b"\xa9mod", &name));
        let mut moov = mk(b"mvhd", &mvhd);
        moov.extend(udta);
        for (t, o) in tracks.iter().zip(offsets) {
            moov.extend((t.trak_without_stco)(o));
        }
        [mk(b"ftyp", b"qt  \0\0\0\0qt  "), mk(b"moov", &moov)].concat()
    };
    let zero: Vec<Vec<u32>> = chunked.iter().map(|c| vec![0; c.len()]).collect();
    let head_len = header(&zero).len() as u32;
    let mut offsets: Vec<Vec<u32>> = chunked.iter().map(|_| vec![]).collect();
    let mut mdat = vec![];
    for c in 0..max_chunks {
        for (t, chunks) in chunked.iter().enumerate() {
            if let Some(chunk) = chunks.get(c) {
                offsets[t].push(head_len + 8 + mdat.len() as u32);
                for s in chunk.iter() {
                    mdat.extend_from_slice(s);
                }
            }
        }
    }
    let mut file = header(&offsets);
    assert_eq!(file.len() as u32, head_len);
    file.extend(mk(b"mdat", &mdat));
    file
}

pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rv-edit-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn write(dir: &Path, name: &str, spec: &Spec) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, build(spec)).unwrap();
    p
}

/// GPS record as Registrator Viewer writes it.
pub fn g_rev(lat: f64, lon: f64, speed: f64, course: f64, warn: u8) -> Vec<u8> {
    let mut r = b"gReV".to_vec();
    r.extend_from_slice(&((lon * 1e6).round() as i32).to_le_bytes());
    r.extend_from_slice(&((lat * 1e6).round() as i32).to_le_bytes());
    r.extend_from_slice(&0f32.to_le_bytes());
    r.extend_from_slice(&((speed * 10.0).round() as u16).to_le_bytes());
    r.extend_from_slice(&[(course / 2.0).round() as u8, warn, 0x0c]);
    r
}

pub fn s_rev(x: f64, y: f64, z: f64) -> Vec<u8> {
    let mut r = b"sReV".to_vec();
    for v in [x, y, z] {
        r.extend_from_slice(&((v * 1000.0).round() as i16).to_le_bytes());
    }
    r.push(0x0c);
    r
}

pub fn t_rev(tdatetime: f64) -> Vec<u8> {
    let mut r = b"tReV".to_vec();
    r.extend_from_slice(&tdatetime.to_le_bytes());
    r.push(0x0c);
    r
}

/// A box body found by path (e.g. `["moov", "trak", "mdia", "minf", "stbl", "stsd"]`) in the
/// `n`-th match of the last-but-one level.
pub fn find_box(d: &[u8], path: &[&[u8; 4]], nth_trak: usize) -> Option<Vec<u8>> {
    fn children(d: &[u8]) -> Vec<([u8; 4], &[u8])> {
        let mut out = vec![];
        let mut pos = 0;
        while pos + 8 <= d.len() {
            let size = u32::from_be_bytes(d[pos..pos + 4].try_into().unwrap()) as usize;
            if size < 8 || pos + size > d.len() {
                break;
            }
            out.push((
                d[pos + 4..pos + 8].try_into().unwrap(),
                &d[pos + 8..pos + size],
            ));
            pos += size;
        }
        out
    }
    let mut cur = d;
    for (level, kind) in path.iter().enumerate() {
        let matches: Vec<&[u8]> = children(cur)
            .into_iter()
            .filter(|(k, _)| k == *kind)
            .map(|(_, b)| b)
            .collect();
        cur = if **kind == *b"trak" && level > 0 {
            matches.get(nth_trak).copied()?
        } else {
            matches.first().copied()?
        };
    }
    Some(cur.to_vec())
}
