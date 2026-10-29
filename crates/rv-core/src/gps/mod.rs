//! GPS / G-sensor data from logs and videos, ported from Registrator Viewer's `UnitGPSManager`
//! and its 31 `UnitGps_*` format units, plus Novatek GPS blocks embedded in MP4/MOV.
//!
//! The pipeline follows the original: a log is split into records (lines, or a format's own
//! binary records); every format that accepts the data is offered each record, the format
//! that read the previous record first; records become [`format::Item`]s with local
//! wall-clock TDateTime times; then [`build::to_track`] turns them into a [`Track`] whose
//! times are seconds from `Track.start`.
//!
//! UTC times in the data are converted to local time with the computer's current time-zone
//! offset, like the original (`timeZoneLocalSystem`): file times and camera clocks are local.

pub mod build;
pub mod embedded;
pub mod format;
pub mod formats;
pub mod manager;

pub mod util;
pub mod write;

use std::fmt;
use std::io::Read;
use std::path::Path;

use crate::track::Track;

pub use format::{Ctx, Format, Item, RawItem};

/// Logs are read up to this size (`rvReadFile(fn, $A00000)`); the rest is ignored.
pub const MAX_LOG_SIZE: u64 = 0xa0_0000;

/// A loaded GPS / G-sensor source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpsLog {
    /// Format id as the original reports it ("NMEA", "gpx", "movRegV", "movDOD_D", ...).
    pub format: String,
    pub track: Track,
    /// The camera marked the recording as parking mode.
    pub parking_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpsError {
    Io(String),
    /// No format recognised the data, or it holds no usable record.
    NoData,
    /// Recognised but not readable (e.g. encrypted GPS blocks).
    Unsupported(String),
}

impl fmt::Display for GpsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpsError::Io(e) => write!(f, "{e}"),
            GpsError::NoData => write!(f, "no GPS or G-sensor data"),
            GpsError::Unsupported(what) => write!(f, "unsupported GPS data: {what}"),
        }
    }
}

impl std::error::Error for GpsError {}

/// Environment of a load: time zone and file times.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadOptions {
    /// Local time-zone offset in days added to UTC times.
    pub tz: f64,
    /// File-system (creation, last write) times as local TDateTime; (0, 0) when unknown.
    pub file_times: (f64, f64),
}

impl LoadOptions {
    /// The computer's current offset and no file times.
    pub fn local() -> LoadOptions {
        LoadOptions { tz: local_tz_offset_days(), file_times: (0.0, 0.0) }
    }
}

/// `DetectTimeZone`: the current local offset from UTC in days.
pub fn local_tz_offset_days() -> f64 {
    f64::from(chrono::Local::now().offset().local_minus_utc()) / 86_400.0
}

/// Creation and last-write times of a file as local TDateTime values.
pub fn file_times(meta: &std::fs::Metadata) -> (f64, f64) {
    let conv = |t: std::io::Result<std::time::SystemTime>| {
        t.ok().map(|t| naive_to_tdatetime(chrono::DateTime::<chrono::Local>::from(t).naive_local())).unwrap_or(0.0)
    };
    let write = conv(meta.modified());
    let create = conv(meta.created());
    (if create > 0.0 { create } else { write }, write)
}

pub fn naive_to_tdatetime(dt: chrono::NaiveDateTime) -> f64 {
    let epoch =
        chrono::NaiveDate::from_ymd_opt(1899, 12, 30).and_then(|d| d.and_hms_opt(0, 0, 0)).expect("valid epoch");
    let d = dt - epoch;
    d.num_milliseconds() as f64 / 86_400_000.0
}

/// Lower-case extensions of GPS logs the formats claim (`allExts`), in registry order.
pub fn extensions() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for f in formats::all() {
        for e in f.all_ext() {
            if !out.contains(&e) {
                out.push(e);
            }
        }
    }
    out
}

/// Video extensions whose embedded data is read instead of parsing the file as a log.
pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["mov", "mp4", "mp4v"].iter().any(|v| e.eq_ignore_ascii_case(v)))
}

/// Loads GPS data from a file: the embedded data of a video, or a log.
pub fn load_path(path: &Path) -> Result<GpsLog, GpsError> {
    if is_video_path(path) { embedded::load_video_path(path) } else { load_log_file(path) }
}

/// Loads a GPS log file (`TMovingInfo.CreateAutoSelectSource` for non-video files): at most
/// [`MAX_LOG_SIZE`] bytes are read. A BlackVue `.3gf` next to it supplies G-sensor data.
pub fn load_log_file(path: &Path) -> Result<GpsLog, GpsError> {
    let io = |e: std::io::Error| GpsError::Io(e.to_string());
    let file = std::fs::File::open(path).map_err(io)?;
    let meta = file.metadata().map_err(io)?;
    let mut data = Vec::new();
    file.take(MAX_LOG_SIZE).read_to_end(&mut data).map_err(io)?;
    let opts = LoadOptions { tz: local_tz_offset_days(), file_times: file_times(&meta) };
    let companion = |ext: &str| read_companion(path, ext);
    load_log_bytes(&path.to_string_lossy(), &data, &opts, &companion)
}

/// `<path up to its last '.'>.<ext>` with its write time, if it exists and is not too large.
pub fn read_companion(path: &Path, ext: &str) -> Option<(Vec<u8>, f64)> {
    let p = path.with_extension(ext);
    let meta = std::fs::metadata(&p).ok()?;
    if !meta.is_file() || meta.len() > MAX_LOG_SIZE {
        return None;
    }
    Some((std::fs::read(&p).ok()?, file_times(&meta).1))
}

/// Loads a log from memory. `name` is used for messages only; `companion` supplies files
/// next to the log by extension (see [`read_companion`]).
pub fn load_log_bytes(
    name: &str,
    data: &[u8],
    opts: &LoadOptions,
    companion: manager::CompanionReader,
) -> Result<GpsLog, GpsError> {
    let mut formats = formats::all();
    let env = manager::Env { name: name.to_string(), file_times: opts.file_times, tz: opts.tz, companion };
    let loaded = manager::load_file(&mut formats, data, &env).ok_or(GpsError::NoData)?;
    finish(loaded, opts.file_times)
}

/// Loaded points to the public result.
pub(crate) fn finish(loaded: manager::Loaded, file_times: (f64, f64)) -> Result<GpsLog, GpsError> {
    let post_process = !loaded.info.do_not_post_process_fix;
    let track = build::to_track(&loaded.points, loaded.frame0_time, file_times, post_process);
    if track.is_empty() {
        return Err(GpsError::NoData);
    }
    Ok(GpsLog { format: loaded.info.format_id, track, parking_mode: loaded.info.parking_mode })
}

#[cfg(test)]
pub(crate) mod test_support {
    pub use super::{GpsError, GpsLog};
    use super::{LoadOptions, load_log_bytes};

    /// Loads a log with UTC as the local zone, no file times and no companion files.
    pub fn load_log(name: &str, data: &[u8]) -> Result<GpsLog, GpsError> {
        load_log_bytes(name, data, &LoadOptions { tz: 0.0, file_times: (0.0, 0.0) }, &|_| None)
    }

    /// Like [`load_log`] with file times (creation, write) as TDateTime.
    pub fn load_log_at(name: &str, data: &[u8], file_times: (f64, f64)) -> Result<GpsLog, GpsError> {
        load_log_bytes(name, data, &LoadOptions { tz: 0.0, file_times }, &|_| None)
    }

    /// Recomputes the NMEA checksum ("*hh") of the given lines (0-based). The sentence starts
    /// after the first '$', or at the 'G' of "G?RMC" when there is no '$'.
    pub fn fix_checksums(text: &str, lines: &[usize]) -> String {
        let mut out: Vec<String> = Vec::new();
        for (i, l) in text.split('\n').enumerate() {
            if !lines.contains(&i) {
                out.push(l.to_string());
                continue;
            }
            let start = match l.find('$') {
                Some(p) => p + 1,
                None => l.find("RMC,").map_or(0, |p| p.saturating_sub(2)),
            };
            let star = l.rfind('*').expect("line has a checksum");
            let x = l.as_bytes()[start..star].iter().fold(0u8, |a, &c| a ^ c);
            out.push(format!("{}*{x:02X}{}", &l[..star], &l[star + 3..]));
        }
        out.join("\n")
    }

    /// Deterministic pseudo-random bytes for robustness tests.
    pub fn noise(seed: u64, len: usize) -> Vec<u8> {
        let mut s = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s >> 24) as u8
            })
            .collect()
    }

    /// Feeds random data, truncations of `sample` and byte-flipped copies to every loader
    /// entry point; nothing may panic.
    pub fn fuzz(name: &str, sample: &[u8]) {
        for n in 0..=sample.len().min(400) {
            let _ = load_log(name, &sample[..n]);
        }
        let step = (sample.len() / 200).max(1);
        for n in (0..sample.len()).step_by(step) {
            let _ = load_log(name, &sample[..n]);
            let mut m = sample.to_vec();
            m[n] ^= 0x5a;
            let _ = load_log(name, &m);
            let mut m = sample.to_vec();
            m[n] = 0;
            let _ = load_log(name, &m);
        }
        for seed in 0..40 {
            let _ = load_log(name, &noise(seed, 64 + seed as usize * 97));
            let mut mixed = sample.to_vec();
            let at = (seed as usize * 131) % (sample.len().max(1));
            mixed.splice(at..at, noise(seed, 50));
            let _ = load_log(name, &mixed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_extensions_cover_the_original_list() {
        let ext = extensions();
        for e in ["gpx", "kml", "map", "gmap", "dat", "log", "plt", "txt", "nmea", "ifo", "gps", "srt"] {
            assert!(ext.contains(&e), "{e} missing from {ext:?}");
        }
    }

    #[test]
    fn garbage_is_no_data() {
        assert_eq!(test_support::load_log("x.log", b""), Err(GpsError::NoData));
        assert_eq!(test_support::load_log("x.log", b"hello\nworld\n"), Err(GpsError::NoData));
        for seed in 0..20 {
            let _ = test_support::load_log("x.log", &test_support::noise(seed, 5000));
        }
    }
}
