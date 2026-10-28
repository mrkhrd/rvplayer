//! Grouping (spec §4.6) and clock fixing (§4.11). All functions work on the item list; the
//! caller sorts afterwards.

use crate::model::{FileEntry, Item, TimeBlock, VideoFile, calculate_group, inc_block_time, sort_items};
use crate::time::{NO_DATE, SEC};

const FOUR_HOURS: f64 = 1.0 / 6.0;
const SIX_MINUTES: f64 = 6.0 / 1440.0;
const SIX_HOURS: f64 = 0.25;

/// `GetFileDistance`: compares the two paths up to their last "."; distance = (longer length -
/// common prefix) + 10 for every `\` in either remainder. Identical paths give -1.
pub fn file_distance(a: &str, b: &str) -> i32 {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    // 1-based position of the last '.', or the whole length.
    let l1 = a.iter().rposition(|&c| c == b'.').map_or(a.len(), |p| p + 1);
    let l2 = b.iter().rposition(|&c| c == b'.').map_or(b.len(), |p| p + 1);
    let mut n = 1;
    while n <= l1 && n <= l2 && a[n - 1] == b[n - 1] {
        n += 1;
    }
    let mut r = l1.max(l2) as i32 - n as i32;
    if r > 0 {
        for k in n..n + r as usize {
            if k <= l1 && a[k - 1] == b'\\' {
                r += 10;
            }
            if k <= l2 && b[k - 1] == b'\\' {
                r += 10;
            }
        }
    }
    r
}

/// `GetVideoFileDistance`: file distance + 1000 when the picture sizes differ.
pub fn video_file_distance(a: &VideoFile, b: &VideoFile) -> i32 {
    let d = file_distance(&a.path, &b.path);
    if a.width != b.width || a.height != b.height { d + 1000 } else { d }
}

/// `CheckIt`.
fn check_it(s1: f64, f1: f64, s2: f64, f2: f64, dist: f64) -> bool {
    let gap = s2 - f1;
    !(gap < -dist / 2.0
        || gap > dist
        || f1 - s1 <= SEC
        || f2 - s2 <= SEC
        || s2 - s1 <= SEC
        || f2 - f1 <= SEC)
}

/// `IsConnectedByTime`: with file-system times first, else with real times.
pub fn connected_by_time(t1: &TimeBlock, t2: &TimeBlock, dist: f64) -> bool {
    check_it(t1.creation, t1.write, t2.creation, t2.write, dist)
        || check_it(t1.start(), t1.finish, t2.start(), t2.finish, dist)
}

/// `FindGroupsFast` (§4.6.1): four passes (3 s / 10 s link distance, sorted by full path / by
/// name) merging adjacent items whose boundary files connect.
pub fn find_groups_fast(items: &mut Vec<Item>) {
    for dist in [3.0 * SEC, 10.0 * SEC] {
        for column in [5, 0] {
            sort_items(items, column, true);
            let mut out: Vec<Item> = Vec::with_capacity(items.len());
            let mut changed = false;
            for it in std::mem::take(items) {
                if let Some(last) = out.last_mut() {
                    let (a, b) = (last.files.last(), it.files.first());
                    if let (Some(a), Some(b)) = (a, b)
                        && connected_by_time(&a.video.time, &b.video.time, dist)
                        && video_file_distance(&a.video, &b.video) <= 99
                    {
                        last.files.extend(it.files);
                        changed = true;
                        continue;
                    }
                    if changed {
                        calculate_group(last);
                        changed = false;
                    }
                }
                out.push(it);
            }
            if changed && let Some(last) = out.last_mut() {
                calculate_group(last);
            }
            *items = out;
        }
    }
}

fn item_window(it: &Item) -> (f64, f64) {
    let t = &it.group.video.time;
    let s = if t.duration == 0.0 { 0.0 } else { t.finish - t.duration };
    (s, t.finish)
}

/// `GetL` (§4.6.2): link length in days between items i and z; 1.0 means "no link".
pub fn link_length(items: &[Item], i: usize, z: usize) -> f64 {
    if i == z || items[i].group.video.no_group || items[z].group.video.no_group {
        return 1.0;
    }
    let (s1, f1) = item_window(&items[i]);
    let (s2, f2) = item_window(&items[z]);
    if f1 <= s1 || f2 <= s2 || s1 <= NO_DATE || s2 <= NO_DATE {
        return 1.0;
    }
    let h = 0.5 * SEC;
    if (s1 - h <= s2 && f2 <= h + f1) || (s2 - h <= s1 && f1 <= h + f2) {
        return 1.0; // one span contains the other
    }
    let (Some(a), Some(b)) = (items[i].files.last(), items[z].files.first()) else { return 1.0 };
    let mut d = video_file_distance(&a.video, &b.video);
    if d == 1 {
        d = 0;
    }
    let w = f64::from(d) / 10.0 + 1.0;
    if !(s2 <= 0.0 || f1 <= s2 || f2 <= f1) {
        // z starts inside i and ends after it
        let l = w * (f1 - s2);
        return if l > 60.0 * SEC { 1.0 } else { l };
    }
    if !(s1 <= 0.0 || f2 < s1 || f1 < f2) {
        // z starts before i and ends inside it
        let l = w * (f2 - s1);
        return if l > 60.0 * SEC { 1.0 } else { l };
    }
    let mut r = 1.0f64;
    if f1 > 0.0 {
        r = r.min((f1 - s2).abs());
    }
    if f2 > 0.0 {
        r = r.min((f2 - s1).abs());
    }
    // single precision in the original
    f64::from((d as f32 / 10.0 + 1.0) * r as f32)
}

fn qualifies(items: &[Item], i: usize, z: usize, l: f64) -> bool {
    l < SIX_MINUTES
        || (l < SIX_HOURS
            && (items[i].group.video.time.duration > 4.0 * l || items[z].group.video.time.duration > 4.0 * l))
}

/// The qualifying pair (i < z) with the smallest L; ties go to the first pair in (i, z) order,
/// like the original's double loop. Only pairs whose windows are less than `MARGIN` apart are
/// evaluated: every other pair has L >= 0.9 * gap > 6 h and cannot qualify.
fn best_pair(items: &[Item]) -> Option<(usize, usize)> {
    const MARGIN: f64 = 0.3;
    let eligible = |k: usize| {
        let (s, f) = item_window(&items[k]);
        !items[k].group.video.no_group && s > NO_DATE && f > s
    };
    let mut order: Vec<usize> = (0..items.len()).filter(|&k| eligible(k)).collect();
    order.sort_by(|&a, &b| item_window(&items[a]).0.total_cmp(&item_window(&items[b]).0));
    let mut best: Option<(f64, usize, usize)> = None;
    for (p, &a) in order.iter().enumerate() {
        let fa = item_window(&items[a]).1;
        for &b in &order[p + 1..] {
            if item_window(&items[b]).0 >= fa + MARGIN {
                break;
            }
            let (i, z) = (a.min(b), a.max(b));
            let l = link_length(items, i, z);
            if l >= 0.9 || !qualifies(items, i, z, l) {
                continue;
            }
            let better = match best {
                None => true,
                Some((bl, bi, bz)) => l < bl || (l == bl && (i, z) < (bi, bz)),
            };
            if better {
                best = Some((l, i, z));
            }
        }
    }
    best.map(|(_, i, z)| (i, z))
}

/// `FindGroupsAuto` (§4.6.2).
pub fn find_groups_auto(items: &mut Vec<Item>) {
    find_groups_fast(items);
    fix_clock_changed(items);
    shifts_without_gps(items);
    ungroup_errors(items);
    let mut merged = false;
    while let Some((i, z)) = best_pair(items) {
        group_items(items, &[i, z], false);
        merged = true;
    }
    if merged {
        items.iter_mut().for_each(calculate_group);
        fix_clock_changed(items);
        shifts_without_gps(items);
        ungroup_errors(items);
    }
    items.iter_mut().for_each(calculate_group);
}

/// `UngroupErrors` (§4.6.3): splits groups at consecutive files more than 12 h apart.
pub fn ungroup_errors(items: &mut Vec<Item>) {
    let mut i = 0;
    while i < items.len() {
        let files = &items[i].files;
        let split = (0..files.len().saturating_sub(1)).find(|&z| {
            let (t1, t2) = (&files[z].video.time, &files[z + 1].video.time);
            (t1.finish - t2.finish + t2.duration).abs() > 0.5
        });
        if let Some(z) = split {
            let tail = items[i].files.split_off(z + 1);
            calculate_group(&mut items[i]);
            let mut it = Item { files: tail, group: FileEntry::default() };
            calculate_group(&mut it);
            items.push(it);
        }
        i += 1;
    }
}

/// `GroupItems` (§4.6.4): appends the files of every other valid index to the first (target)
/// and removes the sources. Automatic merges union the target's time block first; manual
/// grouping recomputes the target. Returns the target's new index.
pub fn group_items(items: &mut Vec<Item>, indices: &[usize], manual: bool) -> Option<usize> {
    let &target = indices.first()?;
    if target >= items.len() {
        return None;
    }
    let mut sources: Vec<usize> = Vec::new();
    for &k in &indices[1..] {
        if k != target && k < items.len() && !sources.contains(&k) {
            sources.push(k);
        }
    }
    for &k in &sources {
        if !manual {
            let src = items[k].group.video.time.clone();
            inc_block_time(&mut items[target].group.video.time, &src);
        }
        let files = std::mem::take(&mut items[k].files);
        items[target].files.extend(files);
    }
    let mut desc = sources.clone();
    desc.sort_unstable_by(|a, b| b.cmp(a));
    for k in desc {
        items.remove(k);
    }
    let target = target - sources.iter().filter(|&&k| k < target).count();
    if manual {
        calculate_group(&mut items[target]);
    }
    Some(target)
}

/// `UngroupItem` (FL-056): the first file stays, every other file becomes its own item
/// (appended at the end).
pub fn ungroup_item(items: &mut Vec<Item>, index: usize) {
    let Some(it) = items.get_mut(index) else { return };
    if it.files.len() < 2 {
        return;
    }
    let rest = it.files.split_off(1);
    calculate_group(it);
    items.extend(rest.into_iter().map(Item::single));
}

/// `UngroupAll` (FL-058).
pub fn ungroup_all(items: &mut Vec<Item>) {
    let files: Vec<FileEntry> = std::mem::take(items).into_iter().flat_map(|it| it.files).collect();
    *items = files.into_iter().map(Item::single).collect();
}

/// `GetClockShift`: GPS finish - file write time, or 0 when the GPS window lies inside the file
/// window or the dates are invalid.
pub fn clock_shift(t: &TimeBlock) -> f64 {
    let c = if (t.duration > 0.0 && t.write - t.creation < t.duration) || t.write - t.creation > NO_DATE {
        t.write - t.duration
    } else {
        t.creation
    };
    if t.write < NO_DATE || t.finish < NO_DATE || (c <= t.finish - t.duration && t.finish <= t.write) {
        0.0
    } else {
        t.finish - t.write
    }
}

/// `SyncVideoByGPS` (§4.11): aligns the video window to an attached GPS info or log.
pub fn sync_video_by_gps(f: &mut FileEntry) {
    let has_info = !f.info.path.is_empty();
    if f.video.time.duration <= 0.0
        && f.video.duration_state != 3
        && has_info
        && f.info.gps != 0
        && f.info.time.duration > 0.0
    {
        f.video.time.duration = f.info.time.duration;
    }
    let dv = f.video.time.duration;
    if dv <= 0.0 {
        return;
    }
    let shift = if has_info {
        clock_shift(&f.info.time)
    } else if !f.gps_log.path.is_empty() {
        clock_shift(&f.gps_log.time)
    } else {
        0.0
    };
    f.video.time.finish = f.video.time.write + shift;
    let di = f.info.time.duration;
    if has_info && di > 0.0 {
        if (dv - di).abs() < 11.0 * SEC || (dv / (di + dv) - 0.5).abs() < 0.01 {
            f.video.time.finish = f.info.time.finish;
            f.info.time.duration = f.video.time.duration;
        } else if f.video.time.finish <= f.info.time.finish - di || f.info.time.finish < f.video.time.finish - dv {
            f.video.time.finish = f.info.time.finish;
        }
    }
}

/// `MV`: shifts `x` by `d` if it is a date and stays within `max`.
fn mv(x: &mut f64, d: f64, max: f64, changed: &mut bool) -> bool {
    if *x > NO_DATE && *x + d <= max {
        *changed = true;
        *x += d;
        true
    } else {
        false
    }
}

/// `FixClockChanged` (§4.11): within an item, the first exact file whose write - creation
/// exceeds 4 h while its duration is under 4 h defines a correction for the other files.
pub fn fix_clock_changed(items: &mut [Item]) {
    for it in items.iter_mut() {
        let mut changed = false;
        let mut found = None;
        for (z, f) in it.files.iter_mut().enumerate() {
            let t = &mut f.video.time;
            if t.write - t.creation > FOUR_HOURS
                && f.video.duration_state == 3
                && t.duration > 0.0
                && t.duration < FOUR_HOURS
            {
                let d = (t.write - t.creation) - t.duration;
                t.creation += d;
                found = Some((z, d));
                break;
            }
        }
        let Some((zz, d)) = found else { continue };
        let (tz_creation, tz_finish) = (it.files[zz].video.time.creation, it.files[zz].video.time.finish);
        for (z, f) in it.files.iter_mut().enumerate() {
            if z == zz {
                continue;
            }
            if mv(&mut f.video.time.creation, d, tz_creation, &mut changed) {
                mv(&mut f.video.time.write, d, tz_creation, &mut changed);
                if mv(&mut f.video.time.finish, d, tz_finish, &mut changed) {
                    mv(&mut f.info.time.finish, d, tz_finish, &mut changed);
                    mv(&mut f.gps_log.time.finish, d, tz_finish, &mut changed);
                }
            }
            mv(&mut f.info.time.creation, d, tz_creation, &mut changed);
            mv(&mut f.info.time.write, d, tz_creation, &mut changed);
            mv(&mut f.gps_log.time.creation, d, tz_creation, &mut changed);
            mv(&mut f.gps_log.time.write, d, tz_creation, &mut changed);
        }
        if changed {
            calculate_group(it);
        }
    }
}

/// `GetMedian`: element n/2 of the sorted values.
fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// `DoShiftLikeNear`: shifts file `zz` by the median clock shift of its siblings with better GPS.
fn shift_like_near(files: &mut [FileEntry], zz: usize) -> bool {
    let target_gps = files[zz].info.gps;
    let shifts: Vec<f64> = files
        .iter()
        .enumerate()
        .filter(|&(z, f)| {
            z != zz
                && !f.info.path.is_empty()
                && target_gps < f.info.gps
                && f.info.time.write > NO_DATE
                && f.video.time.write > NO_DATE
        })
        .map(|(_, f)| clock_shift(&f.info.time))
        .collect();
    if shifts.is_empty() {
        return false;
    }
    let med = median(shifts);
    if med == 0.0 {
        return false;
    }
    let f = &mut files[zz];
    let d = if f.info.path.is_empty() {
        med - clock_shift(&f.video.time)
    } else {
        let d = med - clock_shift(&f.info.time);
        f.info.time.finish += d;
        d
    };
    f.video.time.finish += d;
    true
}

/// `ShiftsWithoutGPS` (§4.11).
pub fn shifts_without_gps(items: &mut [Item]) {
    for it in items.iter_mut() {
        if it.files.len() < 2 {
            continue;
        }
        let mut changed = false;
        for z in 0..it.files.len() {
            let f = &it.files[z];
            if (f.info.gps == 1 || f.gps_log.gps == 1) && shift_like_near(&mut it.files, z) {
                changed = true;
            }
        }
        for z in 0..it.files.len() {
            let f = &it.files[z];
            if f.info.path.is_empty() && f.gps_log.path.is_empty() && shift_like_near(&mut it.files, z) {
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
    use crate::model::new_video_item;

    const T0: f64 = 42506.5; // 2016-05-16 12:00

    /// A previewed file: fs window [start, start + dur], exact duration, given size.
    fn file(path: &str, start: f64, dur_s: f64, w: u32) -> FileEntry {
        let mut f = new_video_item(1, path, false, start, start + dur_s * SEC, 1).files.remove(0);
        f.video.duration_state = 3;
        f.video.width = w;
        f.video.height = w * 9 / 16;
        f
    }

    fn names(items: &[Item]) -> Vec<String> {
        let mut v: Vec<String> = items.iter().map(|i| i.group.video.face.clone()).collect();
        v.sort();
        v
    }

    #[test]
    fn distances() {
        assert_eq!(file_distance(r"C:\v\FILE0001.MOV", r"C:\v\FILE0002.MOV"), 1);
        assert_eq!(file_distance(r"C:\v\FILE0009.MOV", r"C:\v\FILE0010.MOV"), 2);
        assert_eq!(file_distance(r"C:\v\FILE0001.MOV", r"C:\v\FILE0001.MOV"), -1);
        // a different folder: 7 differing characters + 10 for each of the 3 backslashes
        assert_eq!(file_distance(r"C:\a\F1.MOV", r"C:\bb\x\F1.MOV"), 7 + 30);
    }

    #[test]
    fn fast_grouping_merges_consecutive_segments() {
        // 3 s apart: merges; the fourth starts 20 s later: does not
        let items: Vec<Item> = [(0.0, "1"), (63.0, "2"), (126.0, "3"), (206.0, "4")]
            .iter()
            .map(|&(s, n)| Item::single(file(&format!(r"C:\v\FILE000{n}.MOV"), T0 + s * SEC, 60.0, 1920)))
            .collect();
        let mut items = items;
        find_groups_fast(&mut items);
        assert_eq!(names(&items), ["FILE0001-3", "FILE0004"]);
        let g = items.iter().find(|i| i.files.len() == 3).unwrap();
        assert_eq!(g.group.video.path, r"C:\v\FILE0001.MOV, C:\v\FILE0002.MOV, C:\v\FILE0003.MOV");
    }

    #[test]
    fn fast_grouping_never_merges_different_resolutions() {
        let mut items = vec![
            Item::single(file(r"C:\v\FILE0001.MOV", T0, 60.0, 1920)),
            Item::single(file(r"C:\v\FILE0002.MOV", T0 + 60.0 * SEC, 60.0, 1280)),
        ];
        find_groups_fast(&mut items);
        assert_eq!(items.len(), 2);
        // a gap of 3..10 s merges in the 10 s pass; overlaps up to dist/2 are allowed
        let mut items = vec![
            Item::single(file(r"C:\v\FILE0001.MOV", T0, 60.0, 1920)),
            Item::single(file(r"C:\v\FILE0002.MOV", T0 + 68.0 * SEC, 60.0, 1920)),
            Item::single(file(r"C:\v\FILE0003.MOV", T0 + 124.0 * SEC, 60.0, 1920)),
        ];
        find_groups_fast(&mut items);
        assert_eq!(names(&items), ["FILE0001-3"]);
    }

    #[test]
    fn link_length_qualification() {
        let mk = |path: &str, s: f64, d: f64| Item::single(file(path, T0 + s * SEC, d, 1920));
        // disjoint, 30 s gap, neighbouring names (distance 1 -> 0): L = 30 s
        let items = vec![mk(r"C:\v\A0001.MOV", 0.0, 60.0), mk(r"C:\v\A0002.MOV", 90.0, 60.0)];
        let l = link_length(&items, 0, 1);
        assert!((l - 30.0 * SEC).abs() < 1e-7, "{}", l / SEC);
        assert!(qualifies(&items, 0, 1, l));
        // containment within 0.5 s: no link
        let items = vec![mk(r"C:\v\A0001.MOV", 0.0, 60.0), mk(r"C:\v\A0002.MOV", 10.0, 20.0)];
        assert_eq!(link_length(&items, 0, 1), 1.0);
        // overlap: z starts inside i and ends after it -> overlap length
        let items = vec![mk(r"C:\v\A0001.MOV", 0.0, 60.0), mk(r"C:\v\A0002.MOV", 50.0, 60.0)];
        assert!((link_length(&items, 0, 1) - 10.0 * SEC).abs() < 1e-9);
        // 1 h gap between 10-minute items: L*4 > both durations -> not qualifying
        let items = vec![mk(r"C:\v\A0001.MOV", 0.0, 600.0), mk(r"C:\v\A0002.MOV", 4200.0, 600.0)];
        let l = link_length(&items, 0, 1);
        assert!((l - 3600.0 * SEC).abs() < 1e-6);
        assert!(!qualifies(&items, 0, 1, l));
        // 30 min gap after a 3 h item: qualifies (6 min <= L < 6 h and 4L < 3 h)
        let items = vec![mk(r"C:\v\A0001.MOV", 0.0, 10_800.0), mk(r"C:\v\A0002.MOV", 12_600.0, 600.0)];
        let l = link_length(&items, 0, 1);
        assert!(l >= SIX_MINUTES && qualifies(&items, 0, 1, l));
        // noGroup never links
        let mut items = vec![mk(r"C:\v\A0001.MOV", 0.0, 60.0), mk(r"C:\v\A0002.MOV", 90.0, 60.0)];
        items[1].group.video.no_group = true;
        assert_eq!(link_length(&items, 0, 1), 1.0);
    }

    #[test]
    fn auto_grouping_merges_closest_pairs() {
        let mk = |path: &str, s: f64, d: f64| Item::single(file(path, T0 + s * SEC, d, 1920));
        let mut items = vec![
            mk(r"C:\v\A0003.MOV", 200.0, 60.0),
            mk(r"C:\v\A0001.MOV", 0.0, 60.0),
            mk(r"C:\v\A0002.MOV", 90.0, 60.0),
            mk(r"C:\v\B0001.MOV", 20_000.0, 60.0), // 5.5 h later: 4L exceeds both durations
        ];
        find_groups_auto(&mut items);
        assert_eq!(names(&items), ["A0001-3", "B0001"]);
    }

    #[test]
    fn split_erroneous_groups() {
        let a = file(r"C:\v\FILE0001.MOV", T0, 60.0, 1920);
        let b = file(r"C:\v\FILE0002.MOV", T0 + 60.0 * SEC, 60.0, 1920);
        let c = file(r"C:\v\FILE0003.MOV", T0 + 0.6, 60.0, 1920); // 14.4 h later
        let d = file(r"C:\v\FILE0004.MOV", T0 + 0.6 + 60.0 * SEC, 60.0, 1920);
        let mut it = Item { files: vec![a, b, c, d], group: FileEntry::default() };
        calculate_group(&mut it);
        let mut items = vec![it];
        ungroup_errors(&mut items);
        assert_eq!(names(&items), ["FILE0001-2", "FILE0003-4"]);
    }

    #[test]
    fn manual_group_and_ungroup() {
        let mk = |path: &str, s: f64| Item::single(file(path, T0 + s * SEC, 60.0, 1920));
        let mut items = vec![mk(r"C:\v\X1.MOV", 0.0), mk(r"C:\v\Y.MOV", 5000.0), mk(r"C:\v\X2.MOV", 70.0)];
        let t = group_items(&mut items, &[2, 0, 0, 9], true).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(t, 1);
        assert_eq!(items[1].group.video.face, "X1-2");
        ungroup_item(&mut items, 1);
        assert_eq!(names(&items), ["X1", "X2", "Y"]);
        let t = group_items(&mut items, &[0, 1, 2], true).unwrap();
        assert_eq!((t, items.len(), items[0].files.len()), (0, 1, 3));
        ungroup_all(&mut items);
        assert_eq!(names(&items), ["X1", "X2", "Y"]);
    }

    #[test]
    fn clock_shift_and_sync() {
        // video file window [T0, T0+60s], GPS says the recording finished 1 h later
        let mut f = file(r"C:\v\a.mov", T0, 60.0, 1920);
        f.info.set_path(r"C:\v\a.gpx");
        f.info.gps = 2;
        f.info.time = TimeBlock {
            creation: T0,
            write: T0 + 60.0 * SEC,
            duration: 58.0 * SEC,
            finish: T0 + 3660.0 * SEC,
            ..Default::default()
        };
        assert!((clock_shift(&f.info.time) - 3600.0 * SEC).abs() < 1e-9);
        sync_video_by_gps(&mut f);
        assert!((f.video.time.finish - (T0 + 3660.0 * SEC)).abs() < 1e-9);
        assert_eq!(f.info.time.duration, f.video.time.duration);
        // GPS window inside the file window: no shift
        let t = TimeBlock { creation: T0, write: T0 + 60.0 * SEC, duration: 30.0 * SEC, finish: T0 + 50.0 * SEC, ..Default::default() };
        assert_eq!(clock_shift(&t), 0.0);
    }

    #[test]
    fn fix_clock_changed_moves_earlier_files() {
        // file 2 was created with an unset clock (creation 5 h before write, duration 60 s);
        // file 1 ended before it with the same wrong clock.
        let a = file(r"C:\v\FILE0001.MOV", T0 - 5.0 / 24.0 - 70.0 * SEC, 60.0, 1920);
        let mut b = file(r"C:\v\FILE0002.MOV", T0, 60.0, 1920);
        b.video.time.creation = T0 + 60.0 * SEC - 5.0 / 24.0;
        let d = (b.video.time.write - b.video.time.creation) - b.video.time.duration;
        let mut it = Item { files: vec![a, b], group: FileEntry::default() };
        calculate_group(&mut it);
        let before = it.files[0].video.time.write;
        let mut items = vec![it];
        fix_clock_changed(&mut items);
        let f = items[0].files.iter().find(|f| f.video.path.ends_with("0001.MOV")).unwrap();
        assert!((f.video.time.write - (before + d)).abs() < 1e-9);
    }
}
