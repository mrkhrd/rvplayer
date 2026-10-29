//! C interface to the fragment exporter (`rv-edit`): a background job polled for its progress.
//!
//! The job JSON given to `rv_export_start`:
//! ```json
//! { "output": "...", "video": true, "audio": true, "meta": true, "trim_head_ms": 0, "trim_tail_ms": 0,
//!   "items": [ { "path": "...", "p1": 0.0, "p2": 1.0, "speed": 1.0, "speed_up_fps": true,
//!                "start": TDateTime | null, "track": "GPS file" | null } ] }
//! ```
//! `track` names a GPS log or sidecar for the meta track; null (or the video itself) uses the
//! video's own track. The status JSON:
//! ```json
//! { "stage": 0..5, "item": n | -1, "output_seconds": s, "fraction": f, "finished": bool,
//!   "ok": bool, "cancelled": bool, "error": null | "text", "problems": ["Cannot open file ..."],
//!   "warnings": [...], "t_min": TDateTime | null, "t_max": TDateTime | null, "duration": seconds }
//! ```

use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use rv_core::gps;
use rv_edit::{CropItem, ExportError, ExportOptions, Progress, Stage, export};
use rv_fm::time::{from_naive, to_naive};
use serde_json::{Value, json};

use super::into_c_string;

struct Shared {
    cancel: AtomicBool,
    status: Mutex<Value>,
}

/// A running or finished export.
pub struct RvExport {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for RvExport {
    fn drop(&mut self) {
        self.shared.cancel.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn stage_index(s: Stage) -> i64 {
    match s {
        Stage::Opening => 0,
        Stage::Meta1 => 1,
        Stage::Meta2 => 2,
        Stage::Meta3 => 3,
        Stage::Copying => 4,
        Stage::Finishing => 5,
    }
}

/// The meta track of an item from a GPS file other than the video.
fn track_of(video: &Path, source: Option<&str>) -> Option<rv_core::Track> {
    let source = Path::new(source.filter(|s| !s.is_empty())?);
    if source.to_string_lossy().eq_ignore_ascii_case(&video.to_string_lossy()) {
        return None;
    }
    gps::load_path(source).ok().map(|log| log.track)
}

fn parse_job(v: &Value) -> Option<(Vec<CropItem>, ExportOptions, PathBuf)> {
    let output = PathBuf::from(v["output"].as_str()?);
    let flag = |k: &str| v[k].as_bool().unwrap_or(true);
    let ms = |k: &str| v[k].as_u64().map_or(0, |x| u32::try_from(x).unwrap_or(u32::MAX));
    let opts = ExportOptions {
        video: flag("video"),
        audio: flag("audio"),
        meta: flag("meta"),
        trim_head_ms: ms("trim_head_ms"),
        trim_tail_ms: ms("trim_tail_ms"),
    };
    let items = v["items"]
        .as_array()?
        .iter()
        .filter_map(|it| {
            let path = PathBuf::from(it["path"].as_str()?);
            let track = if opts.meta { track_of(&path, it["track"].as_str()) } else { None };
            Some(CropItem {
                p1: it["p1"].as_f64().unwrap_or(0.0),
                p2: it["p2"].as_f64().unwrap_or(1.0),
                speed: it["speed"].as_f64().unwrap_or(1.0),
                speed_up_fps: it["speed_up_fps"].as_bool().unwrap_or(true),
                start: it["start"].as_f64().filter(|&t| t > 2.0).and_then(to_naive),
                track,
                path,
            })
        })
        .collect();
    Some((items, opts, output))
}

fn messages(problems: &[rv_edit::ItemProblem]) -> Value {
    Value::from(problems.iter().map(rv_edit::ItemProblem::message).collect::<Vec<_>>())
}

fn run(shared: &Shared, items: &[CropItem], opts: &ExportOptions, output: &Path) -> Value {
    let mut on_progress = |p: &Progress| {
        if let Ok(mut s) = shared.status.lock() {
            s["stage"] = json!(stage_index(p.stage));
            s["item"] = json!(p.item.map_or(-1, |i| i as i64));
            s["output_seconds"] = json!(p.output_seconds);
            s["fraction"] = json!(p.fraction);
        }
        !shared.cancel.load(Ordering::SeqCst)
    };
    let mut done = json!({
        "finished": true, "ok": false, "cancelled": false, "error": null,
        "problems": [], "warnings": [], "t_min": null, "t_max": null
    });
    match export(items, opts, output, &mut on_progress) {
        Ok(r) => {
            done["ok"] = json!(true);
            done["problems"] = messages(&r.problems);
            done["warnings"] = Value::from(r.warnings.clone());
            done["t_min"] = r.t_min.map_or(Value::Null, |t| json!(from_naive(t)));
            done["t_max"] = r.t_max.map_or(Value::Null, |t| json!(from_naive(t)));
            done["duration"] = json!(r.duration);
        }
        Err(ExportError::Cancelled) => done["cancelled"] = json!(true),
        Err(ExportError::NothingToExport { problems }) => done["problems"] = messages(&problems),
        Err(e) => done["error"] = json!(e.to_string()),
    }
    done
}

/// Starts an export on a background thread; NULL when the job JSON is invalid.
///
/// # Safety
/// `job_json` must be a valid NUL-terminated UTF-8 string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_export_start(job_json: *const c_char) -> *mut RvExport {
    if job_json.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(text) = unsafe { CStr::from_ptr(job_json) }.to_str() else { return std::ptr::null_mut() };
    let Ok(v) = serde_json::from_str::<Value>(text) else { return std::ptr::null_mut() };
    catch_unwind(|| {
        let Some((items, opts, output)) = parse_job(&v) else { return std::ptr::null_mut() };
        let shared = Arc::new(Shared {
            cancel: AtomicBool::new(false),
            status: Mutex::new(json!({ "stage": 0, "item": -1, "output_seconds": 0.0, "fraction": 0.0, "finished": false })),
        });
        let worker = shared.clone();
        let thread = std::thread::Builder::new().name("rv-export".into()).spawn(move || {
            let done = catch_unwind(AssertUnwindSafe(|| run(&worker, &items, &opts, &output)))
                .unwrap_or_else(|_| json!({ "finished": true, "ok": false, "error": "internal error" }));
            // The output gets the recording's times: a file of the same name read before
            // must not be taken for it.
            rv_core::infocache::forget(std::path::Path::new(&output));
            if let Ok(mut s) = worker.status.lock()
                && let (Value::Object(s), Value::Object(d)) = (&mut *s, done)
            {
                s.extend(d);
            }
        });
        match thread {
            Ok(t) => Box::into_raw(Box::new(RvExport { shared, thread: Some(t) })),
            Err(_) => std::ptr::null_mut(),
        }
    })
    .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `job` must come from `rv_export_start` and not have been freed, or be NULL. Release the
/// result with `rv_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_export_status_json(job: *const RvExport) -> *mut c_char {
    let v = match unsafe { job.as_ref() } {
        Some(j) => j
            .shared
            .status
            .lock()
            .map_or_else(|_| json!({ "finished": true, "error": "internal error" }), |s| s.clone()),
        None => json!({ "finished": true, "error": "no export" }),
    };
    into_c_string(v.to_string())
}

/// Asks the export to stop; a partly written output is removed.
///
/// # Safety
/// As for `rv_export_status_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_export_cancel(job: *const RvExport) {
    if let Some(j) = unsafe { job.as_ref() } {
        j.shared.cancel.store(true, Ordering::SeqCst);
    }
}

/// Cancels a running export, waits for its thread and frees it.
///
/// # Safety
/// `job` must come from `rv_export_start` and not have been freed already, or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_export_free(job: *mut RvExport) {
    if !job.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(job) })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rv_string_free;
    use std::ffi::CString;

    fn status(job: *const RvExport) -> Value {
        let raw = unsafe { rv_export_status_json(job) };
        let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_string();
        unsafe { rv_string_free(raw) };
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn bad_job_and_missing_source() {
        let bad = CString::new("not json").unwrap();
        assert!(unsafe { rv_export_start(bad.as_ptr()) }.is_null());
        let out = std::env::temp_dir().join(format!("rv-ffi-export-{}.mp4", std::process::id()));
        let missing = std::env::temp_dir().join("rv-ffi-export-missing.mp4");
        let job = json!({ "output": out, "items": [ { "path": missing, "p1": 0.0, "p2": 1.0 } ] });
        let job = CString::new(job.to_string()).unwrap();
        let h = unsafe { rv_export_start(job.as_ptr()) };
        assert!(!h.is_null());
        let s = loop {
            let s = status(h);
            if s["finished"] == true {
                break s;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(s["ok"], false);
        assert!(s["problems"][0].as_str().unwrap().starts_with("Cannot open file"), "{s}");
        unsafe { rv_export_free(h) };
        assert!(!out.exists());
    }
}
