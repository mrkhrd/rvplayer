//! The trip logs of 70mai dashcams (`GPSData000001.txt`, ... at the root of the memory card,
//! next to the `Normal`, `Event`, `Parking` ... folders): one line about every second, naming
//! the clip being recorded. Not a format of the original; decoded from the cameras' files.
//!
//! ```text
//! $V02                                   (each power-on)
//! 1792817548,A,60.115903,31.371963,25900,3472,-92,-22,34,NO20261024-154056-012644F.MP4,0,0,0
//! 0,V, 0.000000,      nan,-5,41,0,0,0,0,0,0,0          (no fix: no time, no clip)
//! ```
//!
//! Fields: Unix time, status, latitude, longitude (degrees), altitude (mm), speed (cm/s),
//! accelerometer x, y, z (1/100 g, gravity included), clip file name, three zeros. The clock
//! of the log does not match the clips' (hours apart), so its records are placed in a clip by
//! the clip name and by the fixes the clip itself recorded: what a clip gets from its log is
//! the accelerometer (the videos have none) and the altitude (or the whole track when the
//! video has no GPS of its own).
//!
//! The logs grow to tens of megabytes over the camera's life: they are read whole (no
//! [`super::MAX_LOG_SIZE`] limit) but only once, to an index of where each clip's lines are.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use crate::track::{GSensorSample, GpsPoint, Track};

/// One fix of a log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Record {
    pub unix: i64,
    pub lat: f64,
    pub lon: f64,
    /// Meters.
    pub altitude: f64,
    pub speed_kmh: f64,
    /// Acceleration in g, gravity included.
    pub acc: [f64; 3],
}

/// Clip names of 70mai cameras: `NO20261024-154056-012644F.MP4` (type, date, time, number,
/// camera F/B).
pub fn is_clip_name(name: &str) -> bool {
    let b = name.as_bytes();
    let digits = |r: std::ops::Range<usize>| b.get(r).is_some_and(|s| s.iter().all(u8::is_ascii_digit));
    b.len() >= 26
        && b[0].is_ascii_uppercase()
        && b[1].is_ascii_uppercase()
        && digits(2..10)
        && b[10] == b'-'
        && digits(11..17)
        && b[17] == b'-'
        && name.rsplit_once('.').is_some_and(|(stem, ext)| {
            ext.eq_ignore_ascii_case("mp4")
                && stem.len() > 19
                && stem[18..stem.len() - 1].bytes().all(|c| c.is_ascii_digit())
                && stem.as_bytes()[stem.len() - 1].is_ascii_uppercase()
        })
}

/// The trip logs of the card a clip is on: `GPSData*.txt` in its folder or up to two folders
/// up (`<card>\Normal\Front\clip`).
pub fn logs_for_video(video: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in video.ancestors().skip(1).take(3) {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        let mut found: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    let n = n.to_ascii_lowercase();
                    n.starts_with("gpsdata") && n.ends_with(".txt")
                })
            })
            .collect();
        if !found.is_empty() {
            found.sort();
            out = found;
            break;
        }
    }
    out
}

/// Size and write time of each log, to tell when results that used them are out of date.
pub fn signature(logs: &[PathBuf]) -> String {
    logs.iter()
        .map(|p| {
            let m = std::fs::metadata(p).ok();
            let size = m.as_ref().map_or(0, |m| m.len());
            let t = m.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok());
            format!("{}:{size}:{}", p.file_name().unwrap_or_default().to_string_lossy(), t.map_or(0, |d| d.as_millis()))
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Where each clip's lines are in a log (byte ranges), by lower-case clip name.
type Index = HashMap<String, Vec<(u64, u64)>>;

/// The indexes built so far, by log path, with the size and write time they were built for.
fn indexes() -> &'static Mutex<HashMap<PathBuf, (u64, Option<SystemTime>, Arc<Index>)>> {
    static I: OnceLock<Mutex<HashMap<PathBuf, (u64, Option<SystemTime>, Arc<Index>)>>> = OnceLock::new();
    I.get_or_init(Default::default)
}

/// The clip name of a fix line (field 10), lower-case; `None` for other lines.
fn clip_of(line: &[u8]) -> Option<String> {
    let mut f = line.split(|&c| c == b',');
    let (_, status) = (f.next()?, f.next()?);
    if status != b"A" {
        return None;
    }
    let name = f.nth(7)?;
    let name = std::str::from_utf8(name).ok()?.trim();
    is_clip_name(name).then(|| name.to_ascii_lowercase())
}

fn build_index(path: &Path) -> std::io::Result<Index> {
    let mut r = BufReader::with_capacity(1 << 20, File::open(path)?);
    let mut index = Index::new();
    let mut line = Vec::new();
    let mut at = 0u64;
    let mut last: Option<String> = None;
    loop {
        line.clear();
        let n = r.read_until(b'\n', &mut line)? as u64;
        if n == 0 {
            break;
        }
        if let Some(clip) = clip_of(&line) {
            let ranges = index.entry(clip.clone()).or_default();
            match ranges.last_mut() {
                Some(range) if last.as_deref() == Some(clip.as_str()) && range.1 == at => range.1 = at + n,
                _ => ranges.push((at, at + n)),
            }
            last = Some(clip);
        } else {
            last = None;
        }
        at += n;
    }
    Ok(index)
}

fn index_of(path: &Path) -> Option<Arc<Index>> {
    let meta = std::fs::metadata(path).ok()?;
    let (size, time) = (meta.len(), meta.modified().ok());
    let mut all = indexes().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((s, t, idx)) = all.get(path)
        && *s == size
        && *t == time
    {
        return Some(idx.clone());
    }
    let idx = Arc::new(build_index(path).ok()?);
    all.insert(path.to_path_buf(), (size, time, idx.clone()));
    Some(idx)
}

/// A fix line; `None` for anything else.
pub fn parse_line(line: &str) -> Option<Record> {
    let f: Vec<&str> = line.trim().split(',').map(str::trim).collect();
    if f.len() < 10 || f[1] != "A" {
        return None;
    }
    let num = |i: usize| f[i].parse::<f64>().ok().filter(|v| v.is_finite());
    let (lat, lon) = (num(2)?, num(3)?);
    if lat.abs() > 90.0 || lon.abs() > 180.0 || (lat == 0.0 && lon == 0.0) {
        return None;
    }
    Some(Record {
        unix: f[0].parse().ok()?,
        lat,
        lon,
        altitude: num(4)? / 1000.0,
        speed_kmh: num(5)? * 0.036,
        acc: [num(6)? / 100.0, num(7)? / 100.0, num(8)? / 100.0],
    })
}

/// The fixes the logs of the card have for a clip, oldest first.
pub fn records_for_clip(video: &Path) -> Vec<Record> {
    let Some(name) = video.file_name().and_then(|n| n.to_str()).filter(|n| is_clip_name(n)) else { return vec![] };
    let key = name.to_ascii_lowercase();
    let mut out = Vec::new();
    for log in logs_for_video(video) {
        let Some(index) = index_of(&log) else { continue };
        let Some(ranges) = index.get(&key) else { continue };
        let Ok(mut file) = File::open(&log) else { continue };
        for &(from, to) in ranges {
            let mut buf = vec![0u8; (to - from) as usize];
            if file.seek(SeekFrom::Start(from)).is_err() || file.read_exact(&mut buf).is_err() {
                continue;
            }
            out.extend(String::from_utf8_lossy(&buf).lines().filter_map(parse_line));
        }
    }
    out.sort_by_key(|r| r.unix);
    out.dedup_by_key(|r| r.unix);
    out
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// Adds a clip's log records to its track (times in seconds of the video, `duration` long):
/// the accelerometer as G-sensor samples when the video has none, the altitude to fixes
/// without one, and the fixes themselves when the video has no GPS. The log's clock is placed
/// on the video's by the fixes both have (the same receiver writes both); without any, the
/// log's first line is the start of the clip.
pub fn merge(track: &mut Track, records: &[Record], duration: f64) {
    let Some(first) = records.first() else { return };
    let rel = |r: &Record| (r.unix - first.unix) as f64;
    let shift = median(
        records
            .iter()
            .filter_map(|r| {
                track
                    .gps
                    .iter()
                    .find(|p| p.warning == 0 && (p.lat - r.lat).abs() < 2e-6 && (p.lon - r.lon).abs() < 2e-6)
                    .map(|p| p.time - rel(r))
            })
            .collect(),
    )
    .unwrap_or(0.0);
    let inside = |t: f64| t >= -1.0 && t <= duration + 1.0;
    let timed: Vec<(f64, &Record)> = records.iter().map(|r| (rel(r) + shift, r)).filter(|(t, _)| inside(*t)).collect();
    if track.gsensor.is_empty() {
        track.gsensor = timed
            .iter()
            .map(|(t, r)| GSensorSample { time: t.max(0.0), x: r.acc[0], y: r.acc[1], z: r.acc[2] })
            .collect();
    }
    if track.gps.is_empty() {
        track.gps = timed
            .iter()
            .map(|(t, r)| GpsPoint {
                time: t.max(0.0),
                lat: r.lat,
                lon: r.lon,
                altitude: r.altitude,
                speed_kmh: r.speed_kmh,
                course: -1.0,
                warning: 0,
            })
            .collect();
        track.gps.dedup_by(|b, a| a.lat == b.lat && a.lon == b.lon && a.speed_kmh == b.speed_kmh);
    } else {
        for p in track.gps.iter_mut().filter(|p| p.altitude == 0.0) {
            if let Some((_, r)) = timed.iter().min_by(|a, b| (a.0 - p.time).abs().total_cmp(&(b.0 - p.time).abs()))
                && (rel(r) + shift - p.time).abs() <= 2.0
            {
                p.altitude = r.altitude;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "$V02\n\
        1792817548,A,60.115903,31.371963,25900,3472,-92,-22,34,NO20261024-154056-012644F.MP4,0,0,0\n\
        1792817549,A,60.116000,31.372500,26000,3400,-90,0,40,NO20261024-154056-012644F.MP4,0,0,0\n\
        0,V, 0.000000,      nan,-5,41,0,0,0,0,0,0,0\n\
        1792817551,A,60.116100,31.373000,26100,3300,-95,5,30,NO20261024-154056-012644F.MP4,0,0,0\n\
        1792817731,A,60.128134,31.409912,400,2972,-93,-2,33,NO20261024-154356-012645F.MP4,0,0,0\n";

    #[test]
    fn clip_names() {
        assert!(is_clip_name("NO20261024-154056-012644F.MP4"));
        assert!(is_clip_name("EV20250717-204808-003826F.mp4"));
        assert!(!is_clip_name("2019_0223_133721_061.MP4"));
        assert!(!is_clip_name("NO20261024-154056-012644F.MOV"));
    }

    #[test]
    fn lines_and_units() {
        let r = parse_line(LOG.lines().nth(1).unwrap()).unwrap();
        assert_eq!((r.unix, r.lat, r.lon), (1_792_817_548, 60.115_903, 31.371_963));
        assert!((r.altitude - 25.9).abs() < 1e-9 && (r.speed_kmh - 124.992).abs() < 1e-9);
        assert_eq!(r.acc, [-0.92, -0.22, 0.34]);
        assert_eq!(parse_line("0,V, 0.000000,      nan,-5,41,0,0,0,0,0,0,0"), None);
        assert_eq!(parse_line("$V02"), None);
    }

    #[test]
    fn a_clip_gets_its_lines_from_the_card_root() {
        let root = std::env::temp_dir().join(format!("rv-mai70log-{}", std::process::id()));
        let dir = root.join("Normal").join("Front");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(root.join("GPSData000001.txt"), LOG).unwrap();
        let clip = dir.join("NO20261024-154056-012644F.MP4");
        let recs = records_for_clip(&clip);
        assert_eq!(recs.iter().map(|r| r.unix - 1_792_817_548).collect::<Vec<_>>(), [0, 1, 3]);
        assert_eq!(records_for_clip(&dir.join("NO20261024-154356-012645F.MP4")).len(), 1);
        assert!(records_for_clip(&dir.join("2019_0223_133721_061.MP4")).is_empty());

        // The video's own fixes place the log: its second line is the video's fix at 10 s.
        let mut track = Track {
            start: None,
            gps: [(0.0, 60.115_903, 31.371_963, 2), (10.0, 60.116, 31.3725, 0), (12.0, 60.1161, 31.373, 0)]
                .into_iter()
                .map(|(time, lat, lon, warning)| GpsPoint { time, lat, lon, altitude: 0.0, speed_kmh: 120.0, course: 90.0, warning })
                .collect(),
            gsensor: vec![],
        };
        merge(&mut track, &recs, 180.0);
        let g: Vec<(f64, f64)> = track.gsensor.iter().map(|s| (s.time, s.x)).collect();
        assert_eq!(g, [(9.0, -0.92), (10.0, -0.9), (12.0, -0.95)]);
        assert_eq!(track.gps[1].altitude, 26.0);
        assert_eq!(track.gps[2].altitude, 26.1);

        // Without GPS in the video, the log's fixes are the track from the start of the clip.
        let mut empty = Track::default();
        merge(&mut empty, &recs, 180.0);
        assert_eq!(empty.gps.iter().map(|p| p.time).collect::<Vec<_>>(), [0.0, 1.0, 3.0]);
        assert_eq!(empty.gsensor.len(), 3);
        let _ = std::fs::remove_dir_all(&root);
    }
}
