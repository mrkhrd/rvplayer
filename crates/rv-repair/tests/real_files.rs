//! Manual test with real dashcam files (exports of the original app, `moov` before `mdat`).
//!
//! The share is READ-ONLY: each file is copied to the temp folder first. A power cut is simulated
//! by rewriting the copy in camera layout (`ftyp`, `mdat`, no `moov`, mdat size 0), which is then
//! repaired with the intact copy (or another file of the same camera) as reference, read back and
//! decoded.
//!
//! `cargo test -p rv-repair --release --test real_files -- --ignored --nocapture`

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rv_core::mp4::{self, Movie, Sample, Track};
use rv_video::VideoStream;

const SHARE: &str = r"\\192.168.6.47\p\Video\reg";

/// (broken source, reference); `None` uses the file itself as reference.
const CASES: [(&str, Option<&str>); 3] = [
    (
        "Fragment of 05161259_0066 (8) (13-06-26 - 13-06-31).MOV",
        None,
    ),
    ("Fragment of 163641AA (2) (16-38-43 - 16-38-48).MP4", None),
    (
        "Fragment of 163641AA (2) (16-38-43 - 16-38-48).MP4",
        Some("Fragment of 132015AA (9) (13-57-23 - 13-57-35).MP4"),
    ),
];

fn temp_copy(name: &str) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("rv-repair-real-{name}"));
    if !dst.exists() {
        fs::copy(Path::new(SHARE).join(name), &dst).expect("copy from share");
    }
    dst
}

fn boxes(d: &[u8]) -> Vec<([u8; 4], usize, usize)> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 8 <= d.len() {
        let size = u32::from_be_bytes(d[pos..pos + 4].try_into().unwrap()) as usize;
        let size = if size == 0 { d.len() - pos } else { size };
        out.push((d[pos + 4..pos + 8].try_into().unwrap(), pos, size));
        pos += size;
    }
    out
}

fn video(m: &Movie) -> &Track {
    m.track_by_handler(b"vide").expect("video track")
}

fn audio(m: &Movie) -> Vec<Sample> {
    m.track_by_handler(b"soun")
        .map(Track::samples)
        .unwrap_or_default()
}

#[test]
#[ignore = "needs the dashcam sample share"]
fn repairs_real_dashcam_files() {
    for (name, reference) in CASES {
        let good_path = temp_copy(name);
        let ref_path = reference.map_or_else(|| good_path.clone(), temp_copy);
        let good = fs::read(&good_path).unwrap();
        let movie = mp4::read_movie(&mut Cursor::new(&good)).unwrap();

        // camera layout: ftyp + mdat (size 0), no moov
        let top = boxes(&good);
        let (_, ftyp_at, ftyp_len) = *top.iter().find(|b| &b.0 == b"ftyp").unwrap();
        let (_, mdat_at, mdat_len) = *top.iter().find(|b| &b.0 == b"mdat").unwrap();
        let mut broken = good[ftyp_at..ftyp_at + ftyp_len].to_vec();
        let new_mdat_at = broken.len();
        broken.extend_from_slice(&[0, 0, 0, 0]);
        broken.extend_from_slice(&good[mdat_at + 4..mdat_at + mdat_len]);
        let shift = mdat_at as i64 - new_mdat_at as i64;
        let broken_path = std::env::temp_dir().join(format!("rv-repair-real-broken-{name}"));
        fs::write(&broken_path, &broken).unwrap();

        let t = Instant::now();
        let summary = rv_repair::repair(&broken_path, &ref_path).expect("repair");
        let took = t.elapsed();
        let fixed_bytes = fs::read(&broken_path).unwrap();
        let fixed = mp4::read_movie(&mut Cursor::new(&fixed_bytes)).unwrap();

        // every video frame at the same place
        let (gv, fv) = (video(&movie), video(&fixed));
        assert_eq!(fv.codec_config, gv.codec_config);
        let expect: Vec<(u64, u32, bool)> = gv
            .samples()
            .iter()
            .map(|s| ((s.offset as i64 - shift) as u64, s.size, s.sync))
            .collect();
        let got: Vec<(u64, u32, bool)> = fv
            .samples()
            .iter()
            .map(|s| (s.offset, s.size, s.sync))
            .collect();
        assert_eq!(got, expect, "{name}: video samples");

        // audio: compare the samples before the last video frame
        let last_video = expect.last().unwrap().0;
        let ga: Vec<(u64, u32)> = audio(&movie)
            .iter()
            .map(|s| ((s.offset as i64 - shift) as u64, s.size))
            .filter(|s| s.0 < last_video)
            .collect();
        let fa_samples = audio(&fixed);
        let fa: Vec<(u64, u32)> = fa_samples.iter().map(|s| (s.offset, s.size)).collect();
        let same = fa.iter().filter(|s| ga.binary_search(s).is_ok()).count();
        let bytes = |v: &[(u64, u32)]| v.iter().map(|s| u64::from(s.1)).sum::<u64>();
        assert_eq!(
            bytes(&fa),
            bytes(&ga),
            "{name}: audio bytes before the last video frame"
        );
        let fixed_audio = fixed.track_by_handler(b"soun").unwrap();
        let good_audio = movie.track_by_handler(b"soun").unwrap();
        let fixed_secs = fixed_audio.duration_secs();
        let good_secs = ga.len() as f64 * 1024.0 / f64::from(good_audio.timescale);

        // decodes
        let mut stream = VideoStream::open(&broken_path).expect("open repaired");
        let first = stream.next_frame().unwrap().expect("first frame");
        let mut decoded = 1;
        while stream.next_frame().unwrap().is_some() {
            decoded += 1;
        }
        assert_eq!(decoded, gv.sample_count(), "{name}: decoded frames");

        println!(
            "{name}\n  reference {}\n  {} bytes -> header at {:?}, {:.2?}\n  video {} frames (all match), \
             first frame {}x{} key={}, {decoded} decoded\n  audio {} samples / {} frames found \
             ({same} identical to the original's {}), {:.3} s vs {:.3} s",
            ref_path.file_name().unwrap().to_string_lossy(),
            broken.len(),
            summary.header_offset,
            took,
            summary.video_frames,
            first.width,
            first.height,
            first.key,
            fa.len(),
            summary.audio_frames,
            ga.len(),
            fixed_secs,
            good_secs,
        );
        fs::remove_file(&broken_path).unwrap();
    }
}
