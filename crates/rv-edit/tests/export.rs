//! Export round trips on synthetic movies, re-read with rv-core.

mod common;

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
use common::*;
use rv_core::FileInfo;
use rv_core::mp4::{Movie, Sample, read_movie};
use rv_edit::{CropItem, ExportError, ExportOptions, ProblemKind, Progress, Stage, export};

fn item(path: &Path, p1: f64, p2: f64) -> CropItem {
    CropItem {
        path: path.to_path_buf(),
        p1,
        p2,
        speed: 1.0,
        speed_up_fps: true,
        start: None,
        track: None,
    }
}

fn opts(meta: bool) -> ExportOptions {
    ExportOptions {
        meta,
        ..ExportOptions::default()
    }
}

fn run(items: &[CropItem], o: &ExportOptions, out: &Path) -> rv_edit::ExportReport {
    export(items, o, out, &mut |_| true).expect("export")
}

fn movie(path: &Path) -> Movie {
    read_movie(&mut BufReader::new(File::open(path).unwrap())).unwrap()
}

fn payloads(path: &Path, samples: &[Sample]) -> Vec<Vec<u8>> {
    let mut f = File::open(path).unwrap();
    samples
        .iter()
        .map(|s| {
            let mut b = vec![0; s.size as usize];
            f.seek(SeekFrom::Start(s.offset)).unwrap();
            f.read_exact(&mut b).unwrap();
            b
        })
        .collect()
}

fn idents(path: &Path, m: &Movie, handler: &[u8; 4]) -> Vec<(u8, u32)> {
    let t = m.track_by_handler(handler).unwrap();
    payloads(path, &t.samples())
        .iter()
        .map(|p| ident(p, handler == b"vide"))
        .collect()
}

/// Sum of the `stts` durations of the `trak`-th track.
fn stts_total(d: &[u8], trak: usize) -> i64 {
    let stts = find_box(
        d,
        &[b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stts"],
        trak,
    )
    .unwrap();
    let n = u32::from_be_bytes(stts[4..8].try_into().unwrap()) as usize;
    (0..n)
        .map(|i| {
            let c = u32::from_be_bytes(stts[8 + 8 * i..12 + 8 * i].try_into().unwrap());
            let d = u32::from_be_bytes(stts[12 + 8 * i..16 + 8 * i].try_into().unwrap()) as i32;
            i64::from(c) * i64::from(d)
        })
        .sum()
}

fn start_time() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2016, 5, 16)
        .unwrap()
        .and_hms_opt(13, 6, 26)
        .unwrap()
}

fn tdatetime(t: NaiveDateTime) -> f64 {
    let base = NaiveDate::from_ymd_opt(1899, 12, 30)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    (t - base).num_milliseconds() as f64 / 86_400_000.0
}

/// A 10 s file with an RVMI track: GPS every second (lat = 60 + i/1000), G-sensor every 0.1 s.
fn spec_with_gps(tag: u8, start: NaiveDateTime) -> Spec {
    let mut s = spec(tag);
    let mut recs = vec![(0u32, t_rev(tdatetime(start)))];
    for k in 0..100u32 {
        if k % 10 == 0 {
            let i = f64::from(k / 10);
            recs.push((k * 5, g_rev(60.0 + i / 1000.0, 30.0, 50.0 + i, 90.0, 0)));
        }
        recs.push((k * 5, s_rev(0.0, 0.0, f64::from(k) / 100.0)));
    }
    s.rvmi = recs;
    s
}

#[test]
fn cuts_a_range_starting_at_a_key_frame() {
    let dir = temp_dir("cut");
    let src = write(&dir, "a.mov", &spec(1));
    let out = dir.join("out.mov");
    let report = run(&[item(&src, 0.35, 0.62)], &opts(false), &out);
    assert!(report.problems.is_empty());

    let m = movie(&out);
    let v = m.track_by_handler(b"vide").unwrap();
    let a = m.track_by_handler(b"soun").unwrap();
    // 3.5 s snaps back to the key frame at 3.0 s (frame 90); 6.2 s ends with frame 186, the
    // audio span (frames 140..=290) is shorter, so video gives up its last 360 ticks.
    let vs = v.samples();
    assert_eq!(vs.len(), 97);
    assert!(vs[0].sync);
    let vid = idents(&out, &m, b"vide");
    assert_eq!(vid[0], (1, 90));
    assert_eq!(*vid.last().unwrap(), (1, 186));
    let aud = idents(&out, &m, b"soun");
    assert_eq!(
        (aud.len(), aud[0], *aud.last().unwrap()),
        (151, (1, 140), (1, 290))
    );
    let d = std::fs::read(&out).unwrap();
    assert_eq!(stts_total(&d, 0), 96 * 1000 + 640);
    assert_eq!(stts_total(&d, 1), 151 * 1024);
    assert!((v.duration_secs() - 3.221333).abs() < 1e-6);
    assert!((a.duration_secs() - v.duration_secs()).abs() < 1e-6);
    assert!((report.duration - 3.221).abs() < 1e-9);
    assert!((m.duration_secs() - 3.221333).abs() < 1e-3);

    // sample descriptions, user data and composition offsets carried over
    let s = std::fs::read(&src).unwrap();
    let stsd = [b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsd"];
    for trak in 0..2 {
        assert_eq!(find_box(&s, &stsd, trak), find_box(&d, &stsd, trak));
    }
    assert_eq!(v.codec_config.as_deref(), Some(&AVCC[..]));
    assert_eq!(m.camera_model().as_deref(), Some("TESTCAM X1"));
    let src_movie = movie(&src);
    let src_v = src_movie.track_by_handler(b"vide").unwrap().samples();
    for (k, s) in vs.iter().enumerate() {
        let want = src_v[90 + k].time - src_v[90].time;
        assert!(
            (s.time - vs[0].time - want).abs() < 1e-9,
            "presentation time of frame {k}"
        );
        assert_eq!(s.sync, (90 + k) % 30 == 0);
    }
    // byte-exact samples
    assert_eq!(payloads(&out, &vs), payloads(&src, &src_v[90..=186]));
    // sdtp and edts regenerated
    let sdtp = find_box(
        &d,
        &[b"moov", b"trak", b"mdia", b"minf", b"stbl", b"sdtp"],
        0,
    )
    .unwrap();
    assert_eq!(sdtp.len(), 4 + 97);
    assert_eq!((sdtp[4], sdtp[5]), (0x20, 0x10));
    let elst = find_box(&d, &[b"moov", b"trak", b"edts", b"elst"], 0).unwrap();
    assert_eq!(&elst[4..8], &1u32.to_be_bytes());
    assert_eq!(
        &elst[12..16],
        &1000u32.to_be_bytes(),
        "media time = first frame duration"
    );
    // no RVMI track without meta
    assert!(m.track_by_codec(b"RVMI").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn joins_two_items() {
    let dir = temp_dir("join");
    let a = write(&dir, "a.mov", &spec(1));
    let b = write(&dir, "b.mov", &spec(2));
    let out = dir.join("out.mov");
    let report = run(
        &[item(&a, 0.5, 1.0), item(&b, 0.0, 0.3)],
        &opts(false),
        &out,
    );
    assert!(report.problems.is_empty());
    assert_eq!(report.items.len(), 2);
    let (s0, s1) = (report.items[0], report.items[1]);
    assert_eq!(s0.start, 0.0);
    assert_eq!(s0.end, s1.start);
    assert!((s1.end - report.duration).abs() < 1e-9);

    let m = movie(&out);
    let vid = idents(&out, &m, b"vide");
    let first_b = vid.iter().position(|&(t, _)| t == 2).unwrap();
    assert_eq!(vid[0], (1, 150));
    assert_eq!(vid[first_b - 1], (1, 299));
    assert_eq!(vid[first_b], (2, 0));
    assert!(vid[first_b..].iter().all(|&(t, _)| t == 2));
    let v = m.track_by_handler(b"vide").unwrap().samples();
    assert!(v[first_b].sync);
    // the second item starts where the first one ended (rounded to ms)
    assert!((v[first_b].time - v[0].time - s1.start).abs() < 0.0006);
    let aud = idents(&out, &m, b"soun");
    assert!(aud.iter().any(|&(t, _)| t == 2));
    let mv = m.track_by_handler(b"vide").unwrap().duration_secs();
    assert!((mv - report.duration).abs() < 0.001);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn speed_4_key_frames_only() {
    let dir = temp_dir("key4");
    let src = write(&dir, "a.mov", &spec(1));
    let out = dir.join("out.mov");
    let mut it = item(&src, 0.0, 1.0);
    it.speed = 4.0;
    it.speed_up_fps = false;
    run(&[it], &opts(false), &out);
    let m = movie(&out);
    let v = m.track_by_handler(b"vide").unwrap();
    let vs = v.samples();
    assert_eq!(vs.len(), 10, "one frame per GOP");
    assert!(vs.iter().all(|s| s.sync));
    let ids: Vec<u32> = idents(&out, &m, b"vide").iter().map(|x| x.1).collect();
    assert_eq!(ids, (0..10).map(|i| i * 30).collect::<Vec<_>>());
    // each kept frame lasts its GOP / 4; the last also takes the audio overhang (160 ticks)
    let d = std::fs::read(&out).unwrap();
    assert_eq!(stts_total(&d, 0), 10 * 7500 + 160);
    // audio decimated: every 4th frame
    let aud: Vec<u32> = idents(&out, &m, b"soun").iter().map(|x| x.1).collect();
    assert_eq!(aud.len(), 468 / 4 + 1);
    assert_eq!(&aud[..3], &[0, 4, 8]);
    assert!((v.duration_secs() - 2.505333).abs() < 1e-5);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn speed_3_key_frames_keeps_p_frames_between_b_frames() {
    let dir = temp_dir("key3b");
    let mut s = spec(1);
    if let Some(v) = &mut s.video {
        v.gop = 15;
        v.aud = b"BBP".to_vec();
    }
    let src = write(&dir, "a.mov", &s);
    let out = dir.join("out.mov");
    let mut it = item(&src, 0.0, 1.0);
    it.speed = 3.0;
    it.speed_up_fps = false;
    run(&[it], &opts(false), &out);
    let m = movie(&out);
    let ids: Vec<u32> = idents(&out, &m, b"vide").iter().map(|x| x.1).collect();
    // GOP "I BBP BBP BBP BBP BB": the I frame and every P frame, three frames apart
    assert_eq!(&ids[..7], &[0, 3, 6, 9, 12, 15, 18]);
    assert_eq!(ids.len(), 100);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn speed_4_all_frames() {
    let dir = temp_dir("fps4");
    let src = write(&dir, "a.mov", &spec(1));
    let out = dir.join("out.mov");
    let mut it = item(&src, 0.0, 1.0);
    it.speed = 4.0;
    it.speed_up_fps = true;
    run(&[it], &opts(false), &out);
    let m = movie(&out);
    let v = m.track_by_handler(b"vide").unwrap();
    assert_eq!(v.samples().len(), 300);
    let d = std::fs::read(&out).unwrap();
    assert_eq!(stts_total(&d, 0), 300 * 250 + 160);
    // composition offsets scaled with the frame durations
    let ctts = find_box(
        &d,
        &[b"moov", b"trak", b"mdia", b"minf", b"stbl", b"ctts"],
        0,
    )
    .unwrap();
    let second = u32::from_be_bytes(ctts[20..24].try_into().unwrap());
    assert_eq!(second, ctts_of(1, &video()) / 4);
    assert_eq!(idents(&out, &m, b"soun").len(), 118);
    assert!((v.duration_secs() - 2.505333).abs() < 1e-5);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn speed_half_repeats_audio() {
    let dir = temp_dir("half");
    let src = write(&dir, "a.mov", &spec(1));
    let out = dir.join("out.mov");
    let mut it = item(&src, 0.0, 1.0);
    it.speed = 0.5;
    run(&[it], &opts(false), &out);
    let m = movie(&out);
    let v = m.track_by_handler(b"vide").unwrap();
    assert_eq!(v.samples().len(), 300);
    let d = std::fs::read(&out).unwrap();
    assert_eq!(stts_total(&d, 0), 300 * 2000 + 160);
    let aud: Vec<u32> = idents(&out, &m, b"soun").iter().map(|x| x.1).collect();
    assert_eq!(aud.len(), 468 * 2 + 1);
    assert_eq!(&aud[..5], &[0, 0, 1, 1, 2]);
    assert!((v.duration_secs() - 20.005333).abs() < 1e-5);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cancellation_removes_the_output() {
    let dir = temp_dir("cancel");
    let src = write(&dir, "a.mov", &spec(1));
    let out = dir.join("out.mov");
    let mut stages = vec![];
    let r = export(
        &[item(&src, 0.0, 1.0)],
        &opts(true),
        &out,
        &mut |p: &Progress| {
            stages.push(p.stage);
            !(p.stage == Stage::Copying && p.fraction > 0.0)
        },
    );
    assert!(matches!(r, Err(ExportError::Cancelled)));
    assert!(!out.exists(), "partial output removed");
    for s in [
        Stage::Opening,
        Stage::Meta1,
        Stage::Meta2,
        Stage::Meta3,
        Stage::Copying,
    ] {
        assert!(stages.contains(&s), "{s:?} reported");
    }
    // an existing file is replaced, and removed if the export is cancelled while copying
    std::fs::write(&out, b"old").unwrap();
    let r = export(
        &[item(&src, 0.0, 1.0)],
        &opts(false),
        &out,
        &mut |p: &Progress| p.stage != Stage::Copying || p.fraction == 0.0,
    );
    assert!(matches!(r, Err(ExportError::Cancelled)));
    assert!(!out.exists());
    // cancelling before copying does not touch the output
    std::fs::write(&out, b"old").unwrap();
    let r = export(&[item(&src, 0.0, 1.0)], &opts(false), &out, &mut |_| false);
    assert!(matches!(r, Err(ExportError::Cancelled)));
    assert_eq!(std::fs::read(&out).unwrap(), b"old");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn different_format_and_unreadable_items_are_skipped() {
    let dir = temp_dir("format");
    let a = write(&dir, "a.mov", &spec(1));
    let mut other_pps = spec(2);
    if let Some(v) = &mut other_pps.video {
        let n = v.config.len();
        v.config[n - 1] ^= 0xff;
    }
    let b = write(&dir, "b.mov", &other_pps);
    let mut other_size = spec(3);
    if let Some(v) = &mut other_size.video {
        v.width = 1280;
    }
    let c = write(&dir, "c.mov", &other_size);
    let mut other_sps = spec(4);
    if let Some(v) = &mut other_sps.video {
        v.config[10] ^= 0xff; // an SPS byte: still joined (only the PPS has to match)
    }
    let d = write(&dir, "d.mov", &other_sps);
    let missing = dir.join("missing.mov");
    let garbage = dir.join("garbage.mov");
    std::fs::write(&garbage, b"not a movie at all").unwrap();
    let out = dir.join("out.mov");
    let items = [
        item(&a, 0.0, 0.5),
        item(&b, 0.0, 0.5),
        item(&c, 0.0, 0.5),
        item(&missing, 0.0, 0.5),
        item(&garbage, 0.0, 0.5),
        item(&d, 0.0, 0.5),
    ];
    let report = run(&items, &opts(false), &out);
    let kinds: Vec<(usize, ProblemKind)> =
        report.problems.iter().map(|p| (p.item, p.kind)).collect();
    assert_eq!(
        kinds,
        [
            (1, ProblemKind::DifferentFormat),
            (2, ProblemKind::DifferentFormat),
            (3, ProblemKind::CannotOpen),
            (4, ProblemKind::CannotOpen),
        ]
    );
    assert_eq!(
        report.problems[0].message(),
        format!("Different format {}", b.display())
    );
    assert!(
        report.problems[2]
            .message()
            .starts_with("Cannot open file ")
    );
    let m = movie(&out);
    let tags: Vec<u8> = idents(&out, &m, b"vide").iter().map(|x| x.0).collect();
    assert!(tags.contains(&1) && tags.contains(&4));
    assert!(!tags.contains(&2) && !tags.contains(&3));
    assert_eq!(
        report.items.iter().map(|s| s.item).collect::<Vec<_>>(),
        [0, 5]
    );

    // nothing usable at all: no file
    let none = dir.join("none.mov");
    let r = export(
        &[item(&missing, 0.0, 1.0)],
        &opts(false),
        &none,
        &mut |_| true,
    );
    assert!(matches!(r, Err(ExportError::NothingToExport { ref problems }) if problems.len() == 1));
    assert!(!none.exists());
    // writing over a source is refused
    let r = export(&[item(&a, 0.0, 1.0)], &opts(false), &a, &mut |_| true);
    assert!(matches!(r, Err(ExportError::OutputIsInput(_))));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn missing_audio_is_filled_with_a_repeated_frame() {
    let dir = temp_dir("noaudio");
    let a = write(&dir, "a.mov", &spec(1));
    let mut silent = spec(2);
    silent.audio = None;
    let b = write(&dir, "b.mov", &silent);
    let out = dir.join("out.mov");
    let report = run(
        &[item(&a, 0.0, 0.5), item(&b, 0.0, 0.5)],
        &opts(false),
        &out,
    );
    assert!(report.problems.is_empty());
    let m = movie(&out);
    let aud = idents(&out, &m, b"soun");
    // item 0: audio frames 0..=last in order; item 1: item 0's next-to-last frame repeated
    let last = (0..aud.len())
        .take_while(|&i| aud[i] == (1, i as u32))
        .last()
        .unwrap();
    assert!(last > 200);
    assert!(aud.len() > last + 200);
    assert!(aud[last + 1..].iter().all(|x| *x == (1, last as u32 - 1)));
    let a_dur = m.track_by_handler(b"soun").unwrap().duration_secs();
    let v_dur = m.track_by_handler(b"vide").unwrap().duration_secs();
    assert!((a_dur - v_dur).abs() < 0.03, "{a_dur} vs {v_dur}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn hevc_video_only() {
    let dir = temp_dir("hevc");
    let mut s = spec(1);
    if let Some(v) = &mut s.video {
        v.fourcc = *b"hvc1";
        v.config = vec![
            1, 1, 0x60, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 0x5d, 0xf0, 0, 0xfc, 0xfd, 0xf8, 0xf8, 0, 0,
            0x0f, 0,
        ];
        v.ctts = false;
        v.sdtp = false;
        v.edts = false;
    }
    let src = write(&dir, "a.mp4", &s);
    let out = dir.join("out.mp4");
    let o = ExportOptions {
        audio: false,
        meta: false,
        ..ExportOptions::default()
    };
    run(&[item(&src, 0.1, 0.4)], &o, &out);
    let m = movie(&out);
    assert_eq!(m.tracks.len(), 1);
    let v = m.track_by_handler(b"vide").unwrap();
    assert_eq!(&v.codec, b"hvc1");
    assert_eq!(v.codec_config, movie(&src).tracks[0].codec_config);
    let vs = v.samples();
    assert!(vs[0].sync);
    assert_eq!(idents(&out, &m, b"vide")[0], (1, 30));
    // 1 s .. 4 s (frame 120 included), nothing to align with
    assert_eq!(vs.len(), 91);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn meta_track_round_trips() {
    let dir = temp_dir("meta");
    let start = start_time();
    let src = write(&dir, "a.mov", &spec_with_gps(1, start));
    let out = dir.join("out.mov");
    let report = run(&[item(&src, 0.35, 0.62)], &opts(true), &out);
    assert_eq!(report.t_min, Some(start + TimeDelta::milliseconds(3500)));
    assert_eq!(report.t_max, Some(start + TimeDelta::milliseconds(6200)));

    let info = FileInfo::load(&out);
    assert!(info.error.is_none(), "{:?}", info.error);
    assert_eq!(info.gps_format.as_deref(), Some("movRegV"));
    // the track starts at the first frame actually written (the key frame at 3 s)
    assert_eq!(info.track.start, Some(start + TimeDelta::seconds(3)));
    let gps = &info.track.gps;
    // fixes at 3..6 s of the source on the output timeline, plus the fix before the window
    // (pieces shorter than 20 s take it along)
    let times: Vec<f64> = gps.iter().map(|p| p.time).collect();
    assert_eq!(times, [-1.0, 0.0, 1.0, 2.0, 3.0]);
    for p in gps {
        let src_second = p.time + 3.0;
        assert!((p.lat - (60.0 + src_second / 1000.0)).abs() < 1e-9);
        assert!((p.speed_kmh - (50.0 + src_second)).abs() < 1e-9);
        assert_eq!(p.course, 90.0);
    }
    let gs = &info.track.gsensor;
    assert_eq!(gs.len(), 33);
    assert!(gs.iter().all(|s| s.time >= 0.0 && s.time < 3.2214));
    assert!((gs[1].z - 0.31).abs() < 1e-9);
    // data track layout
    let m = movie(&out);
    let t = m.track_by_codec(b"RVMI").unwrap();
    assert_eq!((&t.handler, t.timescale), (b"data", 50));
    let d = std::fs::read(&out).unwrap();
    let hdlr = find_box(&d, &[b"moov", b"trak", b"mdia", b"hdlr"], 2).unwrap();
    assert_eq!(&hdlr[24..], b" GPS and G-Sensor");
    // RVMI samples come first in the data
    let first_rvmi = t.samples()[0].offset;
    assert!(
        m.tracks
            .iter()
            .flat_map(|t| t.samples())
            .all(|s| s.offset >= first_rvmi)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn meta_is_compressed_thinned_and_joined() {
    let dir = temp_dir("meta4");
    let start = start_time();
    let a = write(&dir, "a.mov", &spec_with_gps(1, start));
    // the second file was recorded two hours later
    let later = start + TimeDelta::hours(2);
    let b = write(&dir, "b.mov", &spec_with_gps(2, later));
    let out = dir.join("out.mov");
    let mut i0 = item(&a, 0.0, 1.0);
    i0.speed = 4.0;
    let i1 = item(&b, 0.0, 0.5);
    let report = run(&[i0, i1], &opts(true), &out);
    let span1 = report.items[1];
    let info = FileInfo::load(&out);
    assert_eq!(info.track.start, Some(start));
    let gps = &info.track.gps;
    // item 0 at 4x: fixes every 0.25 s (kept: >= 0.21 s apart); RVMI times are in 1/50 s
    let tick = 0.02;
    let first: Vec<f64> = gps
        .iter()
        .map(|p| p.time)
        .filter(|&t| t < span1.start - tick)
        .collect();
    assert_eq!(first.len(), 10);
    assert!(first.windows(2).all(|w| (w[1] - w[0] - 0.25).abs() <= tick));
    // G-sensor at 40 Hz thinned to one per 0.03 s or more
    let gs: Vec<f64> = info
        .track
        .gsensor
        .iter()
        .map(|s| s.time)
        .filter(|&t| t < 2.5)
        .collect();
    assert!(gs.windows(2).all(|w| w[1] - w[0] >= 0.03 - 1e-9));
    assert_eq!(gs.len(), 50);
    // item 1 follows without the two-hour gap, at normal speed
    let second: Vec<f64> = gps
        .iter()
        .map(|p| p.time)
        .filter(|&t| t >= span1.start - tick)
        .collect();
    assert_eq!(second.len(), 6);
    assert!(
        (second[0] - span1.start).abs() <= tick,
        "{second:?} vs {span1:?}"
    );
    assert!(second.windows(2).all(|w| (w[1] - w[0] - 1.0).abs() <= tick));
    // the segment break is flagged on the last fix before the gap (the end marker)
    let m = movie(&out);
    let t = m.track_by_codec(b"RVMI").unwrap();
    let recs = payloads(&out, &t.samples());
    let flagged: Vec<&Vec<u8>> = recs
        .iter()
        .filter(|r| &r[..4] == b"gReV" && r[19] == 1)
        .collect();
    assert_eq!(flagged.len(), 1);
    assert!(flagged[0][4..12].iter().all(|&b| b == 0), "marker fix");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn meta_uses_given_track_and_start() {
    let dir = temp_dir("given");
    let src = write(&dir, "a.mov", &spec(1));
    let out = dir.join("out.mov");
    let start = start_time();
    let track = rv_core::Track {
        start: Some(start),
        gps: (0..10)
            .map(|i| rv_core::GpsPoint {
                time: f64::from(i),
                lat: 59.0,
                lon: 30.0 + f64::from(i) / 1000.0,
                altitude: 10.0,
                speed_kmh: 30.0,
                course: 180.0,
                warning: 0,
            })
            .collect(),
        gsensor: vec![],
    };
    let mut it = item(&src, 0.0, 1.0);
    it.start = Some(start);
    it.track = Some(track);
    let report = run(&[it], &opts(true), &out);
    assert_eq!(report.t_min, Some(start));
    let info = FileInfo::load(&out);
    assert_eq!(info.track.start, Some(start));
    assert_eq!(info.track.gps.len(), 10);
    assert!((info.track.gps[3].lon - 30.003).abs() < 1e-9);
    let _ = std::fs::remove_dir_all(dir);
}
