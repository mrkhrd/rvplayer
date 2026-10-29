//! File manager entry points (`rv_fm_*` in `include/rv_ffi.h`), backed by `rv_fm::FileManager`
//! (spec `docs/spec/file-list.md`). Times are Delphi TDateTime values (days since 1899-12-30,
//! local time; <= 2.0 means "no date"); durations are in days unless named `_ms`.
//!
//! `rv_fm_rows_json` returns the rows in the current sort order and the status:
//! ```json
//! { "status_key": "" | "Initializing" | "Searching" | "Preview" | "Grouping_Title",
//!   "status_arg": "C:\\DCIM\\" (folder for Searching) | "42" (percent for Preview) | "",
//!   "any_locked": false,
//!   "rows": [ {
//!     "name": "FILE0001-3",              // display name (group name for groups)
//!     "path": "C:\\v\\FILE0001.MOV, C:\\v\\FILE0002.MOV, C:\\v\\FILE0003.MOV",
//!     "files": 3,                        // number of files in the item
//!     "lock": 0,                         // 0 none, 1 all files read-only, 2 some
//!     "duration_ms": 180000,             // sum of the files' durations
//!     "duration_state": 3,               // 0 unknown, 1 estimated ("~"), 2 parser failed, 3 exact
//!     "finish": 45361.52,                // TDateTime; start = finish - duration
//!     "duration": 0.00215,               // days, wall-clock span of the item
//!     "size": 314572800,                 // bytes (i64)
//!     "width": 1920, "height": 1080,     // maximum over the files; 0 when unknown
//!     "track_m": 2014.5,                 // metres
//!     "avg_speed": 54.2,                 // km/h; <= 0 means none (-1 for groups without GPS)
//!     "gps": 2                           // 0 none, 1 points without a valid fix, 2 valid fix
//!   } ] }
//! ```
//! The status row is not part of `rows`: the UI shows it while `status_key` is non-empty.
//!
//! `rv_fm_item_json` returns one item: the same fields as a row except that `files` is the list
//! of its files in group order (by start time):
//! ```json
//! { "name": ..., "path": ..., "lock": ..., "duration_ms": ..., "duration_state": ...,
//!   "finish": ..., "duration": ..., "size": ..., "width": ..., "height": ...,
//!   "track_m": ..., "avg_speed": ..., "gps": ...,
//!   "files": [ {
//!     "path": "C:\\v\\FILE0001.MOV", "face": "FILE0001",
//!     "size": 104857600, "lock": 0,      // 0/1: read-only attribute
//!     "duration_ms": 60000, "duration_state": 3,
//!     "duration": 0.000694, "finish": 45361.52,
//!     "p_start": 0.0, "p_finish": 0.3226,  // the file's window as fractions of the item span
//!     "width": 1920, "height": 1080, "fps": 30.0, "gop": 15,
//!     "no_group": false,                 // never grouped automatically
//!     "fs_creation": 45361.51, "fs_write": 45361.52,
//!     "info_path": "C:\\v\\FILE0001.MOV", // GPS info: the video itself (embedded) or a sidecar; "" none
//!     "gps_state": 2,
//!     "gps_log_path": ""                 // pool log assigned by time overlap; "" none
//!   } ] }
//! ```
//! `"null"` when the index is out of range.

use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};

use rv_fm::{FileEntry, FileManager, Item};
use serde_json::{Value, json};

use super::{into_c_string, path_arg};

/// Opaque handle for C.
pub struct RvFileManager(FileManager);

fn summary_fields(g: &FileEntry) -> serde_json::Map<String, Value> {
    let v = json!({
        "name": g.video.face,
        "path": g.video.path,
        "lock": g.video.lock,
        "duration_ms": g.video.duration_ms,
        "duration_state": g.video.duration_state,
        "finish": g.video.time.finish,
        "duration": g.video.time.duration,
        "size": g.video.time.size,
        "width": g.video.width,
        "height": g.video.height,
        "track_m": g.info.track_length,
        "avg_speed": g.info.avg_speed,
        "gps": g.info.gps,
    });
    match v {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}

fn file_json(f: &FileEntry) -> Value {
    let v = &f.video;
    json!({
        "path": v.path,
        "face": v.face,
        "size": v.time.size,
        "lock": v.lock,
        "duration_ms": v.duration_ms,
        "duration_state": v.duration_state,
        "duration": v.time.duration,
        "finish": v.time.finish,
        "p_start": v.time.p_start,
        "p_finish": v.time.p_finish,
        "width": v.width,
        "height": v.height,
        "fps": v.fps,
        "gop": v.gop,
        "no_group": v.no_group,
        "fs_creation": v.time.creation,
        "fs_write": v.time.write,
        "info_path": f.info.path,
        "gps_state": f.info.gps,
        "gps_log_path": f.gps_log.path,
    })
}

pub fn rows_json(fm: &FileManager) -> Value {
    let r = fm.rows();
    let rows: Vec<Value> = r
        .rows
        .iter()
        .map(|row| {
            let mut m = summary_fields(&row.summary);
            m.insert("files".into(), json!(row.files));
            Value::Object(m)
        })
        .collect();
    json!({ "status_key": r.status_key, "status_arg": r.status_arg, "any_locked": r.any_locked, "rows": rows })
}

pub fn item_json(it: &Item) -> Value {
    let mut m = summary_fields(&it.group);
    m.insert("files".into(), Value::Array(it.files.iter().map(file_json).collect()));
    Value::Object(m)
}

fn empty_rows_json() -> Value {
    json!({ "status_key": "", "status_arg": "", "any_locked": false, "rows": [] })
}

/// # Safety
/// `indices` must point to `count` readable `int32_t` values, or be NULL.
unsafe fn indices_arg(indices: *const i32, count: i32) -> Vec<usize> {
    if indices.is_null() || count <= 0 {
        return Vec::new();
    }
    let s = unsafe { std::slice::from_raw_parts(indices, count as usize) };
    s.iter().filter(|&&i| i >= 0).map(|&i| i as usize).collect()
}

/// Runs `f` on the manager, catching panics.
///
/// # Safety
/// `fm` must come from `rv_fm_create` and not have been destroyed, or be NULL.
unsafe fn with_fm<R>(fm: *const RvFileManager, default: R, f: impl FnOnce(&FileManager) -> R) -> R {
    match unsafe { fm.as_ref() } {
        Some(m) => catch_unwind(AssertUnwindSafe(|| f(&m.0))).unwrap_or(default),
        None => default,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rv_fm_create(auto_search: i32, gps: i32) -> *mut RvFileManager {
    catch_unwind(|| Box::into_raw(Box::new(RvFileManager(FileManager::new(auto_search != 0, gps != 0)))))
        .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `fm` must come from `rv_fm_create` and not have been destroyed already, or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_destroy(fm: *mut RvFileManager) {
    if !fm.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(fm) })));
    }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`; `path` a valid NUL-terminated UTF-8 string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_add(fm: *mut RvFileManager, path: *const c_char) {
    let Some(p) = (unsafe { path_arg(path) }) else { return };
    unsafe { with_fm(fm, (), |m| m.add(&p.to_string_lossy())) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_clear(fm: *mut RvFileManager) {
    unsafe { with_fm(fm, (), FileManager::clear) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_set_sort(fm: *mut RvFileManager, column: i32, ascending: i32) {
    unsafe { with_fm(fm, (), |m| m.set_sort(column, ascending != 0)) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_version(fm: *const RvFileManager) -> u64 {
    unsafe { with_fm(fm, 0, FileManager::version) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_is_idle(fm: *const RvFileManager) -> i32 {
    unsafe { with_fm(fm, 0, |m| i32::from(m.is_idle())) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`. Release the result with `rv_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_rows_json(fm: *mut RvFileManager) -> *mut c_char {
    let v = unsafe { with_fm(fm, empty_rows_json(), rows_json) };
    into_c_string(v.to_string())
}

/// # Safety
/// `fm` as for `rv_fm_destroy`. Release the result with `rv_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_item_json(fm: *mut RvFileManager, index: i32) -> *mut c_char {
    let v = unsafe {
        with_fm(fm, Value::Null, |m| {
            usize::try_from(index).ok().and_then(|i| m.item(i)).map_or(Value::Null, |it| item_json(&it))
        })
    };
    into_c_string(v.to_string())
}

/// # Safety
/// `fm` as for `rv_fm_destroy`; `indices` points to `count` values or is NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_remove(fm: *mut RvFileManager, indices: *const i32, count: i32) {
    let idx = unsafe { indices_arg(indices, count) };
    unsafe { with_fm(fm, (), |m| m.remove(&idx)) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`; `indices` points to `count` values or is NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_set_lock(fm: *mut RvFileManager, indices: *const i32, count: i32, lock: i32) {
    let idx = unsafe { indices_arg(indices, count) };
    unsafe { with_fm(fm, (), |m| m.set_lock(&idx, lock != 0)) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`; `indices` points to `count` values or is NULL. The first index
/// is the target the others are merged into.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_group(fm: *mut RvFileManager, indices: *const i32, count: i32) {
    let idx = unsafe { indices_arg(indices, count) };
    unsafe { with_fm(fm, (), |m| m.group(&idx)) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`; `indices` points to `count` values or is NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_ungroup(fm: *mut RvFileManager, indices: *const i32, count: i32) {
    let idx = unsafe { indices_arg(indices, count) };
    unsafe { with_fm(fm, (), |m| m.ungroup(&idx)) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_ungroup_all(fm: *mut RvFileManager) {
    unsafe { with_fm(fm, (), FileManager::ungroup_all) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_group_auto(fm: *mut RvFileManager) {
    unsafe { with_fm(fm, (), FileManager::group_auto) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`. Release the result with `rv_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_log_files_json(fm: *mut RvFileManager) -> *mut c_char {
    let v = unsafe { with_fm(fm, Value::Array(Vec::new()), |m| Value::from(m.log_files())) };
    into_c_string(v.to_string())
}

/// # Safety
/// `fm` as for `rv_fm_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_shift_video(fm: *mut RvFileManager, index: i32, days: f64) {
    let Ok(i) = usize::try_from(index) else { return };
    unsafe { with_fm(fm, (), |m| m.shift_video(i, days)) }
}

/// # Safety
/// `fm` as for `rv_fm_destroy`; `path` a valid NUL-terminated UTF-8 string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rv_fm_set_track(fm: *mut RvFileManager, index: i32, path: *const c_char) -> i32 {
    let (Ok(i), Some(p)) = (usize::try_from(index), unsafe { path_arg(path) }) else { return 0 };
    unsafe { with_fm(fm, 0, |m| i32::from(m.set_track(i, &p.to_string_lossy()))) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rv_string_free;
    use rv_core::testutil::movie_with_track;
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::time::{Duration, Instant, SystemTime};

    fn take_json(raw: *mut c_char) -> Value {
        assert!(!raw.is_null());
        let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_string();
        unsafe { rv_string_free(raw) };
        serde_json::from_str(&text).unwrap()
    }

    fn write_segment(path: &std::path::Path, start_s: u64) {
        fs::write(path, movie_with_track(b"vide", b"avc1", 1, &vec![vec![0u8; 16]; 60])).unwrap();
        let c = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_064_800 + start_s);
        let mut t = fs::FileTimes::new().set_modified(c + Duration::from_secs(60));
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileTimesExt;
            t = t.set_created(c);
        }
        fs::File::options().write(true).open(path).unwrap().set_times(t).unwrap();
    }

    fn wait_idle(fm: *mut RvFileManager) {
        let end = Instant::now() + Duration::from_secs(30);
        while unsafe { rv_fm_is_idle(fm) } == 0 {
            assert!(Instant::now() < end, "manager did not become idle");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn file_manager_round_trip_through_c() {
        let dir = std::env::temp_dir().join(format!("rv-ffi-fm-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        write_segment(&dir.join("FILE0001.MOV"), 0);
        write_segment(&dir.join("FILE0002.MOV"), 60);
        write_segment(&dir.join("OTHER.MOV"), 100_000);

        let fm = rv_fm_create(0, 1);
        assert!(!fm.is_null());
        let c_dir = CString::new(dir.to_str().unwrap()).unwrap();
        let v0 = unsafe { rv_fm_version(fm) };
        unsafe { rv_fm_add(fm, c_dir.as_ptr()) };
        assert!(unsafe { rv_fm_version(fm) } > v0);
        wait_idle(fm);

        let rows = take_json(unsafe { rv_fm_rows_json(fm) });
        assert_eq!(rows["status_key"], "");
        assert_eq!(rows["status_arg"], "");
        assert_eq!(rows["any_locked"], false);
        let list = rows["rows"].as_array().unwrap();
        assert_eq!(list.len(), 2, "{rows}");
        let r0 = &list[0];
        let mut keys: Vec<&str> = r0.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["avg_speed", "duration", "duration_ms", "duration_state", "files", "finish", "gps", "height", "lock", "name", "path", "size", "track_m", "width"]
        );
        assert_eq!(r0["name"], "FILE0001-2");
        assert_eq!(r0["files"], 2);
        assert_eq!(r0["duration_ms"], 120_000);
        assert_eq!(r0["duration_state"], 3);
        assert_eq!((r0["width"].as_u64(), r0["height"].as_u64()), (Some(1920), Some(1080)));
        assert_eq!(list[1]["name"], "OTHER");

        let item = take_json(unsafe { rv_fm_item_json(fm, 0) });
        let files = item["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        let mut keys: Vec<&str> = files[0].as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "duration", "duration_ms", "duration_state", "face", "finish", "fps", "fs_creation", "fs_write",
                "gop", "gps_log_path", "gps_state", "height", "info_path", "lock", "no_group", "p_finish",
                "p_start", "path", "size", "width"
            ]
        );
        assert_eq!(files[0]["face"], "FILE0001");
        assert!(files[0]["p_start"].as_f64().unwrap().abs() < 1e-6);
        assert_eq!(files[1]["p_finish"], 1.0);
        assert!((files[0]["p_finish"].as_f64().unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(item["name"], "FILE0001-2");
        assert!(item.get("rows").is_none());
        assert_eq!(take_json(unsafe { rv_fm_item_json(fm, 2) }), Value::Null);
        assert_eq!(take_json(unsafe { rv_fm_item_json(fm, -1) }), Value::Null);

        // commands
        unsafe { rv_fm_set_sort(fm, 0, 0) };
        let rows = take_json(unsafe { rv_fm_rows_json(fm) });
        assert_eq!(rows["rows"][0]["name"], "OTHER");
        unsafe { rv_fm_set_sort(fm, 2, 1) };
        unsafe { rv_fm_ungroup(fm, [0].as_ptr(), 1) };
        assert_eq!(take_json(unsafe { rv_fm_rows_json(fm) })["rows"].as_array().unwrap().len(), 3);
        unsafe { rv_fm_group(fm, [0, 1].as_ptr(), 2) };
        assert_eq!(take_json(unsafe { rv_fm_rows_json(fm) })["rows"][0]["files"], 2);
        unsafe { rv_fm_ungroup_all(fm) };
        unsafe { rv_fm_group_auto(fm) };
        assert_eq!(take_json(unsafe { rv_fm_rows_json(fm) })["rows"][0]["name"], "FILE0001-2");
        unsafe { rv_fm_set_lock(fm, [1].as_ptr(), 1, 1) };
        let rows = take_json(unsafe { rv_fm_rows_json(fm) });
        assert_eq!((rows["any_locked"].as_bool(), rows["rows"][1]["lock"].as_u64()), (Some(true), Some(1)));
        assert!(fs::metadata(dir.join("OTHER.MOV")).unwrap().permissions().readonly());
        unsafe { rv_fm_set_lock(fm, [1].as_ptr(), 1, 0) };
        assert!(!fs::metadata(dir.join("OTHER.MOV")).unwrap().permissions().readonly());
        unsafe { rv_fm_remove(fm, [1].as_ptr(), 1) };
        assert_eq!(take_json(unsafe { rv_fm_rows_json(fm) })["rows"].as_array().unwrap().len(), 1);
        unsafe { rv_fm_clear(fm) };
        assert_eq!(take_json(unsafe { rv_fm_rows_json(fm) })["rows"].as_array().unwrap().len(), 0);
        unsafe { rv_fm_destroy(fm) };
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_manager_null_arguments_are_safe() {
        let null = std::ptr::null_mut();
        unsafe {
            rv_fm_destroy(null);
            rv_fm_add(null, std::ptr::null());
            rv_fm_clear(null);
            rv_fm_set_sort(null, 0, 1);
            assert_eq!(rv_fm_version(null), 0);
            assert_eq!(rv_fm_is_idle(null), 0);
            assert_eq!(take_json(rv_fm_rows_json(null))["rows"].as_array().unwrap().len(), 0);
            assert_eq!(take_json(rv_fm_item_json(null, 0)), Value::Null);
            rv_fm_remove(null, std::ptr::null(), 3);
            rv_fm_set_lock(null, std::ptr::null(), 0, 1);
            rv_fm_group(null, std::ptr::null(), 0);
            rv_fm_ungroup(null, std::ptr::null(), 0);
            rv_fm_ungroup_all(null);
            rv_fm_group_auto(null);
        }
        let fm = rv_fm_create(0, 0);
        unsafe {
            rv_fm_add(fm, std::ptr::null());
            rv_fm_remove(fm, std::ptr::null(), 5);
            rv_fm_set_lock(fm, [-1, 7].as_ptr(), 2, 1);
            rv_fm_destroy(fm);
        }
    }
}
