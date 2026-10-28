//! Manual smoke check: scans a folder (read-only) and prints the resulting rows.
//!
//! `cargo run -p rv-fm --example smoke -- <folder> [gps 0|1]`

use std::time::{Duration, Instant};

use rv_fm::FileManager;
use rv_fm::time::to_naive;

fn main() {
    let mut args = std::env::args().skip(1);
    let folder = args.next().expect("usage: smoke <folder> [gps 0|1]");
    let gps = args.next().is_none_or(|g| g != "0");
    let fm = FileManager::new(false, gps);
    let t = Instant::now();
    fm.add(&folder);
    let mut last = 0;
    while !fm.wait_idle(Duration::from_millis(250)) {
        let v = fm.version();
        if v != last {
            let r = fm.rows();
            println!("[{:6.2}s] v{v} {} {} rows={}", t.elapsed().as_secs_f64(), r.status_key, r.status_arg, r.rows.len());
            last = v;
        }
    }
    let rows = fm.rows();
    println!("idle after {:.2}s: {} rows, any_locked={}", t.elapsed().as_secs_f64(), rows.rows.len(), rows.any_locked);
    for r in &rows.rows {
        let g = &r.summary;
        let start = to_naive(g.video.time.start()).map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string());
        println!(
            "{:<32} files={} lock={} dur={}ms state={} start={} size={} {}x{} track={:.0}m avg={:.1} gps={}",
            g.video.face,
            r.files,
            g.video.lock,
            g.video.duration_ms,
            g.video.duration_state,
            start.unwrap_or_default(),
            g.video.time.size,
            g.video.width,
            g.video.height,
            g.info.track_length,
            g.info.avg_speed,
            g.info.gps
        );
    }
}
