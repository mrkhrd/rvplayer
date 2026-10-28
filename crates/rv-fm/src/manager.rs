//! `TFileManager`: the item list plus a background thread that scans folders, previews files,
//! matches GPS logs and groups segments (spec §3 A, FL-050…FL-058, §4).
//!
//! All state lives behind one mutex. The background thread takes it only to pick a task and to
//! write results back; file I/O happens outside the lock. Every visible change increments
//! [`FileManager::version`]; the item list is kept sorted by the current sort key whenever the
//! lock is released.

use std::collections::{HashMap, HashSet};
use std::fs::{self, Metadata};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rv_core::{FileInfo, Track, rvmi};

use crate::gps::{
    DefaultGpsLoader, GpsLoader, crop, extension_rank, extract_gps_info, extract_gps_time, has_gps_extension,
    has_scan_video_extension, intersection, match_face, track_stats,
};
use crate::grouping::{find_groups_auto, find_groups_fast, group_items, sync_video_by_gps, ungroup_all, ungroup_item};
use crate::model::{FileEntry, GpsInfo, Item, TimeBlock, calculate_group, new_video_item, sort_items};
use crate::platform;
use crate::sidecar::{DirCache, find_files_for_video};
use crate::time::{SEC, fs_times};

pub const STATUS_INITIALIZING: &str = "Initializing";
pub const STATUS_SEARCHING: &str = "Searching";
pub const STATUS_PREVIEW: &str = "Preview";
pub const STATUS_GROUPING: &str = "Grouping_Title";

/// Entries skipped by name (case-sensitive) during folder scans (FL-004).
const SKIPPED_NAMES: [&str; 8] = [".", "..", "nandlog", "System", "Windows", "Program Files", "ProgramData", "Users"];
/// Recursion limit: the queued folder is level 0; levels deeper than 4 are ignored.
const MAX_DEPTH: u32 = 4;
/// GPS logs of this size or larger are not pooled.
const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;
const SCAN_NOTIFY_INTERVAL: Duration = Duration::from_millis(300);
const IDLE_SLEEP: Duration = Duration::from_millis(500);
const START_PAUSE: Duration = Duration::from_millis(200);

/// One row: an item's group summary and its file count.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub summary: FileEntry,
    pub files: usize,
}

/// Rows and status for the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    /// "" | "Initializing" | "Searching" | "Preview" | "Grouping_Title" (translation keys).
    pub status_key: String,
    /// Folder for "Searching", percent for "Preview".
    pub status_arg: String,
    /// Some item is (partly) locked (`HasAnyImageIndex0`).
    pub any_locked: bool,
    pub rows: Vec<Row>,
}

#[derive(Default)]
struct State {
    items: Vec<Item>,
    /// Lower-cased paths of every file in `items`.
    known: HashSet<String>,
    /// GPS log pool (sorted by lower-cased path after each folder scan).
    pool: Vec<GpsInfo>,
    pool_known: HashSet<String>,
    /// Folders waiting to be scanned.
    queue: Vec<String>,
    status_key: &'static str,
    status_arg: String,
    sort_column: i32,
    ascending: bool,
    next_id: u64,
    /// A task ran since the last idle (FL-010).
    dirty: bool,
}

impl State {
    fn sort(&mut self) {
        sort_items(&mut self.items, self.sort_column, self.ascending);
    }

    fn set_status(&mut self, key: &'static str, arg: &str) {
        self.status_key = key;
        self.status_arg = arg.to_string();
    }

    /// `SetCurrActPreview`: "Preview NN%" with NN = min + (max - min) * p.
    fn set_preview(&mut self, min: u32, max: u32, p: f64) {
        let pct = f64::from(min) + (f64::from(max - min) * p).round();
        self.set_status(STATUS_PREVIEW, &format!("{pct}"));
    }

    fn alloc_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn find_id(&self, id: u64) -> Option<(usize, usize)> {
        self.items
            .iter()
            .enumerate()
            .find_map(|(i, it)| it.files.iter().position(|f| f.id == id).map(|k| (i, k)))
    }

    fn find_path(&self, lower: &str) -> Option<(usize, usize)> {
        self.items
            .iter()
            .enumerate()
            .find_map(|(i, it)| it.files.iter().position(|f| f.video.path.to_lowercase() == lower).map(|k| (i, k)))
    }

    /// `HasAnyGPSLog`: some file uses this path as its GPS log.
    fn has_any_gps_log(&self, lower: &str) -> bool {
        self.items.iter().any(|it| it.files.iter().any(|f| !f.gps_log.path.is_empty() && f.gps_log.path_lower == lower))
    }

    fn remove_item(&mut self, index: usize) {
        let it = self.items.remove(index);
        for f in &it.files {
            self.known.remove(&f.video.path.to_lowercase());
        }
    }

    /// One background step, in the priority order of FL-010 (`DoTaskAction`).
    fn next_task(&mut self, gps: bool, epoch: u64) -> Option<Task> {
        if let Some(dir) = self.queue.first() {
            return Some(Task::Scan { dir: dir.clone(), epoch });
        }
        let mut total = 0usize;
        let mut unpreviewed_videos = 0usize;
        let mut video = None;
        let mut info = None;
        for i in (0..self.items.len()).rev() {
            for (z, f) in self.items[i].files.iter().enumerate() {
                if !f.video.previewed {
                    unpreviewed_videos += 1;
                    video.get_or_insert((i, z));
                } else if !f.info.previewed {
                    info.get_or_insert((i, z, total + z));
                }
            }
            total += self.items[i].files.len();
        }
        if let Some((i, z)) = video {
            let p = (total - unpreviewed_videos) as f64 / total as f64;
            self.set_preview(0, if gps { 40 } else { 99 }, p);
            let f = &self.items[i].files[z];
            return Some(Task::Video { id: f.id, path: f.video.path.clone() });
        }
        let span = |l: &GpsInfo| (l.time.creation - l.time.write).abs();
        let mut best: Option<usize> = None;
        let mut unpreviewed_logs = 0usize;
        for k in (0..self.pool.len()).rev() {
            if !self.pool[k].previewed {
                unpreviewed_logs += 1;
                if best.is_none_or(|b| span(&self.pool[k]) < span(&self.pool[b])) {
                    best = Some(k);
                }
            }
        }
        if let Some(k) = best {
            let n = self.pool.len();
            self.set_preview(40, 80, (n - unpreviewed_logs) as f64 / n as f64);
            return Some(Task::Log { path: self.pool[k].path.clone() });
        }
        if let Some((i, z, pos)) = info {
            self.set_preview(80, 100, pos as f64 / total as f64);
            let f = &self.items[i].files[z];
            return Some(Task::Info { id: f.id, video_path: f.video.path.clone(), info_path: f.info.path.clone() });
        }
        None
    }
}

enum Task {
    Scan { dir: String, epoch: u64 },
    Video { id: u64, path: String },
    Log { path: String },
    Info { id: u64, video_path: String, info_path: String },
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    version: AtomicU64,
    stop: AtomicBool,
    idle: AtomicBool,
    /// Incremented by Clear and on destruction: running scans abort.
    epoch: AtomicU64,
    gps: bool,
    auto_search: bool,
    loader: Arc<dyn GpsLoader>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn bump(&self) {
        self.version.fetch_add(1, Ordering::SeqCst);
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn aborted(&self, epoch: u64) -> bool {
        self.stopped() || self.epoch.load(Ordering::SeqCst) != epoch
    }

    /// New background work was queued (called with the lock held).
    fn work_added(&self) {
        self.idle.store(false, Ordering::SeqCst);
        self.wake.notify_all();
    }
}

/// The file manager. Dropping it stops the background thread.
pub struct FileManager {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl FileManager {
    /// FL-001/FL-002: `auto_search` queues the FL-003 locations; `gps` = false for the
    /// video-only profile.
    pub fn new(auto_search: bool, gps: bool) -> FileManager {
        Self::with_loader(auto_search, gps, Arc::new(DefaultGpsLoader))
    }

    /// Like [`FileManager::new`] with a custom GPS loader (sidecar/log parser).
    pub fn with_loader(auto_search: bool, gps: bool, loader: Arc<dyn GpsLoader>) -> FileManager {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                sort_column: 2,
                ascending: true,
                status_key: STATUS_INITIALIZING,
                ..Default::default()
            }),
            wake: Condvar::new(),
            version: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            idle: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            gps,
            auto_search,
            loader,
        });
        let worker = Worker {
            shared: shared.clone(),
            cache: DirCache::default(),
            embedded: HashMap::new(),
            last_notify: Instant::now(),
        };
        let handle = std::thread::Builder::new()
            .name("rv-fm".into())
            .spawn(move || worker.run())
            .expect("spawn file manager thread");
        FileManager { shared, worker: Some(handle) }
    }

    /// Incremented on every change the UI should show (FL-012).
    pub fn version(&self) -> u64 {
        self.shared.version.load(Ordering::SeqCst)
    }

    /// The background thread has nothing left to do (`WaitForPreview`).
    pub fn is_idle(&self) -> bool {
        self.shared.idle.load(Ordering::SeqCst)
    }

    /// Waits until idle or the timeout passes; returns whether it became idle.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let end = Instant::now() + timeout;
        while !self.is_idle() {
            if Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    /// FL-005: a folder is queued for scanning; a GPS log adds the same-named .mov/.mp4; a known
    /// file is re-previewed; any other existing file is added. Other paths are ignored.
    pub fn add(&self, path: &str) {
        let Ok(meta) = fs::metadata(path) else { return };
        if meta.is_dir() {
            let mut st = self.shared.lock();
            st.queue.push(path.to_string());
            self.shared.work_added();
            self.shared.bump();
            return;
        }
        if has_gps_extension(path) {
            let base = match path.rfind('.') {
                Some(i) if !path[i..].contains(['\\', '/']) => &path[..i],
                _ => path,
            };
            for ext in ["mov", "mp4"] {
                let video = format!("{base}.{ext}");
                if let Ok(m) = fs::metadata(&video)
                    && m.is_file()
                {
                    self.add_file(&video, &m);
                }
            }
            return;
        }
        self.add_file(path, &meta);
    }

    fn add_file(&self, path: &str, meta: &Metadata) {
        let lower = path.to_lowercase();
        let mut st = self.shared.lock();
        if st.known.contains(&lower) {
            let id = st.alloc_id();
            if let Some((i, k)) = st.find_path(&lower) {
                let f = &mut st.items[i].files[k];
                f.id = id;
                f.video.previewed = false;
                f.info = GpsInfo::default();
                f.gps_log = GpsInfo::default();
                calculate_group(&mut st.items[i]);
            }
        } else {
            let (creation, write) = fs_times(meta);
            let id = st.alloc_id();
            let it = new_video_item(id, path, meta.permissions().readonly(), creation, write, meta.len() as i64);
            st.known.insert(lower);
            st.items.push(it);
        }
        st.sort();
        self.shared.work_added();
        self.shared.bump();
    }

    /// FL-009: removes all items and empties the GPS log pool and the search queue.
    pub fn clear(&self) {
        let mut st = self.shared.lock();
        self.shared.epoch.fetch_add(1, Ordering::SeqCst);
        st.items.clear();
        st.known.clear();
        st.pool.clear();
        st.pool_known.clear();
        st.queue.clear();
        st.set_status("", "");
        self.shared.bump();
    }

    /// FL-025: column ID 0..7; the list stays sorted by this key.
    pub fn set_sort(&self, column: i32, ascending: bool) {
        if !(0..=7).contains(&column) {
            return;
        }
        let mut st = self.shared.lock();
        st.sort_column = column;
        st.ascending = ascending;
        st.sort();
        self.shared.bump();
    }

    pub fn sort(&self) -> (i32, bool) {
        let st = self.shared.lock();
        (st.sort_column, st.ascending)
    }

    pub fn rows(&self) -> Rows {
        let st = self.shared.lock();
        Rows {
            status_key: st.status_key.to_string(),
            status_arg: st.status_arg.clone(),
            any_locked: st.items.iter().any(|it| it.group.video.lock != 0),
            rows: st.items.iter().map(|it| Row { summary: it.group.clone(), files: it.files.len() }).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.shared.lock().items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One item with its files in group order.
    pub fn item(&self, index: usize) -> Option<Item> {
        self.shared.lock().items.get(index).cloned()
    }

    fn sorted_unique_desc(indices: &[usize], len: usize) -> Vec<usize> {
        let mut v: Vec<usize> = indices.iter().copied().filter(|&i| i < len).collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        v.dedup();
        v
    }

    /// FL-050: removes rows from the list (files on disk are untouched).
    pub fn remove(&self, indices: &[usize]) {
        let mut st = self.shared.lock();
        for k in Self::sorted_unique_desc(indices, st.items.len()) {
            st.remove_item(k);
        }
        st.sort();
        self.shared.bump();
    }

    /// FL-054: sets or clears the read-only attribute of every file of the rows, then
    /// recomputes their lock state from the files.
    pub fn set_lock(&self, indices: &[usize], lock: bool) {
        let targets: Vec<(u64, String)> = {
            let st = self.shared.lock();
            Self::sorted_unique_desc(indices, st.items.len())
                .into_iter()
                .flat_map(|k| st.items[k].files.iter().map(|f| (f.id, f.video.path.clone())).collect::<Vec<_>>())
                .collect()
        };
        let results: Vec<(u64, u32)> = targets.into_iter().map(|(id, path)| (id, set_read_only(&path, lock))).collect();
        let mut st = self.shared.lock();
        let mut touched = Vec::new();
        for (id, flag) in results {
            if let Some((i, k)) = st.find_id(id) {
                st.items[i].files[k].video.lock = flag;
                touched.push(i);
            }
        }
        touched.sort_unstable();
        touched.dedup();
        for i in touched {
            calculate_group(&mut st.items[i]);
        }
        st.sort();
        self.shared.bump();
    }

    /// §4.6.4 manual grouping: the first index is the target, the others are merged into it.
    pub fn group(&self, indices: &[usize]) {
        let mut st = self.shared.lock();
        group_items(&mut st.items, indices, true);
        st.sort();
        self.shared.bump();
    }

    /// FL-056.
    pub fn ungroup(&self, indices: &[usize]) {
        let mut st = self.shared.lock();
        for k in Self::sorted_unique_desc(indices, st.items.len()) {
            ungroup_item(&mut st.items, k);
        }
        st.sort();
        self.shared.bump();
    }

    /// FL-058.
    pub fn ungroup_all(&self) {
        let mut st = self.shared.lock();
        ungroup_all(&mut st.items);
        st.sort();
        self.shared.bump();
    }

    /// FL-057: automatic grouping, immediately.
    pub fn group_auto(&self) {
        let mut st = self.shared.lock();
        find_groups_auto(&mut st.items);
        st.sort();
        self.shared.bump();
    }
}

impl Drop for FileManager {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.epoch.fetch_add(1, Ordering::SeqCst);
        {
            let mut st = self.shared.lock();
            st.queue.clear();
            self.shared.wake.notify_all();
        }
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

/// Sets or clears the read-only attribute; returns the resulting lock flag.
fn set_read_only(path: &str, lock: bool) -> u32 {
    if let Ok(m) = fs::metadata(path) {
        let mut p = m.permissions();
        if p.readonly() != lock {
            p.set_readonly(lock);
            let _ = fs::set_permissions(path, p);
        }
    }
    fs::metadata(path).map_or(u32::from(lock), |m| u32::from(m.permissions().readonly()))
}

/// What a video preview (§4.4.1) read from the file.
#[derive(Default)]
struct VideoPreview {
    duration_s: f64,
    width: u32,
    height: u32,
    fps: f64,
    kbps: f64,
    gop: u32,
    h264: bool,
    aac: bool,
    /// The embedded GPS track, when GPS is allowed and the file has one.
    track: Option<Track>,
}

fn preview_video(path: &str, gps: bool) -> VideoPreview {
    let (info, movie) = FileInfo::load_with_movie(Path::new(path));
    let mut r = VideoPreview::default();
    let Some(movie) = movie else { return r };
    r.duration_s = info.duration;
    r.width = info.width;
    r.height = info.height;
    if let Some(v) = movie.track_by_handler(b"vide") {
        let samples = v.samples();
        let secs = v.duration_secs();
        if secs > 0.0 {
            r.fps = samples.len() as f64 / secs;
            r.kbps = samples.iter().map(|s| f64::from(s.size)).sum::<f64>() * 8.0 / secs / 1000.0;
        }
        let keys: Vec<usize> = samples.iter().enumerate().filter(|(_, s)| s.sync).map(|(i, _)| i).take(2).collect();
        r.gop = match keys.as_slice() {
            [a, b, ..] => (b - a) as u32,
            [_] => samples.len() as u32,
            [] => 0,
        };
        r.h264 = matches!(&v.codec, b"avc1" | b"avc3");
    }
    r.aac = movie
        .tracks
        .get(1)
        .is_some_and(|t| &t.handler == b"soun" && t.codec.eq_ignore_ascii_case(b"mp4a"));
    if gps && (movie.track_by_codec(&rvmi::CODEC).is_some() || !info.track.is_empty()) {
        r.track = Some(info.track);
    }
    r
}

/// The FL-003 auto-search folders, in order, without duplicates and Desktop sub-folders.
pub fn auto_search_dirs(cwd: Option<&str>, exe_dir: Option<&str>, usb: &[String], desktops: &[String]) -> Vec<String> {
    let with_sep = |s: &str| if s.ends_with(['\\', '/']) { s.to_string() } else { format!("{s}\\") };
    let norm = |s: &str| s.trim_end_matches(['\\', '/']).to_lowercase();
    let mut candidates = Vec::new();
    if let Some(cwd) = cwd.filter(|c| !c.is_empty()) {
        let cwd = with_sep(cwd);
        candidates.push(cwd.clone());
        if let Some(exe) = exe_dir {
            candidates.push(with_sep(exe));
        }
        if !cwd.starts_with("\\\\") {
            candidates.push(format!("{}DCIM\\", cwd.chars().take(3).collect::<String>()));
        }
        candidates.extend(usb.iter().cloned());
    }
    let desktops: Vec<String> = desktops.iter().map(|d| norm(d)).filter(|d| !d.is_empty()).collect();
    let mut out: Vec<String> = Vec::new();
    for c in candidates {
        let n = norm(&c);
        if out.iter().any(|q| norm(q) == n) || desktops.iter().any(|d| n.contains(d.as_str())) {
            continue;
        }
        out.push(c);
    }
    out
}

struct Worker {
    shared: Arc<Shared>,
    cache: DirCache,
    /// Embedded tracks read during video preview, used by the info preview of the same file.
    embedded: HashMap<u64, Track>,
    last_notify: Instant,
}

impl Worker {
    fn run(mut self) {
        let shared = self.shared.clone();
        shared.lock().set_status(STATUS_INITIALIZING, "");
        shared.bump();
        if self.pause(START_PAUSE) {
            return;
        }
        if shared.auto_search {
            self.queue_auto_search();
        }
        shared.lock().set_status("", "");
        shared.bump();
        self.cache.clear();
        loop {
            let task = {
                let mut st = shared.lock();
                loop {
                    if shared.stopped() {
                        return;
                    }
                    let epoch = shared.epoch.load(Ordering::SeqCst);
                    if let Some(t) = st.next_task(shared.gps, epoch) {
                        st.dirty = true;
                        break t;
                    }
                    if st.dirty {
                        st.dirty = false;
                        st.set_status(STATUS_GROUPING, "");
                        shared.bump();
                        find_groups_auto(&mut st.items);
                        st.sort();
                        st.set_status("", "");
                        shared.bump();
                        self.cache.clear();
                        self.embedded.clear();
                        continue;
                    }
                    shared.idle.store(true, Ordering::SeqCst);
                    st = shared.wake.wait_timeout(st, IDLE_SLEEP).unwrap_or_else(|e| e.into_inner()).0;
                }
            };
            match task {
                Task::Scan { dir, epoch } => self.scan(&dir, epoch),
                Task::Video { id, path } => {
                    let r = preview_video(&path, shared.gps);
                    self.apply_video(id, r);
                }
                Task::Log { path } => {
                    let track = shared.loader.load(Path::new(&path));
                    apply_log(&shared, &path, track);
                }
                Task::Info { id, video_path, info_path } => self.preview_info(id, &video_path, &info_path),
            }
        }
    }

    /// Sleeps unless stopped; returns true when stopped.
    fn pause(&self, d: Duration) -> bool {
        let st = self.shared.lock();
        let (_st, _) = self
            .shared
            .wake
            .wait_timeout_while(st, d, |_| !self.shared.stopped())
            .unwrap_or_else(|e| e.into_inner());
        self.shared.stopped()
    }

    fn notify_throttled(&mut self) {
        if self.last_notify.elapsed() >= SCAN_NOTIFY_INTERVAL {
            self.shared.bump();
            self.last_notify = Instant::now();
        }
    }

    /// FL-003.
    fn queue_auto_search(&mut self) {
        let shared = self.shared.clone();
        shared.lock().set_status(STATUS_SEARCHING, "");
        shared.bump();
        let cwd = std::env::current_dir().ok().map(|p| p.to_string_lossy().into_owned());
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()));
        let usb = platform::usb_flash_drives();
        let desktops: Vec<String> =
            platform::desktop_dirs().iter().map(|d| d.to_string_lossy().into_owned()).collect();
        let dirs = auto_search_dirs(cwd.as_deref(), exe_dir.as_deref(), &usb, &desktops);
        let mut st = shared.lock();
        let norm = |s: &str| s.trim_end_matches(['\\', '/']).to_lowercase();
        for d in dirs {
            if !st.queue.iter().any(|q| norm(q) == norm(&d)) {
                st.queue.push(d);
            }
        }
    }

    /// FL-004: scans one queued folder.
    fn scan(&mut self, root: &str, epoch: u64) {
        let shared = self.shared.clone();
        if !self.scan_dir(Path::new(root), 0, epoch) {
            return;
        }
        let mut st = shared.lock();
        if shared.aborted(epoch) {
            return;
        }
        if !st.queue.is_empty() {
            st.queue.swap_remove(0);
        }
        find_groups_fast(&mut st.items);
        st.sort();
        st.pool.sort_by(|a, b| a.path_lower.cmp(&b.path_lower));
        st.set_status("", "");
        shared.bump();
        self.last_notify = Instant::now();
    }

    /// Returns false when the scan was aborted.
    fn scan_dir(&mut self, dir: &Path, depth: u32, epoch: u64) -> bool {
        let shared = self.shared.clone();
        if depth > MAX_DEPTH {
            return true;
        }
        let mut shown = dir.to_string_lossy().into_owned();
        if !shown.ends_with(['\\', '/']) {
            shown.push(std::path::MAIN_SEPARATOR);
        }
        {
            let mut st = shared.lock();
            if shared.aborted(epoch) {
                return false;
            }
            st.set_status(STATUS_SEARCHING, &shown);
        }
        self.notify_throttled();
        let Ok(rd) = fs::read_dir(dir) else { return true };
        let (mut subdirs, mut videos, mut logs) = (Vec::new(), Vec::new(), Vec::new());
        for e in rd.flatten() {
            if shared.aborted(epoch) {
                return false;
            }
            let name = e.file_name();
            if SKIPPED_NAMES.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            if platform::is_hidden_or_system(&meta) {
                continue;
            }
            let path = e.path();
            if platform::is_directory(&meta) {
                subdirs.push(path);
                continue;
            }
            let p = path.to_string_lossy().into_owned();
            if has_gps_extension(&p) {
                if shared.gps && meta.len() < MAX_LOG_SIZE {
                    logs.push((p, meta));
                }
            } else if has_scan_video_extension(&p) {
                videos.push((p, meta));
            }
        }
        if !videos.is_empty() || !logs.is_empty() {
            let mut st = shared.lock();
            if shared.aborted(epoch) {
                return false;
            }
            for (p, m) in logs {
                let lower = p.to_lowercase();
                if st.pool_known.insert(lower.clone()) {
                    let (creation, write) = fs_times(&m);
                    st.pool.push(GpsInfo {
                        path: p,
                        path_lower: lower,
                        time: TimeBlock { creation, write, size: m.len() as i64, ..Default::default() },
                        ..Default::default()
                    });
                }
            }
            let mut added = false;
            for (p, m) in videos {
                if st.known.insert(p.to_lowercase()) {
                    let (creation, write) = fs_times(&m);
                    let id = st.alloc_id();
                    let it = new_video_item(id, &p, m.permissions().readonly(), creation, write, m.len() as i64);
                    st.items.push(it);
                    added = true;
                }
            }
            if added {
                st.sort();
            }
        }
        self.notify_throttled();
        for sub in subdirs {
            if !self.scan_dir(&sub, depth + 1, epoch) {
                return false;
            }
        }
        true
    }

    /// §4.4.1, writing back.
    fn apply_video(&mut self, id: u64, r: VideoPreview) {
        let shared = self.shared.clone();
        let mut st = shared.lock();
        let Some((i, k)) = st.find_id(id) else { return };
        let f = &mut st.items[i].files[k];
        if r.duration_s > 0.0 {
            f.video.duration_state = 3;
            f.video.time.duration = r.duration_s * SEC;
            f.video.width = r.width;
            f.video.height = r.height;
            f.video.fps = r.fps;
            f.video.kbps = r.kbps;
            f.video.h264 = r.h264;
            f.video.aac = r.aac;
            f.video.gop = r.gop;
            if let Some(track) = r.track {
                let path = f.video.path.clone();
                f.info.set_path(&path);
                self.embedded.insert(id, track);
            }
        }
        if f.video.duration_state != 3 {
            f.video.duration_state = 2;
        }
        f.video.previewed = true;
        let t = &mut f.video.time;
        if t.write - t.creation < t.duration {
            t.creation = t.write - t.duration;
        }
        calculate_group(&mut st.items[i]);
        if st.items.iter().all(|it| it.files.iter().all(|f| f.video.previewed)) {
            find_groups_fast(&mut st.items);
        }
        st.sort();
        shared.bump();
    }

    /// §4.4.3: GPS info of a video (embedded or sidecar).
    fn preview_info(&mut self, id: u64, video_path: &str, info_path: &str) {
        let shared = self.shared.clone();
        let mut candidates = Vec::new();
        if shared.gps {
            candidates = find_files_for_video(video_path, &mut self.cache);
            if !info_path.is_empty() {
                candidates.insert(0, info_path.to_string());
            }
        }
        let embedded = self.embedded.remove(&id);
        let loaded = candidates.into_iter().next().map(|path| {
            let track = if path.to_lowercase() == video_path.to_lowercase() {
                embedded.or_else(|| shared.loader.load(Path::new(&path)))
            } else {
                shared.loader.load(Path::new(&path))
            };
            let fs = fs::metadata(&path).ok().map(|m| {
                let (c, w) = fs_times(&m);
                (c, w, m.len() as i64)
            });
            (path, track, fs)
        });

        let mut st = shared.lock();
        let Some((i, k)) = st.find_id(id) else { return };
        if let Some((path, track, fs)) = loaded {
            let lower = path.to_lowercase();
            let stats = track.as_ref().map(track_stats);
            let enough = stats.as_ref().is_some_and(|s| s.gps_points >= 2 || s.gsensor_points >= 2);
            // A shared trip log that does not overlap this video is left to the log matching.
            let shared_trip_log = enough && {
                let f = &st.items[i].files[k];
                let (ic, iw) = fs.map_or((f.info.time.creation, f.info.time.write), |(c, w, _)| (c, w));
                let v = f.video.time.duration;
                let mut fs_start = ic;
                if v <= iw - fs_start || iw - fs_start > 2.0 {
                    fs_start = iw - v;
                }
                let lo = (f.video.time.write - v).max(fs_start);
                let hi = f.video.time.write.min(iw);
                let st_ = stats.as_ref().unwrap();
                hi - lo < 0.0 && 1.1 * v < st_.max_time - st_.min_time && st.has_any_gps_log(&lower)
            };
            let f = &mut st.items[i].files[k];
            f.info.set_path(&path);
            match stats {
                None => f.info.clear_path(),
                Some(s) => {
                    let mut extract = true;
                    if !enough {
                        f.info.clear_path();
                    } else {
                        if let Some((c, w, size)) = fs {
                            f.info.time.creation = c;
                            f.info.time.write = w;
                            f.info.time.size = size;
                        }
                        if shared_trip_log {
                            f.info.clear_path();
                            f.info.time.creation = 0.0;
                            f.info.time.write = 0.0;
                            f.info.time.size = 0;
                            extract = false;
                        }
                    }
                    if extract {
                        extract_gps_info(&mut f.info, &s);
                        extract_gps_time(&mut f.info, &s);
                    }
                }
            }
        }
        st.items[i].files[k].info.previewed = true;
        calculate_group(&mut st.items[i]);
        if st.items.iter().all(|it| it.files.iter().all(|f| f.info.previewed)) {
            find_groups_fast(&mut st.items);
        }
        st.sort();
        shared.bump();
    }
}

/// §4.4.2: a pool log was loaded; record its span and assign it to overlapping videos.
fn apply_log(shared: &Shared, path: &str, track: Option<Track>) {
    let lower = path.to_lowercase();
    let mut st = shared.lock();
    let Some(pi) = st.pool.iter().position(|l| l.path_lower == lower) else { return };
    let stats = track.as_ref().map(track_stats);
    {
        let log = &mut st.pool[pi];
        if let Some(s) = &stats {
            extract_gps_info(log, s);
            extract_gps_time(log, s);
        }
        if stats.as_ref().is_none_or(|s| s.gps_points < 2 && s.gsensor_points < 2) {
            log.time = TimeBlock::default();
        }
        log.previewed = true;
    }
    let log = st.pool[pi].clone();
    if log.time.duration > 0.0
        && let Some(track) = &track
    {
        match_log(&mut st.items, &log, track);
        st.sort();
    }
    shared.bump();
}

/// The matching part of `PreviewGPSLog`.
fn match_log(items: &mut [Item], log: &GpsInfo, track: &Track) {
    let log_face = match_face(&log.path);
    for it in items.iter_mut() {
        let mut changed = false;
        for f in it.files.iter_mut() {
            let v = intersection(&f.video, log, true);
            if v > 2.0 * SEC {
                let v_old = intersection(&f.video, &f.gps_log, true);
                if v_old < v && v > 0.1 * f.video.time.duration {
                    f.gps_log = log.clone();
                    sync_video_by_gps(f);
                    if f.info.path.is_empty() {
                        let part = crop(track, f.video.time.start(), f.video.time.finish);
                        extract_gps_info(&mut f.info, &track_stats(&part));
                        f.info.time = log.time.clone();
                    }
                    changed = true;
                }
            }
            let raw = intersection(&f.video, log, false);
            if log.time.duration - 11.0 * SEC < raw.min(f.video.time.duration)
                && match_face(&f.video.path) == log_face
                && (f.info.path.is_empty() || extension_rank(&log.path) > extension_rank(&f.info.path))
            {
                f.info = log.clone();
                if f.gps_log.path == f.info.path {
                    f.gps_log = GpsInfo::default();
                }
                sync_video_by_gps(f);
                changed = true;
            }
        }
        if changed {
            calculate_group(it);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_search_folders() {
        let dirs = auto_search_dirs(
            Some(r"C:\Work"),
            Some(r"C:\Work\"),
            &[r"E:\".to_string()],
            &[r"C:\Users\me\Desktop".to_string()],
        );
        assert_eq!(dirs, [r"C:\Work\", r"C:\DCIM\", r"E:\"]);
        let dirs = auto_search_dirs(Some(r"\\srv\share\x"), Some(r"C:\Users\me\Desktop\app"), &[], &[r"C:\Users\me\Desktop".to_string()]);
        assert_eq!(dirs, [r"\\srv\share\x\"]);
    }
}
