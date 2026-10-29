//! C interface to the Fast repair engine (`rv-repair`): a background job polled for its status.
//!
//! The status JSON:
//! ```json
//! { "percent": 0..100, "frames": n, "finished": bool, "cancelled": bool, "error": null | "message" }
//! ```

use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};

use rv_repair::Repair;
use serde_json::json;

use super::{into_c_string, path_arg};

/// A running or finished repair.
pub struct RvRepair(Repair);

/// Starts repairing `broken` with `reference` (a good file from the same camera) on a
/// background thread. NULL when an argument is invalid or the thread cannot start.
///
/// # Safety
/// Both arguments must be valid NUL-terminated UTF-8 strings or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_repair_start(broken: *const c_char, reference: *const c_char) -> *mut RvRepair {
    let (Some(b), Some(r)) = (unsafe { path_arg(broken) }, unsafe { path_arg(reference) }) else {
        return std::ptr::null_mut();
    };
    catch_unwind(|| Repair::start(b, r).map_or(std::ptr::null_mut(), |j| Box::into_raw(Box::new(RvRepair(j))))).unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `job` must come from `rv_repair_start` and not have been freed, or be NULL. Release the
/// result with `rv_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_repair_status_json(job: *const RvRepair) -> *mut c_char {
    let v = match unsafe { job.as_ref() } {
        Some(j) => catch_unwind(AssertUnwindSafe(|| {
            let s = j.0.status();
            json!({
                "percent": s.percent,
                "frames": s.frames,
                "finished": s.finished,
                "cancelled": s.cancelled,
                "error": s.error.map(|e| e.to_string()),
            })
        }))
        .unwrap_or_else(|_| json!({ "finished": true, "error": "internal error" })),
        None => json!({ "finished": true, "error": "no repair" }),
    };
    into_c_string(v.to_string())
}

/// Asks the job to stop; the file stays unchanged unless writing had already begun.
///
/// # Safety
/// As for `rv_repair_status_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_repair_cancel(job: *const RvRepair) {
    if let Some(j) = unsafe { job.as_ref() } {
        let _ = catch_unwind(AssertUnwindSafe(|| j.0.cancel()));
    }
}

/// Cancels a running job, waits for its thread and frees it.
///
/// # Safety
/// `job` must come from `rv_repair_start` and not have been freed already, or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_repair_free(job: *mut RvRepair) {
    if !job.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(job) })));
    }
}

/// SF-REPAIR-011: 1 when the file holds encrypted media (MOVCheckEncrypted); a file that cannot
/// be read counts as not encrypted, as in the original.
///
/// # Safety
/// `path` must be a valid NUL-terminated UTF-8 string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_repair_is_encrypted(path: *const c_char) -> i32 {
    let Some(p) = (unsafe { path_arg(path) }) else { return 0 };
    catch_unwind(|| i32::from(rv_repair::is_encrypted(&p).unwrap_or(false))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rv_string_free;
    use std::ffi::{CStr, CString};

    #[test]
    fn invalid_arguments_and_missing_files() {
        assert!(unsafe { rv_repair_start(std::ptr::null(), std::ptr::null()) }.is_null());
        let missing = CString::new(r"Z:\no\such\file.mp4").unwrap();
        assert_eq!(unsafe { rv_repair_is_encrypted(missing.as_ptr()) }, 0);
        // A job on missing files finishes with an error.
        let job = unsafe { rv_repair_start(missing.as_ptr(), missing.as_ptr()) };
        assert!(!job.is_null());
        let status = loop {
            let raw = unsafe { rv_repair_status_json(job) };
            let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_string();
            unsafe { rv_string_free(raw) };
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            if v["finished"].as_bool() == Some(true) {
                break v;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert!(status["error"].is_string());
        assert_eq!(status["frames"], 0);
        unsafe { rv_repair_free(job) };
    }
}
