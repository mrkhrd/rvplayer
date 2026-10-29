//! What [`FileInfo::load`] found in each video, kept across runs: reading a folder of large
//! recordings again (from a memory card above all) then takes no time. An entry is used while
//! the file has the same size and write time and, for a 70mai clip, its card's trip logs are
//! unchanged ([`gps::mai70log`]).
//!
//! The cache is a JSON file chosen by the application ([`set_file`]); without one, entries are
//! kept for the run only. What rewrites a file itself (repair, export) calls [`forget`]. It is written by [`flush`], and at most every [`WRITE_INTERVAL`] as
//! entries are added.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use chrono::NaiveDateTime;
use serde_json::{Value, json};

use crate::fileinfo::FileInfo;
use crate::gps;
use crate::track::{GSensorSample, GpsPoint, Track};

/// Raise when what [`FileInfo::load`] reads changes, so that old entries are read again.
const VERSION: u32 = 1;
/// Least-recently used entries above this are dropped.
const MAX_ENTRIES: usize = 20_000;
pub const WRITE_INTERVAL: Duration = Duration::from_secs(10);

const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.f";

struct Entry {
    size: u64,
    /// Write time, milliseconds since 1970.
    modified: u128,
    logs: String,
    used: u64,
    info: FileInfo,
}

#[derive(Default)]
struct State {
    file: Option<PathBuf>,
    entries: HashMap<String, Entry>,
    clock: u64,
    dirty: bool,
    written: Option<Instant>,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn lock() -> std::sync::MutexGuard<'static, State> {
    state().lock().unwrap_or_else(|e| e.into_inner())
}

/// Size, write time and trip-log signature of a file now; `None` if it cannot be read.
fn stamp(path: &Path) -> Option<(u64, u128, String)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_millis();
    let name = path.file_name()?.to_string_lossy();
    let logs = if gps::mai70log::is_clip_name(&name) {
        gps::mai70log::signature(&gps::mai70log::logs_for_video(path))
    } else {
        String::new()
    };
    Some((meta.len(), modified, logs))
}

fn key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// Keeps the cache in `file` from now on (reading what it has), or for the run only (`None`).
pub fn set_file(file: Option<PathBuf>) {
    let entries = file.as_deref().map(read_file).unwrap_or_default();
    let mut s = lock();
    s.file = file;
    s.clock = entries.values().map(|e| e.used).max().unwrap_or(0);
    s.entries = entries;
    s.dirty = false;
}

/// [`FileInfo::load`], from the cache when the file is unchanged.
pub fn load(path: &Path) -> FileInfo {
    let Some((size, modified, logs)) = stamp(path) else { return FileInfo::load(path) };
    let k = key(path);
    {
        let mut s = lock();
        s.clock += 1;
        let clock = s.clock;
        if let Some(e) = s.entries.get_mut(&k)
            && e.size == size
            && e.modified == modified
            && e.logs == logs
        {
            e.used = clock;
            let mut info = e.info.clone();
            info.path = path.to_path_buf();
            return info;
        }
    }
    let info = FileInfo::load(path);
    let mut s = lock();
    s.clock += 1;
    let used = s.clock;
    s.entries.insert(k, Entry { size, modified, logs, used, info: info.clone() });
    s.dirty = true;
    if s.entries.len() > MAX_ENTRIES {
        let mut by_use: Vec<(u64, String)> = s.entries.iter().map(|(k, e)| (e.used, k.clone())).collect();
        by_use.sort();
        for (_, k) in by_use.into_iter().take(s.entries.len() - MAX_ENTRIES) {
            s.entries.remove(&k);
        }
    }
    if s.written.is_none_or(|t| t.elapsed() >= WRITE_INTERVAL) {
        write(&mut s);
    }
    info
}

/// Drops what is kept for a file: for files the program rewrites itself (repair, export),
/// which may keep their size and get their old write time back.
pub fn forget(path: &Path) {
    let mut s = lock();
    if s.entries.remove(&key(path)).is_some() {
        s.dirty = true;
    }
}

/// Writes the cache file if anything changed since it was last written.
pub fn flush() {
    write(&mut lock());
}

fn write(s: &mut State) {
    s.written = Some(Instant::now());
    let Some(file) = s.file.clone().filter(|_| s.dirty) else { return };
    let entries: Vec<Value> = s
        .entries
        .iter()
        .map(|(k, e)| json!({ "path": k, "size": e.size, "modified": e.modified, "logs": e.logs, "used": e.used, "info": info_json(&e.info) }))
        .collect();
    let text = json!({ "version": VERSION, "entries": entries }).to_string();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // A new file then a rename: a crash while writing leaves the old one.
    let tmp = file.with_extension("tmp");
    if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, &file).is_ok() {
        s.dirty = false;
    }
}

fn read_file(file: &Path) -> HashMap<String, Entry> {
    let Some(v) = std::fs::read(file).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
        return HashMap::new();
    };
    if v["version"].as_u64() != Some(u64::from(VERSION)) {
        return HashMap::new();
    }
    let mut out = HashMap::new();
    for e in v["entries"].as_array().into_iter().flatten() {
        let (Some(k), Some(size), Some(modified)) = (e["path"].as_str(), e["size"].as_u64(), e["modified"].as_u64()) else {
            continue;
        };
        let info = info_from_json(&e["info"], Path::new(k));
        let entry = Entry {
            size,
            modified: u128::from(modified),
            logs: e["logs"].as_str().unwrap_or_default().to_string(),
            used: e["used"].as_u64().unwrap_or(0),
            info,
        };
        out.insert(k.to_string(), entry);
    }
    out
}

fn time_json(t: Option<NaiveDateTime>) -> Value {
    t.map_or(Value::Null, |t| Value::from(t.format(TIME_FORMAT).to_string()))
}

fn info_json(f: &FileInfo) -> Value {
    let t = &f.track;
    json!({
        "name": f.name, "size": f.size, "duration": f.duration, "start": time_json(f.start),
        "width": f.width, "height": f.height, "fps": f.fps, "kbps": f.kbps, "gop": f.gop,
        "h264": f.h264, "aac": f.aac, "camera": f.camera, "gps_format": f.gps_format,
        "gps_error": f.gps_error, "error": f.error, "track_start": time_json(t.start),
        "gps": t.gps.iter().map(|p| json!([p.time, p.lat, p.lon, p.altitude, p.speed_kmh, p.course, p.warning])).collect::<Vec<_>>(),
        "gsensor": t.gsensor.iter().map(|g| json!([g.time, g.x, g.y, g.z])).collect::<Vec<_>>(),
    })
}

fn info_from_json(v: &Value, path: &Path) -> FileInfo {
    let s = |k: &str| v[k].as_str().map(str::to_string);
    let f = |k: &str| v[k].as_f64().unwrap_or(0.0);
    let u = |k: &str| v[k].as_u64().unwrap_or(0);
    let t = |k: &str| v[k].as_str().and_then(|t| NaiveDateTime::parse_from_str(t, TIME_FORMAT).ok());
    let row = |r: &Value, i: usize| r[i].as_f64().unwrap_or(0.0);
    let rows = |k: &str| v[k].as_array().cloned().unwrap_or_default();
    FileInfo {
        path: path.to_path_buf(),
        name: s("name").unwrap_or_default(),
        size: u("size"),
        duration: f("duration"),
        start: t("start"),
        width: u("width") as u32,
        height: u("height") as u32,
        fps: f("fps"),
        kbps: f("kbps"),
        gop: u("gop") as u32,
        h264: v["h264"].as_bool().unwrap_or(false),
        aac: v["aac"].as_bool().unwrap_or(false),
        camera: s("camera"),
        gps_format: s("gps_format"),
        gps_error: s("gps_error"),
        error: s("error"),
        track: Track {
            start: t("track_start"),
            gps: rows("gps")
                .iter()
                .map(|r| GpsPoint {
                    time: row(r, 0),
                    lat: row(r, 1),
                    lon: row(r, 2),
                    altitude: row(r, 3),
                    speed_kmh: row(r, 4),
                    course: row(r, 5),
                    warning: r[6].as_u64().unwrap_or(0) as u8,
                })
                .collect(),
            gsensor: rows("gsensor")
                .iter()
                .map(|r| GSensorSample { time: row(r, 0), x: row(r, 1), y: row(r, 2), z: row(r, 3) })
                .collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{GpsFix, write_rvmi_movie};

    /// One test for the whole cache: it is a single state for the process.
    #[test]
    fn entries_survive_a_restart_and_follow_the_file() {
        let dir = std::env::temp_dir().join(format!("rv-infocache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let video = dir.join("a.mov");
        let cache = dir.join("cache.json");
        let fixes = [GpsFix { lat: 60.0, lon: 30.0, speed_kmh: 50.0, course: 90.0 }];
        write_rvmi_movie(&video, 42413.5, &fixes).unwrap();

        set_file(Some(cache.clone()));
        let first = load(&video);
        assert!(first.error.is_none() && first.track.gps.len() == 1);
        flush();
        assert!(cache.exists());

        // A new run reads the file: the entry is used as it was stored.
        set_file(Some(cache.clone()));
        assert_eq!(lock().entries.len(), 1);
        let again = load(&video);
        assert_eq!(again.duration, first.duration);
        assert_eq!(again.start, first.start);
        assert_eq!(again.track, first.track);
        assert_eq!((again.fps, again.gop, again.gps_format.clone()), (first.fps, first.gop, first.gps_format.clone()));

        // A file rewritten with its old size and time is read again once forgotten.
        let entry_count = lock().entries.len();
        forget(&video);
        assert_eq!(lock().entries.len(), entry_count - 1);

        // A changed file is read again.
        let two = [
            GpsFix { lat: 60.0, lon: 30.0, speed_kmh: 50.0, course: 90.0 },
            GpsFix { lat: 60.001, lon: 30.0, speed_kmh: 50.0, course: 0.0 },
        ];
        write_rvmi_movie(&video, 42413.5, &two).unwrap();
        assert_eq!(load(&video).track.gps.len(), 2);

        set_file(None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
