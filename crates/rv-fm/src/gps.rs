//! GPS data for the file manager: the loader interface, statistics and time span of a track
//! (`ExtractGPSInfo` / `ExtractGPSTime`), the overlap measure and extension ranks (spec §4.4, §4.5).

use std::path::Path;

use rv_core::{FileInfo, GpsPoint, Track};

use crate::model::{GpsInfo, TimeBlock, VideoFile, file_name};
use crate::time::{NO_DATE, SEC, from_naive};

/// Known GPS log / sidecar extensions, in the sidecar search order (§4.5).
pub const GPS_EXTENSIONS: [&str; 12] =
    ["gpx", "kml", "map", "gmap", "dat", "log", "plt", "txt", "nmea", "ifo", "gps", "srt"];

/// Video extensions found by folder scans (FL-004).
pub const SCAN_VIDEO_EXTENSIONS: [&str; 3] = ["mov", "mp4", "mp4v"];

/// Lower-cased text after the last '.' of the file name.
pub fn extension_lower(path: &str) -> Option<String> {
    let name = file_name(path);
    name.rfind('.').map(|i| name[i + 1..].to_lowercase())
}

/// `HasFileExt`.
pub fn has_gps_extension(path: &str) -> bool {
    extension_lower(path).is_some_and(|e| GPS_EXTENSIONS.contains(&e.as_str()))
}

pub fn has_scan_video_extension(path: &str) -> bool {
    extension_lower(path).is_some_and(|e| SCAN_VIDEO_EXTENSIONS.contains(&e.as_str()))
}

/// `GetFileExtPower`: higher = preferred as a file's GPS info.
pub fn extension_rank(path: &str) -> i32 {
    match extension_lower(path).as_deref() {
        Some("kml") => 1,
        Some("srt") => 2,
        Some("txt") => 3,
        Some("dat") => 4,
        Some("log") => 6,
        Some("plt") => 7,
        Some("gpx") => 8,
        _ => 5,
    }
}

/// Reads GPS data from a file: a sidecar or pool log, or a video with an embedded track.
/// Returning `None` means the file could not be read as GPS data.
pub trait GpsLoader: Send + Sync {
    fn load(&self, path: &Path) -> Option<Track>;
}

/// The loader used by default: embedded tracks of videos (rv-core). Sidecar log formats
/// (GPX, NMEA, ...) are not parsed yet, so logs load as `None`; plug a parser in here.
pub struct DefaultGpsLoader;

impl GpsLoader for DefaultGpsLoader {
    fn load(&self, path: &Path) -> Option<Track> {
        let ext = extension_lower(&path.to_string_lossy())?;
        if !SCAN_VIDEO_EXTENSIONS.contains(&ext.as_str()) {
            return None;
        }
        let info = FileInfo::load(path);
        (info.error.is_none() && !info.track.is_empty()).then_some(info.track)
    }
}

fn valid_fix(p: &GpsPoint) -> bool {
    p.lat != 0.0 || p.lon != 0.0
}

/// What the original takes from a loaded track (`TMovingInfo`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackStats {
    pub gps_points: u32,
    pub gsensor_points: u32,
    /// 0 none, 1 points without a valid fix, 2 valid fix.
    pub gps_state: u8,
    pub track_length_m: f64,
    pub avg_speed_kmh: f64,
    /// TDateTime of the first and last point (GPS or G-sensor); small values when the track
    /// has no wall-clock start.
    pub min_time: f64,
    pub max_time: f64,
    pub format_id: String,
}

fn start_tdt(track: &Track) -> f64 {
    track.start.map_or(0.0, from_naive)
}

pub fn track_stats(track: &Track) -> TrackStats {
    let times = track.gps.iter().map(|p| p.time).chain(track.gsensor.iter().map(|g| g.time));
    let (lo, hi) = times.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| (lo.min(t), hi.max(t)));
    let base = start_tdt(track);
    let (min_time, max_time) = if lo.is_finite() { (base + lo * SEC, base + hi * SEC) } else { (0.0, 0.0) };
    let gps_state = if track.gps.iter().any(valid_fix) {
        2
    } else if track.is_empty() {
        0
    } else {
        1
    };
    TrackStats {
        gps_points: track.gps.len() as u32,
        gsensor_points: track.gsensor.len() as u32,
        gps_state,
        // abs(): the sum over no segments is -0.0
        track_length_m: (track.distance_km() * 1000.0).abs(),
        avg_speed_kmh: track.avg_speed_kmh().abs(),
        min_time,
        max_time,
        format_id: String::new(),
    }
}

/// The part of a track inside the wall-clock window [from, to] (TDateTime).
pub fn crop(track: &Track, from: f64, to: f64) -> Track {
    let base = start_tdt(track);
    let inside = |t: f64| {
        let abs = base + t * SEC;
        abs >= from && abs <= to
    };
    Track {
        start: track.start,
        gps: track.gps.iter().filter(|p| inside(p.time)).copied().collect(),
        gsensor: track.gsensor.iter().filter(|g| inside(g.time)).copied().collect(),
    }
}

/// `ExtractGPSInfo`.
pub fn extract_gps_info(info: &mut GpsInfo, st: &TrackStats) {
    info.track_length = st.track_length_m;
    info.gps = st.gps_state;
    info.format_id = st.format_id.clone();
    info.avg_speed = st.avg_speed_kmh;
    info.gps_points = st.gps_points;
    info.gsensor_points = st.gsensor_points;
}

/// `ExtractGPSTime`: finish = last point, duration = span (0 without dates); the file-system
/// creation time is moved back when the file window is shorter than the track.
pub fn extract_gps_time(info: &mut GpsInfo, st: &TrackStats) {
    let t = &mut info.time;
    t.finish = st.max_time;
    t.duration = if st.min_time <= NO_DATE { 0.0 } else { st.max_time - st.min_time };
    if t.duration < 0.0 {
        t.duration = 0.0;
    }
    if t.duration > 0.0 && t.write - t.creation <= t.duration {
        t.creation = t.write - t.duration;
    }
}

/// `GetIntersection2`: weighted overlap (days) of two windows given as file-system start /
/// finish and real duration.
#[allow(clippy::too_many_arguments)]
pub fn intersection2(fn1: &str, fn2: &str, use_names: bool, s1: f64, f1: f64, d1: f64, s2: f64, f2: f64, d2: f64) -> f64 {
    let mut r = 0.0;
    if (s1 as f32) > 2.0 && s1 < f1 && (s2 as f32) > 2.0 && s2 < f2 && d1 > 0.0 && d2 > 0.0 {
        r = f1.min(f2) - s1.max(s2);
        if r > 2.0 {
            r = f1.min(f2) - (f1 - d1).max(f2 - d2);
        }
        if r > 11.0 * SEC && r > 0.9 * d1.min(d2) {
            r = d1.max(d2);
        }
        r = r.min(d1.max(d2));
    }
    if !use_names || r <= 0.0 {
        return r;
    }
    let tail: String = fn2.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    if matches!(tail.to_ascii_lowercase().as_str(), ".log" | ".gpx" | ".plt") {
        r *= 2.0;
    }
    let dist = crate::grouping::file_distance(fn1, fn2);
    f64::from(r as f32 / (dist as f32 / 100.0 + 1.0))
}

/// `GetIntersection`: overlap of a video with a GPS item, using the file-system windows.
pub fn intersection(video: &VideoFile, gps: &GpsInfo, use_names: bool) -> f64 {
    let (v, g): (&TimeBlock, &TimeBlock) = (&video.time, &gps.time);
    intersection2(&video.path, &gps.path, use_names, v.creation, v.write, v.duration, g.creation, g.write, g.duration)
}

/// `GetFileFace` used to match a log to a video: file name without extension, without a
/// trailing ".", lower-cased.
pub fn match_face(path: &str) -> String {
    let face = crate::model::file_face(path);
    face.strip_suffix('.').unwrap_or(face).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn pt(time: f64, lat: f64) -> GpsPoint {
        GpsPoint { time, lat, lon: 30.0, altitude: 0.0, speed_kmh: 50.0, course: 0.0, warning: 0 }
    }

    #[test]
    fn stats_and_time_of_a_track() {
        let start = NaiveDate::from_ymd_opt(2016, 5, 16).unwrap().and_hms_opt(12, 0, 0).unwrap();
        let track = Track { start: Some(start), gps: vec![pt(1.0, 60.0), pt(61.0, 60.01)], gsensor: vec![] };
        let st = track_stats(&track);
        assert_eq!((st.gps_points, st.gps_state), (2, 2));
        assert!((st.track_length_m - 1112.0).abs() < 2.0);
        assert!((st.min_time - (42506.5 + SEC)).abs() < 1e-9);
        let mut info = GpsInfo::default();
        info.time.creation = 42506.5 + 30.0 * SEC;
        info.time.write = 42506.5 + 60.0 * SEC;
        extract_gps_info(&mut info, &st);
        extract_gps_time(&mut info, &st);
        assert!((info.time.duration - 60.0 * SEC).abs() < 1e-9);
        assert!((info.time.finish - (42506.5 + 61.0 * SEC)).abs() < 1e-9);
        assert!((info.time.creation - (42506.5 + 0.0 * SEC)).abs() < 1e-9);
        let c = crop(&track, 42506.5, 42506.5 + 30.0 * SEC);
        assert_eq!(c.gps.len(), 1);
        assert_eq!(track_stats(&Track::default()).gps_state, 0);
    }

    #[test]
    fn ranks_and_overlap() {
        assert!(extension_rank("a.gpx") > extension_rank("a.LOG"));
        assert!(extension_rank("a.log") > extension_rank("a.nmea"));
        assert!(extension_rank("a.nmea") > extension_rank("a.kml"));
        assert!(has_gps_extension(r"C:\x\A.NMEA") && !has_gps_extension(r"C:\x\a.mov"));
        let t0 = 42506.5;
        let ov = intersection2("a", "b", false, t0, t0 + 60.0 * SEC, 60.0 * SEC, t0 + 30.0 * SEC, t0 + 120.0 * SEC, 90.0 * SEC);
        assert!((ov - 30.0 * SEC).abs() < 1e-9);
        // nearly identical windows count as the longer duration
        let ov = intersection2("a", "b", false, t0, t0 + 60.0 * SEC, 60.0 * SEC, t0 + 1.0 * SEC, t0 + 62.0 * SEC, 61.0 * SEC);
        assert!((ov - 61.0 * SEC).abs() < 1e-9);
        assert_eq!(intersection2("a", "b", false, 0.0, 1.0, 1.0, t0, t0 + 1.0, 1.0), 0.0);
        assert_eq!(match_face(r"C:\x\FILE0001..gpx"), "file0001");
    }
}
