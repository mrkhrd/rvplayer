//! Per-file summary shown in the file list (the columns of the original Registrator Viewer).

use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveDateTime, TimeDelta};

use crate::mp4::{self, Movie};
use crate::rvmi;
use crate::track::Track;

pub const VIDEO_EXTENSIONS: [&str; 2] = ["mov", "mp4"];

#[derive(Debug, Clone, Default)]
pub struct FileInfo {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub duration: f64,
    /// Recording start: from the GPS track if available, else the container creation time.
    pub start: Option<NaiveDateTime>,
    pub width: u32,
    pub height: u32,
    pub camera: Option<String>,
    pub track: Track,
    /// Why the file could not be read, if it could not.
    pub error: Option<String>,
}

impl FileInfo {
    /// Reads the container structure and any supported GPS track. Never fails: problems are
    /// reported in `error` so the file still appears in the list.
    pub fn load(path: &Path) -> FileInfo {
        Self::load_with_movie(path).0
    }

    /// Like [`FileInfo::load`], also returning the container structure (tracks and sample
    /// tables) when it could be read, for callers that need more than the summary.
    pub fn load_with_movie(path: &Path) -> (FileInfo, Option<Movie>) {
        let mut info = FileInfo {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            ..Default::default()
        };
        match info.read(path) {
            Ok(movie) => (info, Some(movie)),
            Err(e) => {
                info.error = Some(e.to_string());
                (info, None)
            }
        }
    }

    fn read(&mut self, path: &Path) -> io::Result<Movie> {
        let file = File::open(path)?;
        self.size = file.metadata()?.len();
        let mut r = BufReader::new(file);
        let movie = mp4::read_movie(&mut r)?;
        self.duration = movie.duration_secs();
        self.camera = movie.camera_model();
        if let Some(v) = movie.track_by_handler(b"vide") {
            self.width = v.width.round() as u32;
            self.height = v.height.round() as u32;
        }
        if let Some(track) = rvmi::read_track(&mut r, &movie)? {
            self.track = track;
        }
        self.start = self.track.start.or_else(|| creation_time(&movie));
        Ok(movie)
    }

    pub fn distance_km(&self) -> f64 {
        self.track.distance_km()
    }

    pub fn avg_speed_kmh(&self) -> f64 {
        self.track.avg_speed_kmh()
    }

    pub fn resolution(&self) -> Option<String> {
        (self.width > 0).then(|| format!("{}x{}", self.width, self.height))
    }

    pub fn folder(&self) -> String {
        self.path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }
}

/// Video files in `dir` (not recursive), sorted by name.
pub fn list_videos(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| VIDEO_EXTENSIONS.iter().any(|v| e.eq_ignore_ascii_case(v)))
        })
        .collect();
    out.sort();
    Ok(out)
}

fn creation_time(movie: &Movie) -> Option<NaiveDateTime> {
    if movie.creation_time == 0 {
        return None;
    }
    let epoch = NaiveDate::from_ymd_opt(1904, 1, 1)?.and_hms_opt(0, 0, 0)?;
    epoch.checked_add_signed(TimeDelta::try_seconds(
        i64::try_from(movie.creation_time).ok()?,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads real sample files when `RV_TESTDATA` points to a folder of them.
    #[test]
    fn sample_folder_if_available() {
        let Ok(dir) = std::env::var("RV_TESTDATA") else {
            return;
        };
        for path in list_videos(Path::new(&dir)).unwrap() {
            let info = FileInfo::load(&path);
            assert!(info.error.is_none(), "{}: {:?}", info.name, info.error);
            assert!(info.duration > 0.0, "{}", info.name);
            println!(
                "{} | {:.1}s | {:?} | {:.2} km | {:.1} km/h | {:?} | {:?} | gps {} gs {}",
                info.name,
                info.duration,
                info.start,
                info.distance_km(),
                info.avg_speed_kmh(),
                info.resolution(),
                info.camera,
                info.track.gps.len(),
                info.track.gsensor.len()
            );
        }
    }
}
