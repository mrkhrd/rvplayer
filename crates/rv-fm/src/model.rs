//! Data model (spec §4.1), adding a video file (§4.2), the group summary (§4.7) and the sort
//! keys (FL-025, §4.9).

use std::cmp::Ordering;

use crate::time::{MS_PER_DAY, NO_DATE, is_first_of_january};

/// `TBlockDateTime`: file-system times and the real time window of a file (or group).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeBlock {
    /// File-system creation time (TDateTime, local).
    pub creation: f64,
    /// File-system last-write time (TDateTime, local).
    pub write: f64,
    /// File size in bytes.
    pub size: i64,
    /// Real duration in days.
    pub duration: f64,
    /// Real finish time (TDateTime); the start is always `finish - duration`.
    pub finish: f64,
    /// Progress fractions of this block within its group (§4.7 step 5).
    pub p_start: f64,
    pub p_finish: f64,
}

impl TimeBlock {
    pub fn start(&self) -> f64 {
        self.finish - self.duration
    }
}

/// `TVideoFileItem`. For a group summary `path` holds all paths joined with ", ", `face` the
/// display name and `lock` the lock state 0/1/2.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoFile {
    pub path: String,
    pub face: String,
    pub lock: u32,
    pub duration_ms: i64,
    /// 0 unknown, 1 estimated from file times, 2 the parser failed, 3 exact.
    pub duration_state: u8,
    pub previewed: bool,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub kbps: f64,
    pub h264: bool,
    pub aac: bool,
    /// Key-frame distance in frames (0 = unknown).
    pub gop: u32,
    pub no_group: bool,
    pub time: TimeBlock,
}

/// `TGPSLogFileItem`: GPS info of a video (embedded or sidecar), a pool log, or a file's
/// assigned GPS log.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpsInfo {
    pub path: String,
    pub path_lower: String,
    /// Metres.
    pub track_length: f64,
    /// km/h; for a group summary -1 means "none".
    pub avg_speed: f64,
    pub previewed: bool,
    /// 0 none, 1 points without a valid fix, 2 valid fix.
    pub gps: u8,
    pub format_id: String,
    pub gps_points: u32,
    pub gsensor_points: u32,
    pub time: TimeBlock,
}

impl GpsInfo {
    pub fn set_path(&mut self, path: &str) {
        self.path = path.to_string();
        self.path_lower = path.to_lowercase();
    }

    pub fn clear_path(&mut self) {
        self.path.clear();
        self.path_lower.clear();
    }
}

/// `TFileItemSubitem`: one file of an item.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileEntry {
    /// Identity used by the background thread to find the file again; 0 for summaries.
    pub id: u64,
    pub video: VideoFile,
    pub info: GpsInfo,
    pub gps_log: GpsInfo,
}

/// `TFileItem`: one row: a single file or a group of files that play as one timeline.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Item {
    pub files: Vec<FileEntry>,
    pub group: FileEntry,
}

impl Item {
    pub fn single(file: FileEntry) -> Item {
        let mut it = Item { files: vec![file], group: FileEntry::default() };
        calculate_group(&mut it);
        it
    }
}

/// `ExtractFileName`: text after the last `\`, `/` or `:`.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/', ':']).next().unwrap_or(path)
}

/// `ExtractFileFace`: file name without its last extension.
pub fn file_face(path: &str) -> &str {
    let name = file_name(path);
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

/// Delphi `StrToIntDef(s, -1)` for the cases that matter here: optional sign and digits.
fn delphi_str_to_int(s: &str) -> Option<i32> {
    let s = s.trim_start_matches(' ');
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Display face of a newly added video and its noGroup flag from the name (§4.2 steps 2–3).
pub fn video_face(path: &str) -> (String, bool) {
    let mut s = file_face(path).to_string();
    // "FI012345" -> "FILE2345.01"
    if s.chars().count() == 8 && s.starts_with("FI") && delphi_str_to_int(&s[2..]).is_some_and(|n| n > 0)
    {
        s = format!("FILE{}.{}", &s[4..], &s[2..4]);
    }
    let mut no_group = false;
    let head: String = s.chars().take(12).collect();
    if head.eq_ignore_ascii_case("fragment of ") {
        s = s[12..].to_string();
        if let Some(p) = s.rfind('(') {
            s.truncate(p);
        }
        s.push_str("(fr)");
        no_group = true;
    }
    (s, no_group)
}

/// `AddVideoFile` (§4.2): a single-file item with fields from the name and file-system data.
pub fn new_video_item(id: u64, path: &str, read_only: bool, creation: f64, write: f64, size: i64) -> Item {
    let (face, mut no_group) = video_face(path);
    if is_first_of_january(write + 0.5) {
        no_group = true; // camera clock never set
    }
    let mut duration = write - creation;
    if !(0.0..=1.0 / 6.0).contains(&duration) {
        duration = 0.0;
    }
    let video = VideoFile {
        path: path.to_string(),
        face,
        lock: u32::from(read_only),
        duration_state: if duration > 0.0 { 1 } else { 0 },
        no_group,
        time: TimeBlock { creation, write, size, duration, finish: write, ..Default::default() },
        ..Default::default()
    };
    Item::single(FileEntry { id, video, ..Default::default() })
}

/// `IncBlockTime`: union of time windows (finish = max, start = min).
pub fn inc_block_time(g: &mut TimeBlock, n: &TimeBlock) {
    if g.finish <= NO_DATE {
        g.finish = n.finish;
        g.duration = n.duration;
    } else {
        let start = g.start().min(n.start());
        g.finish = g.finish.max(n.finish);
        g.duration = g.finish - start;
    }
}

fn is_index_char(b: u8) -> bool {
    b == b'-' || b == b'.' || b.is_ascii_digit()
}

/// `GetInd`: the trailing run of `0-9`, `-`, `.` if 1–5 characters long, as a number; -1 = none.
pub fn name_index(face: &str) -> f64 {
    let b = face.as_bytes();
    let mut i = b.len();
    while i > 0 && is_index_char(b[i - 1]) {
        i -= 1;
    }
    let run = &face[i..];
    if run.is_empty() || run.len() > 5 {
        return -1.0;
    }
    run.parse::<f64>().ok().filter(|v| v.is_finite()).unwrap_or(-1.0)
}

/// `IsFilesNeigbor`: after the common prefix both remainders are only digits, `-` or `.`.
pub fn faces_are_neighbours(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let common = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    a[common..].iter().all(|&c| is_index_char(c)) && b[common..].iter().all(|&c| is_index_char(c))
}

/// Display name of a group from its file faces in group order (§4.7 step 4).
pub fn group_display_name(faces: &[&str]) -> String {
    let n = faces.len();
    let mut name = String::new();
    let mut prev = -1.0f64;
    let mut pending = -1.0f64;
    for (i, face) in faces.iter().enumerate() {
        let cur = if i == 0 || faces_are_neighbours(faces[i - 1], face) { name_index(face) } else { -1.0 };
        if prev < 0.0 || cur < 0.0 || (prev - cur).abs() >= 1.99 {
            if pending >= 0.0 {
                name.push_str(&format!("-{}", pending.floor() as i64));
                pending = -1.0;
            }
            if i == 0 {
                name.push_str(face);
            } else if cur < 0.0 {
                if i == n - 1 {
                    name.push_str(&format!(" ({n})"));
                }
            } else {
                name.push_str(&format!(", {}", cur.floor() as i64));
            }
        } else {
            pending = cur;
            if i == n - 1 {
                name.push_str(&format!("-{}", cur.floor() as i64));
            }
        }
        prev = cur;
    }
    if name.chars().count() > 50 {
        name = name.chars().take(50).collect::<String>() + "...";
    }
    name
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// `CalculateGroup` (§4.7): orders the files and recomputes the group summary and the
/// progress fractions.
pub fn calculate_group(it: &mut Item) {
    // 1. by start time; equal starts by full path ascending (OQ-1). Bubble sort is stable, as is
    // this sort, so the result is identical.
    it.files.sort_by(|a, b| {
        a.video
            .time
            .start()
            .partial_cmp(&b.video.time.start())
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.video.path.cmp(&b.video.path))
    });
    // 2.
    for f in &mut it.files {
        f.video.duration_ms = (f.video.time.duration * MS_PER_DAY).round() as i64;
    }
    let n = it.files.len();
    if n == 1 {
        // 3.
        it.group = it.files[0].clone();
        it.group.id = 0;
    } else if n > 1 {
        // 4.
        let mut g = FileEntry::default();
        g.video.duration_state = 3;
        let faces: Vec<&str> = it.files.iter().map(|f| f.video.face.as_str()).collect();
        g.video.face = group_display_name(&faces);
        let mut locked = 0usize;
        let mut gps_time = 0.0f64;
        for f in &it.files {
            if !g.video.path.is_empty() {
                g.video.path.push_str(", ");
            }
            g.video.path.push_str(&f.video.path);
            g.video.time.size += f.video.time.size;
            g.video.duration_ms += f.video.duration_ms;
            g.video.duration_state = g.video.duration_state.min(f.video.duration_state);
            inc_block_time(&mut g.video.time, &f.video.time);
            locked += f.video.lock as usize;
            g.video.width = g.video.width.max(f.video.width);
            g.video.height = g.video.height.max(f.video.height);
            g.video.fps = g.video.fps.max(f.video.fps);
            g.video.kbps = g.video.kbps.max(f.video.kbps);
            if !f.info.path.is_empty() || !f.gps_log.path.is_empty() {
                if !f.info.path.is_empty() {
                    if !g.info.path.is_empty() {
                        g.info.path.push_str(", ");
                    }
                    g.info.path.push_str(&f.info.path);
                }
                if f.info.gps == 2 {
                    g.info.track_length += f.info.track_length;
                    if f.info.avg_speed > 0.0 {
                        gps_time += f.info.track_length / f.info.avg_speed;
                    }
                }
                g.info.gps = g.info.gps.max(f.info.gps);
                inc_block_time(&mut g.info.time, &f.info.time);
                g.info.gps_points += f.info.gps_points;
                g.info.gsensor_points += f.info.gsensor_points;
            }
        }
        g.info.path_lower = g.info.path.to_lowercase();
        g.video.lock = if locked == 0 {
            0
        } else if locked < n {
            2
        } else {
            1
        };
        g.info.avg_speed = if gps_time <= 0.0 { -1.0 } else { g.info.track_length / gps_time };
        it.group = g;
    }
    // 5.
    let dt = if it.group.video.time.duration <= 0.0 { 1.0 } else { it.group.video.time.duration };
    let finish = it.group.video.time.finish;
    for f in &mut it.files {
        for b in [&mut f.video.time, &mut f.info.time, &mut f.gps_log.time] {
            b.p_finish = 1.0 - clamp01((finish - b.finish) / dt);
            b.p_start = b.p_finish - clamp01(b.duration / dt);
        }
    }
}

/// Sort key of an item for column `column` (FL-025): numbers compare numerically, names
/// upper-cased (ASCII, like Delphi `UpperCase`) and ordinal, paths case-sensitive ordinal.
#[derive(Debug, Clone, PartialEq)]
pub enum SortKey {
    Num(f64),
    Text(String),
}

impl Eq for SortKey {}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (SortKey::Num(a), SortKey::Num(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (SortKey::Text(a), SortKey::Text(b)) => a.cmp(b),
            (SortKey::Num(_), SortKey::Text(_)) => Ordering::Less,
            (SortKey::Text(_), SortKey::Num(_)) => Ordering::Greater,
        }
    }
}

pub fn sort_key(it: &Item, column: i32) -> SortKey {
    let g = &it.group;
    match column {
        0 => SortKey::Text(g.video.face.to_ascii_uppercase()),
        1 => SortKey::Num(g.video.duration_ms as f64),
        3 => SortKey::Num(g.info.track_length),
        4 => SortKey::Num(g.info.avg_speed),
        5 => SortKey::Text(g.video.path.clone()),
        6 => SortKey::Num(g.video.time.size as f64),
        7 => SortKey::Num(f64::from(g.video.width) + f64::from(g.video.height)),
        _ => SortKey::Num(g.video.time.start()),
    }
}

/// Sorts items by a column. Stable: equal keys keep their current order (the original used a
/// non-stable quicksort; descending just reverses the comparison). Cheap when already sorted.
pub fn sort_items(items: &mut Vec<Item>, column: i32, ascending: bool) {
    let keys: Vec<SortKey> = items.iter().map(|it| sort_key(it, column)).collect();
    let cmp = |a: &SortKey, b: &SortKey| if ascending { a.cmp(b) } else { b.cmp(a) };
    if keys.windows(2).all(|w| cmp(&w[0], &w[1]) != Ordering::Greater) {
        return;
    }
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| cmp(&keys[a], &keys[b]));
    let mut old: Vec<Option<Item>> = std::mem::take(items).into_iter().map(Some).collect();
    *items = order.into_iter().filter_map(|i| old[i].take()).collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::SEC;

    fn file(path: &str, start: f64, dur_s: f64) -> FileEntry {
        let mut it = new_video_item(1, path, false, start, start + dur_s * SEC, 1 << 20);
        it.files.remove(0)
    }

    fn group(files: Vec<FileEntry>) -> Item {
        let mut it = Item { files, group: FileEntry::default() };
        calculate_group(&mut it);
        it
    }

    #[test]
    fn display_name_composition() {
        assert_eq!(group_display_name(&["FILE0001", "FILE0002", "FILE0003"]), "FILE0001-3");
        assert_eq!(group_display_name(&["A_0001", "A_0005"]), "A_0001, 5");
        assert_eq!(group_display_name(&["A_0001", "A_0002", "A_0005", "A_0006"]), "A_0001-2, 5-6");
        assert_eq!(group_display_name(&["clip", "other", "third"]), "clip (3)");
        assert_eq!(group_display_name(&["clip", "other", "x1"]), "clip (3)"); // "x1" is no neighbour of "other"
        assert_eq!(group_display_name(&["clip1", "clip7", "zz"]), "clip1, 7 (3)");
        let long = "N".repeat(60);
        let name = group_display_name(&[&long, "zzz"]);
        assert_eq!(name, format!("{}...", "N".repeat(50)));
        assert_eq!(name_index("FILE0001"), 1.0);
        assert_eq!(name_index("FILE123456"), -1.0);
        assert_eq!(name_index("X"), -1.0);
        assert!(faces_are_neighbours("FILE0009", "FILE0010"));
        assert!(!faces_are_neighbours("FILE0009", "FILX0010"));
    }

    #[test]
    fn add_video_file_face_quirks() {
        assert_eq!(video_face(r"C:\v\FI012345.MOV"), ("FILE2345.01".to_string(), false));
        assert_eq!(video_face(r"C:\v\FI000000.MOV").0, "FI000000");
        assert_eq!(video_face(r"C:\v\FILE0001.MOV").0, "FILE0001");
        assert_eq!(video_face(r"C:\v\fragment of X(1).mov"), ("X(fr)".to_string(), true));
        assert_eq!(
            video_face(r"C:\v\Fragment of 05161259_0066 (2).MOV"),
            ("05161259_0066 (fr)".to_string(), true)
        );
        assert_eq!(video_face(r"C:\v\Fragment of AB.MOV").0, "AB(fr)");
        assert_eq!(video_face(r"C:\v\noext").0, "noext");
    }

    #[test]
    fn add_video_file_times_and_no_group() {
        // 2016-05-16 12:00 local, a 60 s file
        let w = 42506.5;
        let it = new_video_item(1, r"C:\v\a.mov", true, w - 60.0 * SEC, w, 100);
        let f = &it.files[0];
        assert_eq!(f.video.lock, 1);
        assert_eq!(f.video.duration_state, 1);
        assert_eq!(f.video.duration_ms, 60_000);
        assert_eq!(f.video.time.finish, w);
        assert!(!f.video.no_group);
        assert_eq!(it.group.video.lock, 1);
        // longer than 4 h, or negative: no estimate
        let it = new_video_item(1, r"C:\v\a.mov", false, w - 0.5, w, 100);
        assert_eq!((it.files[0].video.time.duration, it.files[0].video.duration_state), (0.0, 0));
        let it = new_video_item(1, r"C:\v\a.mov", false, w + 0.1, w, 100);
        assert_eq!(it.files[0].video.time.duration, 0.0);
        // written on 1 January (2000-01-01 00:05 = 36526 + 5 min): camera clock never set
        let jan = 36526.0 + 5.0 / 1440.0;
        assert!(new_video_item(1, r"C:\v\a.mov", false, jan - SEC, jan, 1).files[0].video.no_group);
        // 31 December 13:00 + 12 h is 1 January too
        let dec31 = 36525.0 + 13.0 / 24.0;
        assert!(new_video_item(1, r"C:\v\a.mov", false, dec31 - SEC, dec31, 1).group.video.no_group);
        let jan_noon = 36526.0 + 13.0 / 24.0;
        assert!(!new_video_item(1, r"C:\v\a.mov", false, jan_noon - SEC, jan_noon, 1).group.video.no_group);
    }

    #[test]
    fn group_summary_and_progress_fractions() {
        let t0 = 42506.5;
        // three 60 s files; a 30 s gap before the third
        let mut a = file(r"C:\v\FILE0001.MOV", t0, 60.0);
        let b = file(r"C:\v\FILE0002.MOV", t0 + 60.0 * SEC, 60.0);
        let mut c = file(r"C:\v\FILE0003.MOV", t0 + 150.0 * SEC, 60.0);
        a.video.lock = 1;
        a.info = GpsInfo { path: "a.gpx".into(), gps: 2, track_length: 1000.0, avg_speed: 60.0, ..Default::default() };
        c.info = GpsInfo { path: "c.gpx".into(), gps: 2, track_length: 2000.0, avg_speed: 40.0, ..Default::default() };
        let it = group(vec![c, a, b]);
        let paths: Vec<&str> = it.files.iter().map(|f| f.video.path.as_str()).collect();
        assert_eq!(paths, [r"C:\v\FILE0001.MOV", r"C:\v\FILE0002.MOV", r"C:\v\FILE0003.MOV"]);
        let g = &it.group;
        assert_eq!(g.video.face, "FILE0001-3");
        assert_eq!(g.video.path, r"C:\v\FILE0001.MOV, C:\v\FILE0002.MOV, C:\v\FILE0003.MOV");
        assert_eq!(g.video.duration_ms, 180_000);
        assert_eq!(g.video.time.size, 3 << 20);
        assert_eq!(g.video.duration_state, 1);
        assert_eq!(g.video.lock, 2);
        assert!((g.video.time.duration - 210.0 * SEC).abs() < 1e-9);
        assert_eq!(g.info.path, "a.gpx, c.gpx");
        assert_eq!(g.info.track_length, 3000.0);
        // 3000 m over (1000/60 + 2000/40) "hours"
        assert!((g.info.avg_speed - 3000.0 / (1000.0 / 60.0 + 2000.0 / 40.0)).abs() < 1e-9);
        let p: Vec<(f64, f64)> = it.files.iter().map(|f| (f.video.time.p_start, f.video.time.p_finish)).collect();
        let close = |x: f64, y: f64| (x - y).abs() < 1e-6;
        assert!(close(p[0].0, 0.0) && close(p[0].1, 60.0 / 210.0));
        assert!(close(p[1].0, 60.0 / 210.0) && close(p[1].1, 120.0 / 210.0));
        assert!(close(p[2].0, 150.0 / 210.0) && close(p[2].1, 1.0));

        // all locked -> 1; no GPS -> avg speed none
        let mut x = file(r"C:\v\B_0001.MOV", t0, 10.0);
        let mut y = file(r"C:\v\B_0005.MOV", t0 + 20.0 * SEC, 10.0);
        x.video.lock = 1;
        y.video.lock = 1;
        let it = group(vec![x, y]);
        assert_eq!(it.group.video.lock, 1);
        assert_eq!(it.group.info.avg_speed, -1.0);
        assert_eq!(it.group.video.face, "B_0001, 5");
    }

    #[allow(clippy::too_many_arguments)]
    fn item_with(face: &str, path: &str, start: f64, dur_s: f64, size: i64, track: f64, speed: f64, w: u32) -> Item {
        let mut f = file(path, start, dur_s);
        f.video.face = face.into();
        f.video.time.size = size;
        f.video.width = w;
        f.video.height = w / 2;
        f.info.track_length = track;
        f.info.avg_speed = speed;
        Item::single(f)
    }

    #[test]
    fn sorting_by_each_column() {
        let t0 = 42506.5;
        let mut items = vec![
            item_with("beta", r"C:\b.mov", t0 + 100.0 * SEC, 30.0, 300, 5.0, 10.0, 640),
            item_with("Alpha", r"C:\c.mov", t0 + 300.0 * SEC, 10.0, 100, 1.0, 30.0, 1920),
            item_with("gamma", r"C:\A.mov", t0, 20.0, 200, 3.0, 20.0, 1280),
        ];
        let order = |items: &[Item]| items.iter().map(|i| i.group.video.face.clone()).collect::<Vec<_>>();
        let expect = [
            (0, ["Alpha", "beta", "gamma"]),
            (1, ["Alpha", "gamma", "beta"]),
            (2, ["gamma", "beta", "Alpha"]),
            (3, ["Alpha", "gamma", "beta"]),
            (4, ["beta", "gamma", "Alpha"]),
            (5, ["gamma", "beta", "Alpha"]), // "C:\A" < "C:\b" < "C:\c" (case-sensitive)
            (6, ["Alpha", "gamma", "beta"]),
            (7, ["beta", "gamma", "Alpha"]),
        ];
        for (col, want) in expect {
            sort_items(&mut items, col, true);
            assert_eq!(order(&items), want, "column {col}");
            sort_items(&mut items, col, false);
            let mut rev = want.to_vec();
            rev.reverse();
            assert_eq!(order(&items), rev, "column {col} descending");
        }
        // equal keys keep their order (stable)
        let mut items = vec![
            item_with("x1", r"C:\1.mov", t0, 10.0, 100, 0.0, 0.0, 640),
            item_with("x2", r"C:\2.mov", t0 + SEC, 10.0, 100, 0.0, 0.0, 640),
            item_with("x3", r"C:\3.mov", t0 + 2.0 * SEC, 10.0, 100, 0.0, 0.0, 640),
        ];
        sort_items(&mut items, 6, false);
        assert_eq!(order(&items), ["x1", "x2", "x3"]);
    }
}
