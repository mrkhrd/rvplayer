//! Decode benchmark: `cargo run -p rv-video --release --example bench -- <file> [seconds] [frame.ppm]`
//!
//! Decodes the first `seconds` (default 10) of the video, prints per-stage throughput, times a seek
//! to the middle, and optionally writes the seek target frame as a PPM image for visual checking.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use rv_video::VideoStream;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: bench <file> [seconds] [frame.ppm]");
        std::process::exit(2);
    };
    let seconds: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10.0);

    let mut s = VideoStream::open(Path::new(path))?;
    println!(
        "{:?} {}x{}, {:.1}s, {:.2} fps, {} threads",
        s.codec,
        s.width,
        s.height,
        s.duration,
        s.frame_rate(),
        rayon::current_num_threads()
    );

    let t0 = Instant::now();
    let mut frames = 0u32;
    let mut last = 0.0;
    while let Some(f) = s.next_frame()? {
        frames += 1;
        last = f.time;
        if f.time >= seconds {
            break;
        }
    }
    let wall = t0.elapsed().as_secs_f64();
    let st = s.stats;
    let per = |d: std::time::Duration| d.as_secs_f64() * 1000.0 / f64::from(frames.max(1));
    println!(
        "decoded {frames} frames ({last:.2}s of video) in {wall:.2}s -> {:.1} fps ({:.2}x realtime)",
        f64::from(frames) / wall,
        last / wall
    );
    println!(
        "per frame: read {:.2} ms, decode {:.2} ms, convert {:.2} ms",
        per(st.read),
        per(st.decode),
        per(st.convert)
    );

    let mid = s.duration / 2.0;
    let t = Instant::now();
    let frame = s.seek(mid)?;
    let seek_ms = t.elapsed().as_secs_f64() * 1000.0;
    match &frame {
        Some(f) => println!(
            "seek to {mid:.2}s -> frame at {:.3}s in {seek_ms:.0} ms",
            f.time
        ),
        None => println!("seek to {mid:.2}s -> no frame ({seek_ms:.0} ms)"),
    }

    if let (Some(out), Some(f)) = (args.get(2), frame) {
        let mut w = std::io::BufWriter::new(std::fs::File::create(out)?);
        write!(w, "P6\n{} {}\n255\n", f.width, f.height)?;
        for px in f.rgba.chunks_exact(4) {
            w.write_all(&px[..3])?;
        }
        println!("wrote {out}");
    }
    Ok(())
}
