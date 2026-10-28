//! Sidecar GPS log lookup for a video (spec §4.5, `UnitGPSManager.FindFilesForVideo`) with the
//! cached, case-insensitive directory listing (`UnitFileListCache`).

use std::collections::HashMap;
use std::fs;

use crate::gps::GPS_EXTENSIONS;
use crate::time::{SEC, fs_times};

/// Lower-cased file names of a folder with their creation / last-write times.
#[derive(Debug, Default)]
pub struct DirCache {
    dirs: HashMap<String, HashMap<String, (f64, f64)>>,
}

fn split_dir(path: &str) -> (&str, &str) {
    match path.rfind(['\\', '/']) {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => ("", path),
    }
}

impl DirCache {
    pub fn clear(&mut self) {
        self.dirs.clear();
    }

    fn dir(&mut self, dir: &str) -> &HashMap<String, (f64, f64)> {
        let key = dir.trim_end_matches(['\\', '/']).to_lowercase();
        self.dirs.entry(key).or_insert_with(|| {
            let lookup = if dir.is_empty() { "." } else { dir };
            let mut m = HashMap::new();
            if let Ok(rd) = fs::read_dir(lookup) {
                for e in rd.flatten() {
                    if let Ok(meta) = e.metadata()
                        && !meta.is_dir()
                    {
                        m.insert(e.file_name().to_string_lossy().to_lowercase(), fs_times(&meta));
                    }
                }
            }
            m
        })
    }

    /// Creation and write times of an existing file (case-insensitive name match).
    pub fn lookup(&mut self, path: &str) -> Option<(f64, f64)> {
        let (dir, name) = split_dir(path);
        let name = name.to_lowercase();
        self.dir(dir).get(&name).copied()
    }

    pub fn exists(&mut self, path: &str) -> bool {
        self.lookup(path).is_some()
    }

    /// `GetFileTime`: (start, finish) of a file; a missing or implausible start becomes
    /// finish - 1 min.
    fn file_window(&mut self, path: Option<&str>) -> (f64, f64) {
        let (s, f) = path.and_then(|p| self.lookup(p)).unwrap_or((0.0, 0.0));
        if f == 0.0 {
            (0.0, f)
        } else if f <= s || f - s >= 1.0 / 6.0 {
            (f - 60.0 * SEC, f)
        } else {
            (s, f)
        }
    }
}

/// `CutoffEnd`: the path without its last component (and without a trailing separator).
fn cutoff_end(s: &str) -> String {
    let s = s.strip_suffix(['\\', '/']).unwrap_or(s);
    match s.rfind(['\\', '/']) {
        Some(i) => s[..i].to_string(),
        None => String::new(),
    }
}

/// `FindFileExtDirect`: `face` is the video path without extension.
fn find_direct(face: &str, only_current_folder: bool, ext: &str, cache: &mut DirCache) -> Option<String> {
    let dir = cutoff_end(face);
    let name: &str = if dir.is_empty() { face } else { &face[dir.len() + 1..] };
    let folders = if only_current_folder {
        vec![dir.clone()]
    } else {
        let parent = cutoff_end(&dir);
        vec![format!("{dir}\\GPS"), format!("{parent}\\INFO"), format!("{}\\gps", cutoff_end(&parent))]
    };
    let chars: Vec<char> = name.chars().collect();
    let faces = [
        name.to_string(),
        format!("{name}."),
        format!("gps{}", chars.iter().skip(4).collect::<String>()),
        chars[..chars.len().saturating_sub(1)].iter().collect(),
    ];
    for folder in &folders {
        for f in &faces {
            let candidate = format!("{folder}\\{f}.{ext}");
            if cache.exists(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// `GetNearFile` with Q-19 applied: bumps the last run of digits in the *file name without
/// extension* by `delta`, keeping the zero padding. `has_extension` says whether `path` ends with
/// an extension that must be kept unchanged.
pub fn near_file(path: &str, delta: i64, has_extension: bool) -> Option<String> {
    let name_start = path.rfind(['\\', '/']).map_or(0, |i| i + 1);
    let stem_end = if has_extension {
        path[name_start..].rfind('.').map_or(path.len(), |i| name_start + i)
    } else {
        path.len()
    };
    let stem = &path[name_start..stem_end];
    let b = stem.as_bytes();
    let last = b.iter().rposition(|c| c.is_ascii_digit())?;
    let first = b[..last].iter().rposition(|c| !c.is_ascii_digit()).map_or(0, |i| i + 1);
    let run = &stem[first..=last];
    let n: i64 = run.parse().unwrap_or(0) + delta;
    let digits = if n < 0 {
        format!("-{:0width$}", -n, width = run.len().saturating_sub(1))
    } else {
        format!("{n:0width$}", width = run.len())
    };
    Some(format!(
        "{}{}{}{}",
        &path[..name_start + first],
        digits,
        &stem[last + 1..],
        &path[stem_end..]
    ))
}

fn overlap((s1, f1): (f64, f64), (s2, f2): (f64, f64)) -> f64 {
    f1.min(f2) - s1.max(s2)
}

/// `FindFileExt`: the sidecar with extension `ext` for a video, choosing between this
/// segment's log and the previous segment's log by overlap.
fn find_ext(video: &str, only_current_folder: bool, ext: &str, cache: &mut DirCache) -> Option<String> {
    let name_start = video.rfind(['\\', '/']).map_or(0, |i| i + 1);
    let face = match video[name_start..].rfind('.') {
        Some(i) => &video[..name_start + i],
        None => video,
    };
    let mut gps = find_direct(face, only_current_folder, ext, cache);
    let gps_prev = match &gps {
        None => near_file(face, -1, false).and_then(|f| find_direct(&f, only_current_folder, ext, cache)),
        Some(g) => near_file(g, -1, true).filter(|p| cache.exists(p)),
    };
    if let Some(g) = &gps
        && let Some(next_video) = near_file(video, 1, true).filter(|p| cache.exists(p))
    {
        let v = cache.file_window(Some(video));
        let w = cache.file_window(Some(g));
        let v1 = cache.file_window(Some(&next_video));
        if overlap(v, w) < 2.0 * SEC && overlap(v1, w) > 2.0 * SEC {
            gps = None; // the log belongs to the next segment
        }
    }
    let Some(prev) = gps_prev else { return gps };
    let v = cache.file_window(Some(video));
    let vg = overlap(v, cache.file_window(gps.as_deref()));
    let vg_prev = overlap(v, cache.file_window(Some(&prev)));
    if vg_prev <= 0.0 || vg_prev <= vg { gps } else { Some(prev) }
}

/// `FindFilesForVideo`: candidates from the video's own folder first, then from the GPS
/// sub-folders, one per extension in [`GPS_EXTENSIONS`] order.
pub fn find_files_for_video(video: &str, cache: &mut DirCache) -> Vec<String> {
    let mut out = Vec::new();
    for only_current_folder in [true, false] {
        for ext in GPS_EXTENSIONS {
            if let Some(s) = find_ext(video, only_current_folder, ext, cache)
                && cache.exists(&s)
            {
                out.push(s);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_file_bumps_the_name_number_only() {
        assert_eq!(near_file(r"C:\v\FILE0009.MP4", 1, true).unwrap(), r"C:\v\FILE0010.MP4");
        assert_eq!(near_file(r"C:\v\FILE0010.MP4", -1, true).unwrap(), r"C:\v\FILE0009.MP4");
        assert_eq!(near_file(r"C:\v1\FILE0001", -1, false).unwrap(), r"C:\v1\FILE0000");
        assert_eq!(near_file(r"C:\v1\FILE_A.MP4", 1, true), None);
        assert_eq!(near_file(r"C:\v\2016_0512_A.gpx", 1, true).unwrap(), r"C:\v\2016_0513_A.gpx");
    }

    #[test]
    fn finds_sidecars_in_folder_and_gps_subfolders() {
        let root = std::env::temp_dir().join(format!("rv-fm-sidecar-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let v = root.join("DCIM").join("100");
        fs::create_dir_all(v.join("GPS")).unwrap();
        fs::create_dir_all(root.join("DCIM").join("INFO")).unwrap();
        // one-minute segments with their logs written alongside
        let write = |p: std::path::PathBuf, minute: u64| {
            fs::write(&p, b"x").unwrap();
            let c = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_710_064_800 + 60 * minute);
            let mut t = fs::FileTimes::new().set_modified(c + std::time::Duration::from_secs(60));
            #[cfg(windows)]
            {
                use std::os::windows::fs::FileTimesExt;
                t = t.set_created(c);
            }
            fs::File::options().write(true).open(&p).unwrap().set_times(t).unwrap();
        };
        write(v.join("FILE0001.MOV"), 0);
        write(v.join("FILE0001.NMEA"), 0);
        write(v.join("FILE0002.MOV"), 1);
        write(v.join("FILE0003.MOV"), 2);
        write(v.join("GPS").join("gps0002.gpx"), 1);
        write(root.join("DCIM").join("INFO").join("FILE0003.log"), 2);
        let mut cache = DirCache::default();
        let s = |p: &std::path::Path| p.to_string_lossy().into_owned();
        let c1 = find_files_for_video(&s(&v.join("FILE0001.MOV")), &mut cache);
        assert_eq!(c1.len(), 1);
        assert!(c1[0].to_lowercase().ends_with("file0001.nmea"), "{c1:?}");
        let c2 = find_files_for_video(&s(&v.join("FILE0002.MOV")), &mut cache);
        assert!(c2.iter().any(|c| c.ends_with(r"GPS\gps0002.gpx")), "{c2:?}");
        let c3 = find_files_for_video(&s(&v.join("FILE0003.MOV")), &mut cache);
        assert!(c3.iter().any(|c| c.ends_with(r"INFO\FILE0003.log")), "{c3:?}");
        fs::remove_dir_all(&root).unwrap();
    }
}
