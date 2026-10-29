//! Manual test on real dash-cam recordings (ignored by default):
//!
//! `cargo test -p rv-edit --test real_files -- --ignored --nocapture`
//!
//! The samples are copied from the read-only share (or the folder in `RV_EDIT_SAMPLES`) to
//! `%TEMP%\rvedit_samples` first; outputs go to `%TEMP%\rvedit_out`.

use std::path::{Path, PathBuf};

use rv_core::FileInfo;
use rv_core::mp4::read_movie;
use rv_edit::{CropItem, ExportOptions, export};
use rv_video::VideoStream;

const SHARE: &str = r"\\192.168.6.47\p\Video\reg";
const FILES: [&str; 2] = [
    "Fragment of 05161259_0066 (8) (13-06-26 - 13-06-31).MOV",
    "Fragment of 2019_0316_152714_195 (13) (15-29-57 - 15-30-55).MP4",
];

fn sample(name: &str) -> Option<PathBuf> {
    let local_dir = std::env::temp_dir().join("rvedit_samples");
    let local = local_dir.join(name);
    if local.exists() {
        return Some(local);
    }
    let share = std::env::var("RV_EDIT_SAMPLES").unwrap_or_else(|_| SHARE.to_string());
    let remote = Path::new(&share).join(name);
    std::fs::create_dir_all(&local_dir).ok()?;
    match std::fs::copy(&remote, &local) {
        Ok(_) => Some(local),
        Err(e) => {
            eprintln!("skipping {name}: {e}");
            None
        }
    }
}

fn piece(path: &Path, from: f64, to: f64, duration: f64) -> CropItem {
    CropItem {
        path: path.to_path_buf(),
        p1: from / duration,
        p2: to / duration,
        speed: 1.0,
        speed_up_fps: true,
        start: None,
        track: None,
    }
}

/// Exports, then decodes every frame of the output; returns (video samples, decoded frames).
fn export_and_decode(items: &[CropItem], out: &Path) -> (usize, usize) {
    let report = export(items, &ExportOptions::default(), out, &mut |_| true).expect("export");
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    let movie = read_movie(&mut std::io::BufReader::new(
        std::fs::File::open(out).unwrap(),
    ))
    .unwrap();
    let video = movie.track_by_handler(b"vide").unwrap();
    let samples = video.samples();
    assert!(samples[0].sync, "first sample is a key frame");
    let info = FileInfo::load(out);
    assert!(info.error.is_none(), "{:?}", info.error);

    let mut stream = VideoStream::open(out).expect("open output");
    let first = stream.next_frame().expect("decode").expect("first frame");
    assert!(first.width > 0 && first.rgba.len() == (first.width * first.height * 4) as usize);
    let mut frames = 1;
    while stream.next_frame().expect("decode").is_some() {
        frames += 1;
    }
    println!(
        "{}: {:.3} s, {} bytes, {} video samples, {} decoded ({}x{}), audio {}, gps {} ({:?}), g-sensor {}, start {:?}",
        out.file_name().unwrap().to_string_lossy(),
        report.duration,
        report.bytes,
        samples.len(),
        frames,
        first.width,
        first.height,
        movie
            .track_by_handler(b"soun")
            .map_or(0, |t| t.sample_count()),
        info.track.gps.len(),
        info.gps_format,
        info.track.gsensor.len(),
        info.track.start,
    );
    (samples.len(), frames)
}

#[test]
#[ignore = "needs the sample recordings (network share)"]
fn cuts_real_samples_and_the_output_decodes() {
    let out_dir = std::env::temp_dir().join("rvedit_out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut done = 0;
    for name in FILES {
        let Some(src) = sample(name) else { continue };
        let info = FileInfo::load(&src);
        assert!(info.error.is_none(), "{name}: {:?}", info.error);
        let d = info.duration;
        println!(
            "source {name}: {d:.3} s, {:?}, gps {} g-sensor {}",
            info.resolution(),
            info.track.gps.len(),
            info.track.gsensor.len()
        );
        let ext = src.extension().unwrap().to_string_lossy().to_string();

        // a few seconds from the middle
        let stem = src.file_stem().unwrap().to_string_lossy().to_string();
        let out = out_dir.join(format!("cut {stem}.{ext}"));
        let (samples, frames) = export_and_decode(&[piece(&src, 1.2, 4.2, d)], &out);
        assert_eq!(frames, samples);

        // two pieces joined, the second at 4x key frames only
        let mut fast = piece(&src, 0.0, d.min(40.0), d);
        fast.speed = 4.0;
        fast.speed_up_fps = false;
        let out = out_dir.join(format!("join {stem}.{ext}"));
        let (samples, frames) = export_and_decode(&[piece(&src, 0.5, 2.5, d), fast], &out);
        assert_eq!(frames, samples);
        done += 1;
    }
    assert!(done > 0, "no sample files available");
}
