//! C interface to `rv-core` and `rv-video` for the Qt application; see `include/rv_ffi.h`.
//!
//! File information crosses the boundary as JSON:
//! ```json
//! { "path": "...", "name": "...", "size": 123, "duration": 73.5,
//!   "start": "2016-02-13T18:39:24.044" | null, "width": 2304, "height": 1296,
//!   "camera": "AMBARELLA A7L" | null, "error": null | "message",
//!   "distance_km": 2.01, "avg_speed_kmh": 102.1, "max_speed_kmh": 112.5,
//!   "gps": [[time, lat, lon, altitude, speed_kmh, course, warning], ...],
//!   "gsensor": [[time, x, y, z], ...] }
//! ```
//! Every function catches panics so that none unwinds into C++.

pub mod fm;

use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use rv_core::FileInfo;
use rv_core::fileinfo::list_videos;
use rv_video::player::{Event, Player};
use serde_json::{Value, json};

pub const EVENT_NONE: i32 = 0;
pub const EVENT_OPENED: i32 = 1;
pub const EVENT_FRAME: i32 = 2;
pub const EVENT_END_OF_STREAM: i32 = 3;
pub const EVENT_ERROR: i32 = 4;

#[repr(C)]
pub struct RvEvent {
    pub kind: i32,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    pub time: f64,
    pub key: i32,
    pub rgba: *mut u8,
    pub rgba_len: usize,
    pub duration: f64,
    pub frame_rate: f64,
    pub error: *mut c_char,
}

impl RvEvent {
    fn empty() -> Self {
        Self {
            kind: EVENT_NONE,
            generation: 0,
            width: 0,
            height: 0,
            time: 0.0,
            key: 0,
            rgba: std::ptr::null_mut(),
            rgba_len: 0,
            duration: 0.0,
            frame_rate: 0.0,
            error: std::ptr::null_mut(),
        }
    }
}

/// Opaque handle for C.
pub struct RvPlayer(Player);

/// # Safety
/// `path` must be a valid NUL-terminated UTF-8 string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_file_info_json(path: *const c_char) -> *mut c_char {
    let json = catch_unwind(|| match unsafe { path_arg(path) } {
        Some(p) => file_info_json(&FileInfo::load(&p)),
        None => json!({ "error": "invalid path" }),
    })
    .unwrap_or_else(|_| json!({ "error": "internal error" }));
    into_c_string(json.to_string())
}

/// # Safety
/// `dir` must be a valid NUL-terminated UTF-8 string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_list_videos_json(dir: *const c_char) -> *mut c_char {
    let json = catch_unwind(|| {
        let paths = unsafe { path_arg(dir) }
            .and_then(|d| list_videos(&d).ok())
            .unwrap_or_default();
        Value::from(paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>())
    })
    .unwrap_or_else(|_| Value::Array(Vec::new()));
    into_c_string(json.to_string())
}

/// # Safety
/// `s` must come from an `rv_*` function and not have been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// # Safety
/// `path` must be a valid NUL-terminated UTF-8 string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_player_open(path: *const c_char, start: f64) -> *mut RvPlayer {
    let Some(p) = (unsafe { path_arg(path) }) else { return std::ptr::null_mut() };
    catch_unwind(|| Box::into_raw(Box::new(RvPlayer(Player::open(p, start))))).unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `player` must come from `rv_player_open` and not have been closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_player_seek(player: *mut RvPlayer, time: f64) {
    if let Some(p) = unsafe { player.as_mut() } {
        let _ = catch_unwind(AssertUnwindSafe(|| p.0.seek(time)));
    }
}

/// # Safety
/// `player` must come from `rv_player_open` and not have been closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_player_generation(player: *const RvPlayer) -> u64 {
    unsafe { player.as_ref() }.map_or(0, |p| p.0.generation())
}

/// # Safety
/// `player` must come from `rv_player_open` and not have been closed; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_player_poll(player: *mut RvPlayer, out: *mut RvEvent) -> i32 {
    let (Some(p), false) = (unsafe { player.as_mut() }, out.is_null()) else { return 0 };
    let event = catch_unwind(AssertUnwindSafe(|| p.0.poll())).unwrap_or(None);
    let mut e = RvEvent::empty();
    match event {
        None => {
            unsafe { out.write(e) };
            return 0;
        }
        Some(Event::Opened { width, height, duration, frame_rate }) => {
            e.kind = EVENT_OPENED;
            (e.width, e.height, e.duration, e.frame_rate) = (width, height, duration, frame_rate);
        }
        Some(Event::Frame { generation, frame }) => {
            e.kind = EVENT_FRAME;
            e.generation = generation;
            (e.width, e.height, e.time, e.key) = (frame.width, frame.height, frame.time, i32::from(frame.key));
            let buf = frame.rgba.into_boxed_slice();
            e.rgba_len = buf.len();
            e.rgba = Box::into_raw(buf).cast::<u8>();
        }
        Some(Event::EndOfStream { generation }) => {
            e.kind = EVENT_END_OF_STREAM;
            e.generation = generation;
        }
        Some(Event::Error(msg)) => {
            e.kind = EVENT_ERROR;
            e.error = into_c_string(msg);
        }
    }
    unsafe { out.write(e) };
    1
}

/// # Safety
/// `player` must come from `rv_player_open` and not have been closed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_player_close(player: *mut RvPlayer) {
    if !player.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(player) })));
    }
}

/// # Safety
/// `rgba`/`len` must come from an `RvEvent` frame and not have been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_frame_free(rgba: *mut u8, len: usize) {
    if !rgba.is_null() {
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(rgba, len)) });
    }
}

/// # Safety
/// `s` must be a valid NUL-terminated string or NULL.
unsafe fn path_arg(s: *const c_char) -> Option<PathBuf> {
    if s.is_null() {
        return None;
    }
    let s = unsafe { CStr::from_ptr(s) }.to_str().ok()?;
    (!s.is_empty()).then(|| Path::new(s).to_path_buf())
}

fn into_c_string(s: String) -> *mut c_char {
    CString::new(s.replace('\0', "")).map_or(std::ptr::null_mut(), CString::into_raw)
}

pub fn file_info_json(f: &FileInfo) -> Value {
    let t = &f.track;
    json!({
        "path": f.path.to_string_lossy(),
        "name": f.name,
        "size": f.size,
        "duration": f.duration,
        "start": f.start.map(|d| d.format("%Y-%m-%dT%H:%M:%S%.3f").to_string()),
        "width": f.width,
        "height": f.height,
        "camera": f.camera,
        "error": f.error,
        "distance_km": t.distance_km(),
        "avg_speed_kmh": t.avg_speed_kmh(),
        "max_speed_kmh": t.max_speed_kmh(),
        "gps": t.gps.iter().map(|p| json!([p.time, p.lat, p.lon, p.altitude, p.speed_kmh, p.course, p.warning])).collect::<Vec<_>>(),
        "gsensor": t.gsensor.iter().map(|g| json!([g.time, g.x, g.y, g.z])).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_core::testutil::{GpsFix, write_rvmi_movie};

    #[test]
    fn file_info_round_trip_through_c_strings() {
        let dir = std::env::temp_dir().join(format!("rv-ffi-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.mov");
        let fixes = [GpsFix { lat: 60.0, lon: 30.0, speed_kmh: 50.0, course: 90.0 }];
        write_rvmi_movie(&file, 42413.5, &fixes).unwrap();

        let c_path = CString::new(file.to_str().unwrap()).unwrap();
        let raw = unsafe { rv_file_info_json(c_path.as_ptr()) };
        let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_string();
        unsafe { rv_string_free(raw) };
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["name"], "a.mov");
        assert!(v["error"].is_null());
        assert_eq!(v["gps"][0][1], 60.0);
        assert_eq!(v["gps"][0][4], 50.0);

        let c_dir = CString::new(dir.to_str().unwrap()).unwrap();
        let raw = unsafe { rv_list_videos_json(c_dir.as_ptr()) };
        let list: Value = serde_json::from_str(unsafe { CStr::from_ptr(raw) }.to_str().unwrap()).unwrap();
        unsafe { rv_string_free(raw) };
        assert_eq!(list.as_array().unwrap().len(), 1);
    }

    #[test]
    fn null_arguments_are_safe() {
        let raw = unsafe { rv_file_info_json(std::ptr::null()) };
        assert!(!raw.is_null());
        unsafe { rv_string_free(raw) };
        assert!(unsafe { rv_player_open(std::ptr::null(), 0.0) }.is_null());
        let mut e = RvEvent::empty();
        assert_eq!(unsafe { rv_player_poll(std::ptr::null_mut(), &mut e) }, 0);
        unsafe { rv_player_close(std::ptr::null_mut()) };
    }
}
