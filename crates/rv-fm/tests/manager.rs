//! File manager tests on temporary folders.

use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rv_core::testutil::{GpsFix, movie_with_track, write_rvmi_movie};
use rv_core::{GpsPoint, Track};
use rv_fm::time::{SEC, fs_times, to_naive};
use rv_fm::{FileManager, GpsLoader};

const IDLE: Duration = Duration::from_secs(30);

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!("rv-fm-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        clear_read_only(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn clear_read_only(dir: &Path) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            clear_read_only(&p);
        } else if let Ok(m) = fs::metadata(&p) {
            let mut perm = m.permissions();
            if perm.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                perm.set_readonly(false);
                let _ = fs::set_permissions(&p, perm);
            }
        }
    }
}

/// A video-only movie of `secs` one-second samples (1920x1080 unless patched).
fn video_movie(secs: usize, small: bool) -> Vec<u8> {
    let mut v = movie_with_track(b"vide", b"avc1", 1, &vec![vec![0u8; 16]; secs]);
    if small {
        patch(&mut v, &(1920u32 << 16).to_be_bytes(), &(1280u32 << 16).to_be_bytes());
        patch(&mut v, &(1080u32 << 16).to_be_bytes(), &(720u32 << 16).to_be_bytes());
    }
    v
}

fn patch(data: &mut [u8], from: &[u8; 4], to: &[u8; 4]) {
    let i = data.windows(4).position(|w| w == from).unwrap();
    data[i..i + 4].copy_from_slice(to);
}

/// Base time for file-system timestamps: 2024-03-10 10:00:00 UTC.
fn base() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_064_800)
}

fn set_times(path: &Path, created: SystemTime, written: SystemTime) {
    let f = File::options().write(true).open(path).unwrap();
    let mut t = FileTimes::new().set_modified(written);
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTimesExt;
        t = t.set_created(created);
    }
    #[cfg(not(windows))]
    let _ = created;
    f.set_times(t).unwrap();
}

/// Writes a file and sets its times to [base + start_s, base + start_s + dur_s].
fn write_at(path: &Path, data: &[u8], start_s: u64, dur_s: u64) {
    fs::write(path, data).unwrap();
    let c = base() + Duration::from_secs(start_s);
    set_times(path, c, c + Duration::from_secs(dur_s));
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn all_files(fm: &FileManager) -> Vec<String> {
    let mut v: Vec<String> = (0..fm.len())
        .flat_map(|i| fm.item(i).unwrap().files.into_iter().map(|f| f.video.path))
        .collect();
    v.sort();
    v
}

fn names(fm: &FileManager) -> Vec<String> {
    let mut v: Vec<String> = fm.rows().rows.into_iter().map(|r| r.summary.video.face).collect();
    v.sort();
    v
}

#[test]
fn folder_scan_rules() {
    let t = TempDir::new("scan");
    let r = &t.0;
    let mut deep = r.clone();
    for (level, name) in ["l1", "l2", "l3", "l4", "l5"].iter().enumerate() {
        deep = deep.join(name);
        fs::create_dir_all(&deep).unwrap();
        write_at(&deep.join(format!("deep{}.mov", level + 1)), b"x", 1000 * level as u64, 0);
    }
    for (i, f) in ["a.mov", "b.MP4", "c.mp4v", "d.avi", "e.txt", "f.gpx"].iter().enumerate() {
        write_at(&r.join(f), b"not a movie", 5000 + 1000 * i as u64, 1);
    }
    for skipped in ["System", "nandlog", "Users"] {
        fs::create_dir_all(r.join(skipped)).unwrap();
        write_at(&r.join(skipped).join("x.mov"), b"x", 20_000, 1);
    }
    #[cfg(windows)]
    {
        write_at(&r.join("hidden.mov"), b"x", 30_000, 1);
        let st = std::process::Command::new("attrib").arg("+h").arg(r.join("hidden.mov")).status().unwrap();
        assert!(st.success());
    }

    let fm = FileManager::new(false, true);
    fm.add(&s(r));
    assert!(fm.wait_idle(IDLE));
    let files = all_files(&fm);
    let rel: Vec<String> = files
        .iter()
        .map(|f| f[s(r).len() + 1..].replace('\\', "/"))
        .collect();
    let mut want = vec!["a.mov", "b.MP4", "c.mp4v", "l1/l2/l3/l4/deep4.mov", "l1/deep1.mov", "l1/l2/deep2.mov", "l1/l2/l3/deep3.mov"];
    want.sort();
    let mut rel_sorted = rel.clone();
    rel_sorted.sort();
    assert_eq!(rel_sorted, want);
    // unreadable files were previewed and flagged
    let rows = fm.rows();
    assert_eq!(rows.status_key, "");
    assert!(rows.rows.iter().all(|r| r.summary.video.duration_state == 2), "{:?}", rows.rows);
    // adding the same folder again finds nothing new
    let v = fm.version();
    fm.add(&s(r));
    assert!(fm.wait_idle(IDLE));
    assert!(fm.version() > v);
    assert_eq!(all_files(&fm).len(), files.len());
}

#[test]
fn preview_reads_duration_and_embedded_gps() {
    let t = TempDir::new("preview");
    let fixes: Vec<GpsFix> = (0..30)
        .map(|i| GpsFix { lat: 60.0 + f64::from(i) * 0.0005, lon: 30.0, speed_kmh: 60.0, course: 0.0 })
        .collect();
    let path = t.0.join("FILE0001.MOV");
    write_rvmi_movie(&path, 42_506.5, &fixes).unwrap();
    let plain = t.0.join("FILE0100.MOV");
    write_at(&plain, &video_movie(20, false), 100_000, 20);

    let fm = FileManager::new(false, true);
    fm.add(&s(&path));
    fm.add(&s(&plain));
    assert!(fm.wait_idle(IDLE));
    let rows = fm.rows();
    assert_eq!(rows.rows.len(), 2);
    let gps_item = (0..2).map(|i| fm.item(i).unwrap()).find(|it| it.group.video.path == s(&path)).unwrap();
    let f = &gps_item.files[0];
    assert_eq!(f.video.duration_state, 3);
    assert_eq!(f.video.duration_ms, 31_000);
    assert_eq!(f.info.path, s(&path));
    assert_eq!(f.info.gps, 2);
    assert_eq!(f.info.gps_points, 30);
    assert!(f.info.track_length > 1500.0, "{}", f.info.track_length);
    assert!(f.info.avg_speed > 0.0);
    // the GPS span: 2016-05-16 12:00:01 .. 12:00:30
    assert!((f.info.time.finish - (42_506.5 + 30.0 * SEC)).abs() < 1e-7);
    assert!((f.info.time.duration - 29.0 * SEC).abs() < 1e-7);

    let plain_item = (0..2).map(|i| fm.item(i).unwrap()).find(|it| it.group.video.path == s(&plain)).unwrap();
    let p = &plain_item.files[0];
    assert_eq!((p.video.width, p.video.height, p.video.duration_ms), (1920, 1080, 20_000));
    assert_eq!(p.video.gop, 1);
    assert!((p.video.fps - 1.0).abs() < 1e-9);
    assert!(p.info.path.is_empty());
    assert_eq!(plain_item.group.info.gps, 0);

    // without GPS (video-only profile) the embedded track is ignored
    let fm = FileManager::new(false, false);
    fm.add(&s(&path));
    assert!(fm.wait_idle(IDLE));
    let it = fm.item(0).unwrap();
    assert!(it.files[0].info.path.is_empty());
    assert_eq!(it.group.info.gps, 0);
}

#[test]
fn pipeline_groups_consecutive_segments() {
    let t = TempDir::new("group");
    // three 60 s segments 3 s apart, then one 12 s later at a different resolution
    for (i, start) in [0u64, 63, 126].iter().enumerate() {
        write_at(&t.0.join(format!("FILE000{}.MOV", i + 1)), &video_movie(60, false), *start, 60);
    }
    write_at(&t.0.join("FILE0004.MOV"), &video_movie(60, true), 198, 60);
    // an unrelated recording two days later
    write_at(&t.0.join("FILE0100.MOV"), &video_movie(60, false), 200_000, 60);

    let fm = FileManager::new(false, true);
    fm.add(&s(&t.0));
    assert!(fm.wait_idle(IDLE));
    assert_eq!(names(&fm), ["FILE0001-3", "FILE0004", "FILE0100"]);
    let rows = fm.rows();
    // default sort: start time ascending
    let order: Vec<String> = rows.rows.iter().map(|r| r.summary.video.face.clone()).collect();
    assert_eq!(order, ["FILE0001-3", "FILE0004", "FILE0100"]);
    let g = &rows.rows[0];
    assert_eq!((g.files, g.summary.video.duration_ms, g.summary.video.duration_state), (3, 180_000, 3));
    assert!((g.summary.video.time.duration - 186.0 * SEC).abs() < 1e-6);
    let item = fm.item(0).unwrap();
    let fr: Vec<(f64, f64)> = item.files.iter().map(|f| (f.video.time.p_start, f.video.time.p_finish)).collect();
    assert!((fr[1].0 - 63.0 / 186.0).abs() < 1e-6 && (fr[2].1 - 1.0).abs() < 1e-9, "{fr:?}");
    assert!(fm.item(3).is_none());

    // sorting by name descending
    fm.set_sort(0, false);
    let order: Vec<String> = fm.rows().rows.iter().map(|r| r.summary.video.face.clone()).collect();
    assert_eq!(order, ["FILE0100", "FILE0004", "FILE0001-3"]);
    fm.set_sort(2, true);

    // ungroup, regroup automatically
    let v = fm.version();
    fm.ungroup(&[0]);
    assert!(fm.version() > v, "commands bump the version immediately");
    assert_eq!(fm.len(), 5);
    fm.group_auto();
    assert_eq!(names(&fm), ["FILE0001-3", "FILE0004", "FILE0100"]);
    fm.ungroup_all();
    assert_eq!(fm.len(), 5);
    // manual grouping ignores compatibility
    let idx: Vec<usize> = (0..fm.len()).collect();
    fm.group(&idx);
    assert_eq!(fm.len(), 1);
    assert_eq!(fm.rows().rows[0].files, 5);
    fm.ungroup_all();
    // remove two rows
    fm.remove(&[0, 4, 4, 99]);
    assert_eq!(fm.len(), 3);
    assert!(!all_files(&fm).iter().any(|f| f.ends_with("FILE0001.MOV") || f.ends_with("FILE0100.MOV")));
    // a removed file can be added again
    fm.add(&s(&t.0.join("FILE0100.MOV")));
    assert_eq!(fm.len(), 4);
    assert!(fm.wait_idle(IDLE));
    fm.clear();
    assert_eq!((fm.len(), fm.rows().status_key.as_str()), (0, ""));
}

#[test]
fn set_lock_toggles_read_only() {
    let t = TempDir::new("lock");
    write_at(&t.0.join("A0001.MOV"), &video_movie(10, false), 0, 10);
    write_at(&t.0.join("A0002.MOV"), &video_movie(10, false), 10, 10);
    let fm = FileManager::new(false, true);
    fm.add(&s(&t.0));
    assert!(fm.wait_idle(IDLE));
    assert_eq!(fm.len(), 1, "{:?}", names(&fm));
    assert!(!fm.rows().any_locked);
    fm.set_lock(&[0], true);
    let rows = fm.rows();
    assert!(rows.any_locked);
    assert_eq!(rows.rows[0].summary.video.lock, 1);
    assert!(fs::metadata(t.0.join("A0001.MOV")).unwrap().permissions().readonly());
    // partial lock: unlock one file of the group
    fm.set_lock(&[0], false);
    fm.ungroup(&[0]);
    fm.set_lock(&[0], true);
    assert!(fs::metadata(t.0.join("A0001.MOV")).unwrap().permissions().readonly());
    assert!(!fs::metadata(t.0.join("A0002.MOV")).unwrap().permissions().readonly());
    fm.group(&[0, 1]);
    let rows = fm.rows();
    assert_eq!((rows.rows.len(), rows.rows[0].summary.video.lock), (1, 2));
    assert!(rows.any_locked);
    fm.set_lock(&[0], false);
    let rows = fm.rows();
    assert!(!rows.any_locked);
    assert_eq!(rows.rows[0].summary.video.lock, 0);
    assert!(!fs::metadata(t.0.join("A0002.MOV")).unwrap().permissions().readonly());
}

/// A loader that returns a fixed track for `.gpx` files (stands in for a log parser).
struct FakeGpx(Track);

impl GpsLoader for FakeGpx {
    fn load(&self, path: &Path) -> Option<Track> {
        path.extension().is_some_and(|e| e.eq_ignore_ascii_case("gpx")).then(|| self.0.clone())
    }
}

#[test]
fn pool_log_is_matched_to_its_video() {
    let t = TempDir::new("pool");
    let video = t.0.join("FILE0001.MOV");
    write_at(&video, &video_movie(60, false), 0, 60);
    let gpx = t.0.join("FILE0001.gpx");
    write_at(&gpx, b"<gpx/>", 0, 60);
    let (_, write) = fs_times(&fs::metadata(&video).unwrap());
    // the GPS clock is one hour ahead of the camera clock
    let start = to_naive(write - 60.0 * SEC + 3600.0 * SEC).unwrap();
    let gps = (1..=60)
        .map(|i| GpsPoint { time: f64::from(i), lat: 60.0 + f64::from(i) * 1e-4, lon: 30.0, altitude: 0.0, speed_kmh: 40.0, course: 0.0, warning: 0 })
        .collect();
    let loader = Arc::new(FakeGpx(Track { start: Some(start), gps, gsensor: vec![] }));

    let fm = FileManager::with_loader(false, true, loader);
    fm.add(&s(&t.0));
    assert!(fm.wait_idle(IDLE));
    let it = fm.item(0).unwrap();
    assert_eq!(fm.len(), 1);
    let f = &it.files[0];
    assert_eq!(f.info.path, s(&gpx));
    assert!(f.gps_log.path.is_empty(), "the log became the file's info");
    assert_eq!(it.group.info.gps, 2);
    // the video window moved to the GPS clock
    assert!((f.video.time.finish - (write + 3600.0 * SEC)).abs() < 1e-6, "{}", (f.video.time.finish - write) / SEC);

    // adding the GPS log adds its video (already known: re-previewed)
    let v = fm.version();
    fm.add(&s(&gpx));
    assert!(fm.version() > v);
    assert!(fm.wait_idle(IDLE));
    assert_eq!(fm.len(), 1);
}

#[test]
fn status_is_initializing_then_cleared() {
    let fm = FileManager::new(false, true);
    assert_eq!(fm.rows().status_key, "Initializing");
    assert!(fm.wait_idle(IDLE));
    let rows = fm.rows();
    assert_eq!((rows.status_key.as_str(), rows.status_arg.as_str(), rows.rows.len()), ("", "", 0));
    // a missing path is ignored silently
    fm.add(r"Z:\does\not\exist.mov");
    assert!(fm.is_idle());
    assert_eq!(fm.len(), 0);
}

#[test]
fn destroy_stops_promptly_and_clear_aborts_scan() {
    let t = TempDir::new("stop");
    for i in 0..50 {
        let d = t.0.join(format!("d{i}"));
        fs::create_dir_all(&d).unwrap();
        write_at(&d.join("x.mov"), b"x", i * 1000, 1);
    }
    let fm = FileManager::new(false, true);
    fm.add(&s(&t.0));
    fm.clear();
    assert!(fm.wait_idle(IDLE));
    assert_eq!(fm.len(), 0);
    fm.add(&s(&t.0));
    let start = std::time::Instant::now();
    drop(fm);
    assert!(start.elapsed() < Duration::from_secs(5));
}

/// An NMEA RMC sentence with its checksum.
fn rmc(time: &str, lat: &str, lon: &str) -> String {
    let body = format!("GPRMC,{time},A,{lat},N,{lon},E,20.0,90.0,100324,,,A");
    let sum = body.bytes().fold(0u8, |a, c| a ^ c);
    format!("${body}*{sum:02X}\r\n")
}

#[test]
fn default_loader_reads_sidecar_logs() {
    let t = TempDir::new("sidecar-logs");
    let v1 = t.0.join("FILE0001.MOV");
    let v2 = t.0.join("FILE0002.MOV");
    write_at(&v1, &video_movie(30, false), 0, 30);
    write_at(&v2, &video_movie(30, false), 3600, 30);
    let nmea: String = (0..30)
        .map(|i| rmc(&format!("1000{i:02}.000"), &format!("6003.{:04}", 8176 + i * 10), "03019.6213"))
        .collect();
    write_at(&t.0.join("FILE0001.NMEA"), nmea.as_bytes(), 0, 30);
    let gpx: String = std::iter::once("<?xml version=\"1.0\"?>\n<gpx version=\"1.1\"><trk><trkseg>\n".to_string())
        .chain((0..30).map(|i| {
            format!(
                "<trkpt lat=\"60.{:06}\" lon=\"30.327022\"><time>2024-03-10T11:00:{i:02}Z</time></trkpt>\n",
                63627 + i * 100
            )
        }))
        .chain(std::iter::once("</trkseg></trk></gpx>\n".to_string()))
        .collect();
    write_at(&t.0.join("FILE0002.gpx"), gpx.as_bytes(), 3600, 30);

    let fm = FileManager::new(false, true);
    fm.add(&s(&t.0));
    assert!(fm.wait_idle(IDLE));
    let files: Vec<_> = (0..fm.len()).flat_map(|i| fm.item(i).unwrap().files).collect();
    assert_eq!(files.len(), 2);
    for (video, log, format) in [(&v1, "FILE0001.NMEA", "NMEA"), (&v2, "FILE0002.gpx", "gpx")] {
        let f = files.iter().find(|f| f.video.path == s(video)).unwrap();
        assert_eq!(f.info.gps, 2, "{log}: {:?}", f.info);
        assert!(f.info.path.ends_with(log), "{}", f.info.path);
        assert_eq!(f.info.gps_points, 30);
        assert_eq!(f.info.format_id, format);
        assert!(f.info.track_length > 40.0, "{}", f.info.track_length);
    }
    let rows = fm.rows();
    assert!(rows.rows.iter().all(|r| r.summary.info.gps == 2), "{:?}", rows.rows);
}

#[test]
fn attached_track_aligns_the_videos_and_shift_moves_them() {
    let t = TempDir::new("set-track");
    let video = t.0.join("A0001.MOV");
    write_at(&video, &video_movie(60, false), 0, 60);
    let gpx = t.0.join("other/trip.gpx");
    // A 65 s trip that ended three hours after the camera clock says the video ended.
    let (_, write) = fs_times(&fs::metadata(&video).unwrap());
    let start = to_naive(write - 65.0 * SEC + 3.0 * 3600.0 * SEC).unwrap();
    let gps = (1..=65)
        .map(|i| GpsPoint { time: f64::from(i - 1), lat: 60.0 + f64::from(i) * 1e-4, lon: 30.0, altitude: 0.0, speed_kmh: 40.0, course: 0.0, warning: 0 })
        .collect();
    let loader = Arc::new(FakeGpx(Track { start: Some(start), gps, gsensor: vec![] }));
    let fm = FileManager::with_loader(false, true, loader);
    fm.add(&s(&video));
    assert!(fm.wait_idle(IDLE));
    assert!(fm.item(0).unwrap().files[0].info.path.is_empty());

    // FL-073: durations within 11 s, so the video now ends with the track.
    assert!(fm.set_track(0, &s(&gpx)));
    let it = fm.item(0).unwrap();
    let f = &it.files[0];
    assert_eq!(f.info.path, s(&gpx));
    assert!(f.info.previewed);
    assert_eq!(f.info.gps, 2);
    let track_end = write + 3.0 * 3600.0 * SEC - SEC;
    assert!((f.video.time.finish - track_end).abs() < 1e-6, "{}", (f.video.time.finish - track_end) / SEC);
    assert!((it.group.video.time.finish - track_end).abs() < 1e-6);

    // FL-074: ten seconds later, and back.
    fm.shift_video(0, 10.0 * SEC);
    assert!((fm.item(0).unwrap().group.video.time.finish - (track_end + 10.0 * SEC)).abs() < 1e-6);
    fm.shift_video(0, -10.0 * SEC);
    assert!((fm.item(0).unwrap().group.video.time.finish - track_end).abs() < 1e-6);

    // A file the loader cannot read attaches nothing.
    assert!(!fm.set_track(0, &s(&t.0.join("none.txt"))));
}
