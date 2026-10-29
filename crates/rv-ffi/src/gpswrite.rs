//! C interface to the GPS track writers (`rv_core::gps::write`): Export Track, Open in Google
//! Earth, the GPS converter, SRT subtitles of an export and the subtitle template preview.
//!
//! Settings shared by the jobs below (all optional):
//! ```json
//! { "user_shift": days, "imperial": false, "srt_template": "..." | null,
//!   "locale": { "decimal": ".", "time_sep": ":", "date_format": "dd.MM.yyyy", "time_format": "HH:mm:ss" },
//!   "words": { "Altitude": "...", "Meters": "...", ... } }   // by the original's translation ids
//! ```
//! A write job adds:
//! ```json
//! { "writer": "kml" | null, "output": "path" | null, "name": "..." | null,
//!   "sources": [ { "path": "GPS log or video", "from": TDateTime | null, "to": TDateTime | null,
//!                  "video_duration": seconds | null } ] }
//! ```
//! Sources are read whole, or cropped to [from, to]; a null writer only describes the track.
//! The result: `{ "ok", "error", "has_points", "name", "file_name", "start", "finish" }`.

use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::panic::catch_unwind;
use std::path::Path;

use rv_core::gps::{self, write};
use rv_core::{GpsPoint, Track};
use rv_fm::gps::crop;
use rv_fm::time::to_naive;
use serde_json::{Value, json};

use super::into_c_string;

fn json_arg(s: *const c_char) -> Option<Value> {
    if s.is_null() {
        return None;
    }
    serde_json::from_str(unsafe { CStr::from_ptr(s) }.to_str().ok()?).ok()
}

fn options(v: &Value) -> write::WriteOptions {
    let mut o = write::WriteOptions::local();
    // The app's own name; DATAKAM branding is not reused (decisions.md).
    o.creator = concat!("RV Player ", env!("CARGO_PKG_VERSION")).to_string();
    o.user_shift = v["user_shift"].as_f64().unwrap_or(0.0);
    o.imperial = v["imperial"].as_bool().unwrap_or(false);
    o.srt_template = v["srt_template"].as_str().map(str::to_string);
    let l = &v["locale"];
    let ch = |k: &str, d: char| l[k].as_str().and_then(|s| s.chars().next()).unwrap_or(d);
    o.locale.decimal_separator = ch("decimal", '.');
    o.locale.time_separator = ch("time_sep", ':');
    if let Some(f) = l["date_format"].as_str() {
        o.locale.date_format = f.to_string();
    }
    if let Some(f) = l["time_format"].as_str() {
        o.locale.time_format = f.to_string();
    }
    if let Some(words) = v["words"].as_object() {
        let map: HashMap<&str, &str> = words.iter().filter_map(|(k, v)| Some((k.as_str(), v.as_str()?))).collect();
        let english = write::Words::default();
        let fallback = write::Words::from_translation(|id| map.get(id).map_or_else(String::new, |s| (*s).to_string()));
        // An id the caller does not know keeps the English word.
        o.words = write::Words {
            altitude: pick(fallback.altitude, english.altitude),
            meters: pick(fallback.meters, english.meters),
            speed: pick(fallback.speed, english.speed),
            km_per_h: pick(fallback.km_per_h, english.km_per_h),
            bearing: pick(fallback.bearing, english.bearing),
            track_length: pick(fallback.track_length, english.track_length),
            track: pick(fallback.track, english.track),
            miles: pick(fallback.miles, english.miles),
            miles_per_h: pick(fallback.miles_per_h, english.miles_per_h),
        };
    }
    o
}

fn pick(given: String, default: String) -> String {
    if given.is_empty() { default } else { given }
}

fn write_job(v: &Value) -> Value {
    let opts = options(v);
    let mut info = write::TrackInfo::default();
    for s in v["sources"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let Some(path) = s["path"].as_str() else { continue };
        let Ok(log) = gps::load_path(Path::new(path)) else { continue };
        let track = match (s["from"].as_f64(), s["to"].as_f64()) {
            (Some(from), Some(to)) => crop(&log.track, from, to),
            _ => log.track,
        };
        if !track.is_empty() {
            info.add(&track, s["video_duration"].as_f64().filter(|d| *d > 0.0), &opts);
        }
    }
    if let Some(name) = v["name"].as_str() {
        info.name = name.to_string();
    }
    let mut r = json!({
        "ok": true, "error": null, "has_points": info.has_points(), "name": info.name,
        "file_name": info.default_file_name(), "start": info.start, "finish": info.finish
    });
    if let (Some(id), Some(out)) = (v["writer"].as_str(), v["output"].as_str()) {
        if !info.has_points() {
            r["ok"] = json!(false);
            r["error"] = json!("no GPS data");
        } else if let Err(e) = write::write_file(id, &info, &opts, Path::new(out)) {
            r["ok"] = json!(false);
            r["error"] = json!(e.to_string());
        }
    }
    r
}

/// The export formats in the original's order: `[{"id", "caption", "ext"}]`.
#[unsafe(no_mangle)]
pub extern "C" fn rv_gps_writers_json() -> *mut c_char {
    let v = catch_unwind(|| {
        Value::from(write::formats().iter().map(|f| json!({ "id": f.id, "caption": f.caption, "ext": f.ext })).collect::<Vec<_>>())
    })
    .unwrap_or_else(|_| Value::Array(Vec::new()));
    into_c_string(v.to_string())
}

/// Reads the sources of a write job and writes them as one track (see the module docs).
///
/// # Safety
/// `job_json` must be a valid NUL-terminated UTF-8 string or NULL. Release the result with
/// `rv_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_gps_write_json(job_json: *const c_char) -> *mut c_char {
    let v = match json_arg(job_json) {
        Some(job) => catch_unwind(|| write_job(&job)).unwrap_or_else(|_| json!({ "ok": false, "error": "internal error" })),
        None => json!({ "ok": false, "error": "invalid job" }),
    };
    into_c_string(v.to_string())
}

/// The subtitle template window: the default template and the cue text of one sample fix.
/// The job holds the shared settings plus `"time": TDateTime, "lat", "lng", "alt", "speed",
/// "bearing", "track"` (metres); the result is `{"default_template", "text"}`.
///
/// # Safety
/// As for `rv_gps_write_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_gps_srt_preview_json(job_json: *const c_char) -> *mut c_char {
    let v = json_arg(job_json).unwrap_or(Value::Null);
    let r = catch_unwind(|| {
        let opts = options(&v);
        let f = |k: &str| v[k].as_f64().unwrap_or(0.0);
        let track = Track {
            start: to_naive(f("time")),
            gps: vec![GpsPoint {
                time: 0.0,
                lat: f("lat"),
                lon: f("lng"),
                altitude: f("alt"),
                speed_kmh: f("speed"),
                course: f("bearing"),
                warning: 0,
            }],
            gsensor: Vec::new(),
        };
        let mut info = write::TrackInfo::default();
        info.add(&track, Some(1.0), &opts);
        if let Some(p) = info.points.first_mut() {
            p.track_pos = f("track");
        }
        let srt = write::render("srt", &info, &opts).unwrap_or_default();
        // The cue's text: the lines after its timing line.
        let text = srt.split("\r\n").skip_while(|l| !l.contains("-->")).skip(1).take_while(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
        json!({ "default_template": write::default_srt_template(opts.imperial, &opts.words), "text": text })
    })
    .unwrap_or_else(|_| json!({ "default_template": "", "text": "" }));
    into_c_string(r.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rv_string_free;
    use std::ffi::CString;

    fn call(f: unsafe extern "C" fn(*const c_char) -> *mut c_char, job: &Value) -> Value {
        let job = CString::new(job.to_string()).unwrap();
        let raw = unsafe { f(job.as_ptr()) };
        let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_string();
        unsafe { rv_string_free(raw) };
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn writers_are_listed_in_order() {
        let raw = rv_gps_writers_json();
        let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_string();
        unsafe { rv_string_free(raw) };
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v[0]["id"], "kml");
        assert_eq!(v[7]["ext"], "srt");
    }

    #[test]
    fn gpx_log_converts_to_kml() {
        let dir = std::env::temp_dir().join(format!("rv-ffi-gpswrite-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gpx = dir.join("trip.gpx");
        std::fs::write(
            &gpx,
            r#"<?xml version="1.0"?><gpx version="1.1" creator="t"><trk><trkseg>
<trkpt lat="60.0001" lon="30.0001"><time>2024-03-10T10:00:00Z</time></trkpt>
<trkpt lat="60.0002" lon="30.0002"><time>2024-03-10T10:00:01Z</time></trkpt>
<trkpt lat="60.0003" lon="30.0003"><time>2024-03-10T10:00:02Z</time></trkpt>
</trkseg></trk></gpx>"#,
        )
        .unwrap();
        let out = dir.join("trip.kml");
        let r = call(rv_gps_write_json, &json!({ "writer": "kml", "output": out, "sources": [ { "path": gpx } ] }));
        assert_eq!(r["ok"], true, "{r}");
        assert!(r["name"].as_str().unwrap().starts_with("Track "));
        assert!(std::fs::read_to_string(&out).unwrap().contains("<kml"));
        let none = call(rv_gps_write_json, &json!({ "writer": "kml", "output": out, "sources": [ { "path": dir.join("none.gpx") } ] }));
        assert_eq!(none["ok"], false);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn srt_preview_expands_the_template() {
        let r = call(rv_gps_srt_preview_json, &json!({ "srt_template": "%lat% %speed%", "time": 45000.5, "lat": 60.5, "lng": 30.25, "speed": 70.0 }));
        assert!(r["text"].as_str().unwrap().contains("N60.5"), "{r}");
        assert!(r["default_template"].as_str().unwrap().contains("%lat%"));
    }
}
