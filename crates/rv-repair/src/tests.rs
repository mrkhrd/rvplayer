//! End-to-end tests: camera-layout movies (`ftyp`, `mdat`, `moov`) with length-prefixed H.264 /
//! H.265 NAL units and AAC-like audio are broken the way a power cut breaks them, repaired with
//! the intact file as reference and read back with `rv_core`.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use rv_core::mp4::{self, Movie, Sample, Track};

use super::*;

/// Deterministic byte source.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 24) as u8
    }

    fn below(&mut self, n: usize) -> usize {
        (usize::from(self.next()) << 8 | usize::from(self.next())) % n
    }

    /// Bytes that never contain 0 (keeps audio free of accidental frame headers).
    fn nonzero(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() | 1).collect()
    }
}

#[derive(Clone, Copy)]
struct Spec {
    hevc: bool,
    /// Frames begin with an access unit delimiter.
    aud: bool,
    slices: usize,
    frames: usize,
    gop: usize,
    audio_per_gap: usize,
    /// Size of a `free` box between `ftyp` and `mdat` (0: none).
    reserve: usize,
    seed: u64,
}

const H264: Spec = Spec {
    hevc: false,
    aud: true,
    slices: 1,
    frames: 40,
    gop: 10,
    audio_per_gap: 2,
    reserve: 0,
    seed: 7,
};

fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(kind);
    v.extend_from_slice(body);
    v
}

fn be(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_be_bytes()).collect()
}

fn nal(header: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut v = ((header.len() + payload.len()) as u32)
        .to_be_bytes()
        .to_vec();
    v.extend_from_slice(header);
    v.extend_from_slice(payload);
    v
}

/// A good movie in camera layout and where its parts are.
struct Built {
    file: Vec<u8>,
    mdat_at: usize,
    moov_at: usize,
}

fn video_frame(spec: &Spec, key: bool, rng: &mut Rng) -> Vec<u8> {
    let mut f = Vec::new();
    let slice = |hdr: &[u8], first: bool, len: usize, rng: &mut Rng| {
        let mut p = vec![if first { 0x88 } else { 0x08 }];
        p.extend((1..len).map(|_| rng.next()));
        nal(hdr, &p)
    };
    let size = if key { 3000 } else { 600 } + rng.below(400);
    if spec.hevc {
        if spec.aud {
            f.extend(nal(&[0x46, 0x01], &[if key { 0x10 } else { 0x30 }]));
        }
        if key {
            f.extend(nal(&[0x40, 0x01], &[0x0c, 0x01, 0xff, 0xff]));
            f.extend(nal(&[0x42, 0x01], &[0x01, 0x01, 0x60, 0x00, 0x90]));
            f.extend(nal(&[0x44, 0x01], &[0xc1, 0x72, 0xb4]));
        }
        let hdr: &[u8] = if key { &[0x26, 0x01] } else { &[0x02, 0x01] };
        for s in 0..spec.slices {
            f.extend(slice(hdr, s == 0, size / spec.slices, rng));
        }
    } else {
        if spec.aud {
            f.extend(nal(&[0x09], &[if key { 0x10 } else { 0x30 }]));
        }
        if key {
            f.extend(nal(&[0x67], &[0x64, 0x00, 0x28, 0xac, 0xd9, 0x40, 0x78]));
            f.extend(nal(&[0x68], &[0xeb, 0xe3, 0xcb, 0x22]));
        }
        let hdr: &[u8] = if key { &[0x65] } else { &[0x41] };
        for s in 0..spec.slices {
            f.extend(slice(hdr, s == 0, size / spec.slices, rng));
        }
    }
    f
}

fn audio_frame(rng: &mut Rng) -> Vec<u8> {
    let mut f = vec![0x21, 0x1a, 0x0f, 0xff];
    let n = 256 + rng.below(80);
    f.extend(rng.nonzero(n));
    f
}

fn build(spec: &Spec) -> Built {
    let mut rng = Rng(spec.seed);
    let ftyp = bx(b"ftyp", b"qt  \0\0\0\0qt  ");
    let reserve = if spec.reserve > 0 {
        bx(b"free", &vec![0; spec.reserve - 8])
    } else {
        Vec::new()
    };
    let mdat_at = ftyp.len() + reserve.len();
    let mut data = Vec::new();
    let mut video = Vec::new();
    let mut audio = Vec::new();
    for i in 0..spec.frames {
        let key = i % spec.gop == 0;
        let f = video_frame(spec, key, &mut rng);
        video.push(((mdat_at + 8 + data.len()) as u32, f.len() as u32, key));
        data.extend(f);
        for _ in 0..spec.audio_per_gap {
            let a = audio_frame(&mut rng);
            audio.push(((mdat_at + 8 + data.len()) as u32, a.len() as u32));
            data.extend(a);
        }
    }
    let mdat = bx(b"mdat", &data);
    let moov = moov(spec, &video, &audio);
    let file = [ftyp, reserve, mdat, moov].concat();
    let moov_at = mdat_at + 8 + data.len();
    Built {
        file,
        mdat_at,
        moov_at,
    }
}

fn full(version: u8, flags: u32) -> Vec<u8> {
    be(&[(u32::from(version) << 24) | flags])
}

fn moov(spec: &Spec, video: &[(u32, u32, bool)], audio: &[(u32, u32)]) -> Vec<u8> {
    let tables = |offsets: &[u32], sizes: &[u32], delta: u32| {
        let n = offsets.len() as u32;
        [
            bx(b"stts", &[full(0, 0), be(&[1, n, delta])].concat()),
            bx(b"stsc", &[full(0, 0), be(&[1, 1, 1, 1])].concat()),
            bx(b"stsz", &[full(0, 0), be(&[0, n]), be(sizes)].concat()),
            bx(b"stco", &[full(0, 0), be(&[n]), be(offsets)].concat()),
        ]
        .concat()
    };
    let dinf = bx(
        b"dinf",
        &bx(
            b"dref",
            &[full(0, 0), be(&[1]), bx(b"url ", &full(0, 1))].concat(),
        ),
    );
    let tkhd = |id: u32, duration: u32, w: u32, h: u32| {
        let mut b = full(0, 7);
        b.extend(be(&[0, 0, id, 0, duration]));
        b.resize(76, 0);
        b.extend(be(&[w << 16, h << 16]));
        bx(b"tkhd", &b)
    };
    let mdhd = |scale: u32, duration: u32| {
        bx(
            b"mdhd",
            &[full(0, 0), be(&[0, 0, scale, duration, 0x55c4_0000])].concat(),
        )
    };
    let hdlr = |kind: &[u8; 4]| {
        bx(
            b"hdlr",
            &[full(0, 0), be(&[0]), kind.to_vec(), vec![0; 13]].concat(),
        )
    };
    let elst = bx(
        b"edts",
        &bx(b"elst", &[full(0, 0), be(&[1, 0, 0, 0x0001_0000])].concat()),
    );

    // video
    let n = video.len() as u32;
    let mut entry = vec![0u8; 6];
    entry.extend_from_slice(&[0, 1]);
    entry.extend(vec![0u8; 16]);
    entry.extend_from_slice(&[0x07, 0x80, 0x04, 0x38]); // 1920 x 1080
    entry.extend(be(&[0x0048_0000, 0x0048_0000, 0]));
    entry.extend_from_slice(&[0, 1]);
    entry.extend(vec![0u8; 32]);
    entry.extend_from_slice(&[0, 0x18, 0xff, 0xff]);
    let (codec, config) = if spec.hevc {
        let mut c = vec![
            1, 1, 0x60, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 0x5d, 0xf0, 0, 0xfc, 0xfd, 0xf8, 0xf8, 0, 0,
            0x0f, 0,
        ];
        c[21] = 0x0f; // lengthSizeMinusOne = 3
        (*b"hvc1", bx(b"hvcC", &c))
    } else {
        let c = [
            1, 0x64, 0, 0x28, 0xff, 0xe1, 0, 8, 0x67, 0x64, 0, 0x28, 0xac, 0xd9, 0x40, 0x78, 1, 0,
            5, 0x68, 0xeb, 0xe3, 0xcb, 0x22,
        ];
        (*b"avc1", bx(b"avcC", &c))
    };
    entry.extend(config);
    let stsd = bx(
        b"stsd",
        &[full(0, 0), be(&[1]), bx(&codec, &entry)].concat(),
    );
    let keys: Vec<u32> = (1..)
        .zip(video)
        .filter(|(_, v)| v.2)
        .map(|(i, _)| i)
        .collect();
    let stss = bx(
        b"stss",
        &[full(0, 0), be(&[keys.len() as u32]), be(&keys)].concat(),
    );
    let offsets: Vec<u32> = video.iter().map(|v| v.0).collect();
    let sizes: Vec<u32> = video.iter().map(|v| v.1).collect();
    let stbl = bx(
        b"stbl",
        &[stsd, tables(&offsets, &sizes, 1001), stss].concat(),
    );
    let vmhd = bx(b"vmhd", &[full(0, 1), vec![0; 8]].concat());
    let minf = bx(b"minf", &[vmhd, dinf.clone(), stbl].concat());
    let vdur = n * 1001;
    let mdia = bx(b"mdia", &[mdhd(30000, vdur), hdlr(b"vide"), minf].concat());
    let vtrak = bx(
        b"trak",
        &[tkhd(1, vdur / 30, 1920, 1080), elst.clone(), mdia].concat(),
    );

    // audio
    let mut entry = vec![0u8; 6];
    entry.extend_from_slice(&[0, 1]);
    entry.extend(be(&[0, 0]));
    entry.extend_from_slice(&[0, 1, 0, 16, 0, 0, 0, 0]);
    entry.extend(be(&[48000 << 16]));
    entry.extend(bx(
        b"esds",
        &[
            full(0, 0),
            vec![
                3, 0x19, 0, 2, 0, 4, 0x11, 0x40, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 2, 0x11,
                0x88, 6, 1, 2,
            ],
        ]
        .concat(),
    ));
    let stsd = bx(
        b"stsd",
        &[full(0, 0), be(&[1]), bx(b"mp4a", &entry)].concat(),
    );
    let offsets: Vec<u32> = audio.iter().map(|a| a.0).collect();
    let sizes: Vec<u32> = audio.iter().map(|a| a.1).collect();
    let stbl = bx(b"stbl", &[stsd, tables(&offsets, &sizes, 1024)].concat());
    let smhd = bx(b"smhd", &[full(0, 0), vec![0; 4]].concat());
    let minf = bx(b"minf", &[smhd, dinf, stbl].concat());
    let adur = audio.len() as u32 * 1024;
    let mdia = bx(b"mdia", &[mdhd(48000, adur), hdlr(b"soun"), minf].concat());
    let atrak = bx(b"trak", &[tkhd(2, adur / 48, 0, 0), mdia].concat());

    let mut mvhd = full(0, 0);
    mvhd.extend(be(&[0, 0, 1000, vdur / 30, 0x0001_0000]));
    mvhd.extend_from_slice(&[1, 0]);
    mvhd.resize(96, 0);
    mvhd.extend(be(&[3]));
    let udta = bx(b"udta", &bx(b"\xa9inf", b"TEST CAMERA\0"));
    // a box the original does not copy
    let iods = bx(
        b"iods",
        &[
            full(0, 0),
            vec![0x10, 7, 0, 0x4f, 0xff, 0xff, 0xff, 0xff, 0xff],
        ]
        .concat(),
    );
    bx(
        b"moov",
        &[bx(b"mvhd", &mvhd), iods, udta, vtrak, atrak].concat(),
    )
}

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rv-repair-{}-{name}", std::process::id()))
}

fn read(data: &[u8]) -> Movie {
    mp4::read_movie(&mut Cursor::new(data)).expect("readable movie")
}

fn track<'m>(m: &'m Movie, handler: &[u8; 4]) -> &'m Track {
    m.track_by_handler(handler).expect("track")
}

fn layout(s: &[Sample]) -> Vec<(u64, u32, bool)> {
    s.iter().map(|s| (s.offset, s.size, s.sync)).collect()
}

/// Writes `broken` and `reference` to disk, repairs, returns the repaired bytes.
fn repaired(name: &str, broken: &[u8], reference: &[u8]) -> (Summary, Vec<u8>) {
    let b = temp(&format!("{name}-broken.mov"));
    let r = temp(&format!("{name}-ref.mov"));
    fs::write(&b, broken).unwrap();
    fs::write(&r, reference).unwrap();
    let summary = repair(&b, &r).expect("repair");
    let out = fs::read(&b).unwrap();
    let _ = fs::remove_file(&b);
    let _ = fs::remove_file(&r);
    (summary, out)
}

/// Cuts the `moov` off and zeroes the mdat size, as a camera leaves a file after a power cut.
fn power_cut(good: &Built, at: usize) -> Vec<u8> {
    let mut f = good.file[..at.min(good.moov_at)].to_vec();
    f[good.mdat_at..good.mdat_at + 4].copy_from_slice(&[0; 4]);
    f
}

/// Checks the repaired movie against the good one: codec configuration, the first `video_frames`
/// video frames and the audio frames before video frame `audio_until`, with the same timing.
fn assert_same_samples(good: &Movie, fixed: &Movie, video_frames: usize, audio_until: usize) {
    let (gv, fv) = (track(good, b"vide"), track(fixed, b"vide"));
    assert_eq!(fv.codec, gv.codec);
    assert_eq!(fv.codec_config, gv.codec_config);
    assert_eq!(
        (fv.timescale, fv.width, fv.height),
        (gv.timescale, gv.width, gv.height)
    );
    let gs = gv.samples();
    let fs = fv.samples();
    assert_eq!(layout(&fs), layout(&gs[..video_frames]));
    let times = |s: &[Sample]| s.iter().map(|s| s.time).collect::<Vec<_>>();
    assert_eq!(times(&fs), times(&gs[..video_frames]));
    assert_eq!(fv.duration, video_frames as u64 * 1001);

    let until = gs[audio_until].offset;
    let ga: Vec<Sample> = track(good, b"soun")
        .samples()
        .into_iter()
        .filter(|s| s.offset < until)
        .collect();
    let fa = track(fixed, b"soun").samples();
    assert_eq!(layout(&fa), layout(&ga));
    assert_eq!(times(&fa), times(&ga));
    // tracks that were not rebuilt and boxes the original does not copy are gone
    assert_eq!(fixed.tracks.len(), 2);
    assert_eq!(fixed.camera_model().as_deref(), Some("TEST CAMERA"));
}

#[test]
fn repairs_camera_file_without_moov() {
    let good = build(&H264);
    let broken = power_cut(&good, usize::MAX);
    let (summary, out) = repaired("nomoov", &broken, &good.file);
    assert_eq!(summary.video_frames, 40);
    assert_eq!(summary.audio_frames, 78); // the audio after the last video frame is not recovered
    assert_eq!(summary.header_offset, Some(broken.len() as u64));
    assert_eq!(
        &out[..broken.len()][good.mdat_at + 4..],
        &broken[good.mdat_at + 4..]
    );
    let size = u32::from_be_bytes(out[good.mdat_at..good.mdat_at + 4].try_into().unwrap());
    assert_eq!(size as usize, broken.len() - good.mdat_at);
    let fixed = read(&out);
    // the audio after the last video frame is not recovered (as the original)
    assert_same_samples(&read(&good.file), &fixed, 40, 39);
    assert!(!out.windows(4).any(|w| w == b"iods"));
}

#[test]
fn repairs_file_cut_in_the_middle_of_a_frame() {
    let good = build(&H264);
    let movie = read(&good.file);
    let frame = track(&movie, b"vide").samples()[25];
    let cut = frame.offset as usize + frame.size as usize / 2;
    let broken = power_cut(&good, cut);
    let (summary, out) = repaired("cut", &broken, &good.file);
    assert_eq!(summary.video_frames, 25);
    // the audio before the cut frame is kept as well
    assert_same_samples(&movie, &read(&out), 25, 25);
}

#[test]
fn stale_mdat_size_is_extended_over_the_found_frames() {
    let good = build(&H264);
    let mut broken = good.file[..good.moov_at].to_vec();
    let stale = (good.moov_at - good.mdat_at) as u32 / 2;
    broken[good.mdat_at..good.mdat_at + 4].copy_from_slice(&stale.to_be_bytes());
    let (_, out) = repaired("stale", &broken, &good.file);
    assert_same_samples(&read(&good.file), &read(&out), 40, 39);
}

#[test]
fn header_goes_into_reserved_space() {
    let spec = Spec {
        reserve: 64 << 10,
        ..H264
    };
    let good = build(&spec);
    let broken = power_cut(&good, usize::MAX);
    let (summary, out) = repaired("reserve", &broken, &good.file);
    let at = good.mdat_at - (64 << 10);
    assert_eq!(summary.header_offset, Some(at as u64));
    assert_eq!(out.len(), broken.len()); // the file ends with the media data
    assert_eq!(&out[good.mdat_at + 4..], &broken[good.mdat_at + 4..]);
    let moov_len = u32::from_be_bytes(out[at..at + 4].try_into().unwrap()) as usize;
    assert_eq!(&out[at + 4..at + 8], b"moov");
    let free =
        u32::from_be_bytes(out[at + moov_len..at + moov_len + 4].try_into().unwrap()) as usize;
    assert_eq!(&out[at + moov_len + 4..at + moov_len + 8], b"free");
    assert_eq!(at + moov_len + free, good.mdat_at);
    assert_same_samples(&read(&good.file), &read(&out), 40, 39);
}

#[test]
fn rebuilds_file_whose_header_is_gone() {
    let good = build(&H264);
    let mut broken = power_cut(&good, usize::MAX);
    broken[..good.mdat_at + 8].fill(0); // ftyp and mdat header lost
    let (summary, out) = repaired("raw", &broken, &good.file);
    assert_eq!(summary.video_frames, 40);
    assert_eq!(&out[..16], b"\0\0\0\x10ftypqt  \0\0\0\0");
    assert_eq!(&out[20..24], b"mdat");
    assert_eq!(
        u32::from_be_bytes(out[16..20].try_into().unwrap()) as usize,
        broken.len() - 16
    );
    assert_same_samples(&read(&good.file), &read(&out), 40, 39);
}

#[test]
fn shorter_reference_repeats_its_gop() {
    let good = build(&H264);
    let reference = build(&Spec {
        frames: 12,
        seed: 99,
        ..H264
    });
    let broken = power_cut(&good, usize::MAX);
    let (_, out) = repaired("shortref", &broken, &reference.file);
    assert_same_samples(&read(&good.file), &read(&out), 40, 39);
}

#[test]
fn hevc_without_delimiters() {
    let spec = Spec {
        hevc: true,
        aud: false,
        ..H264
    };
    let good = build(&spec);
    let broken = power_cut(&good, usize::MAX);
    let (summary, out) = repaired("hevc", &broken, &good.file);
    assert_eq!(summary.video_frames, 40);
    assert_same_samples(&read(&good.file), &read(&out), 40, 39);
}

#[test]
fn hevc_with_delimiters() {
    let spec = Spec { hevc: true, ..H264 };
    let good = build(&spec);
    let (_, out) = repaired("hevcaud", &power_cut(&good, usize::MAX), &good.file);
    assert_same_samples(&read(&good.file), &read(&out), 40, 39);
}

#[test]
fn h264_multi_slice_without_delimiters() {
    let spec = Spec {
        aud: false,
        slices: 3,
        ..H264
    };
    let good = build(&spec);
    let (summary, out) = repaired("slices", &power_cut(&good, usize::MAX), &good.file);
    assert_eq!(summary.video_frames, 40);
    assert_same_samples(&read(&good.file), &read(&out), 40, 39);
}

#[test]
fn too_few_frames_leave_the_file_untouched() {
    let good = build(&H264);
    let second = track(&read(&good.file), b"vide").samples()[1];
    let broken = power_cut(&good, second.offset as usize + 10);
    let (summary, out) = repaired("few", &broken, &good.file);
    assert_eq!(summary, Summary::default());
    assert_eq!(out, broken);
}

#[test]
fn unusable_reference_is_reported() {
    let good = build(&H264);
    let broken = power_cut(&good, usize::MAX);
    let b = temp("badref-broken.mov");
    let r = temp("badref-ref.mov");
    fs::write(&b, &broken).unwrap();
    fs::write(&r, &broken).unwrap(); // no moov
    assert!(matches!(repair(&b, &r), Err(Error::Reference(_))));
    assert_eq!(fs::read(&b).unwrap(), broken);
    let _ = fs::remove_file(&b);
    let _ = fs::remove_file(&r);
}

#[test]
fn cancelled_repair_leaves_the_file_untouched() {
    let good = build(&H264);
    let broken = power_cut(&good, usize::MAX);
    let b = temp("cancel-broken.mov");
    let r = temp("cancel-ref.mov");
    fs::write(&b, &broken).unwrap();
    fs::write(&r, &good.file).unwrap();
    let shared = Shared::default();
    shared.cancel.store(true, Ordering::Relaxed);
    assert_eq!(run(&b, &r, &shared), Ok(None));
    assert_eq!(fs::read(&b).unwrap(), broken);
    let _ = fs::remove_file(&b);
    let _ = fs::remove_file(&r);
}

#[test]
fn background_repair_reports_progress_and_result() {
    let good = build(&H264);
    let broken = power_cut(&good, usize::MAX);
    let b = temp("thread-broken.mov");
    let r = temp("thread-ref.mov");
    fs::write(&b, &broken).unwrap();
    fs::write(&r, &good.file).unwrap();
    let status = Repair::start(b.clone(), r.clone()).unwrap().wait();
    assert_eq!(
        status,
        Status {
            percent: 100,
            frames: 118,
            finished: true,
            cancelled: false,
            error: None
        }
    );
    read(&fs::read(&b).unwrap());
    // a failing repair finishes with an error and no frames
    let status = Repair::start(b.clone(), temp("missing.mov"))
        .unwrap()
        .wait();
    assert!(status.finished && status.frames == 0);
    assert!(matches!(status.error, Some(Error::Reference(_))));
    let _ = fs::remove_file(&b);
    let _ = fs::remove_file(&r);
}

#[test]
fn suspicious_sizes_are_exact_multiples_of_32_mib() {
    assert!(is_suspicious_size(32 << 20));
    assert!(is_suspicious_size(3 * (32 << 20)));
    assert!(!is_suspicious_size(0));
    assert!(!is_suspicious_size((32 << 20) + 1));
    assert!(!is_suspicious_size(16 << 20));
}
