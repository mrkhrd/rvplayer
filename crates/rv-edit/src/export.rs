//! The export job: stages 1-6 of `TFormCropping.ButtonStartClick` (SF-CROPPING-024…034).

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{NaiveDateTime, TimeDelta};
use rv_core::{FileInfo, Track};

use crate::group::{AddError, Cut, Group, ItemCut};
use crate::meta::{Combined, export_window};
use crate::source::Source;
use crate::writer::{self, Output, WriteError};

/// One piece of the output: a range of one source file and how it is played.
#[derive(Debug, Clone, Default)]
pub struct CropItem {
    pub path: PathBuf,
    /// Start and end as fractions (0..1) of the file's container duration.
    pub p1: f64,
    pub p2: f64,
    /// 1 = normal, > 1 faster, < 1 slower (1/N). Values <= 0 or not finite count as 1.
    pub speed: f64,
    /// At speed > 1: keep all frames and raise the frame rate (`true`), or keep key frames
    /// only (`false`).
    pub speed_up_fps: bool,
    /// Wall-clock time of the file's start (`sysTime`). When `None`: the GPS track's start,
    /// else [`FileInfo`]'s start, else the file's modification time minus its duration.
    pub start: Option<NaiveDateTime>,
    /// GPS / G-sensor track of the file. When `None` and meta is exported, the file's own
    /// embedded track is loaded with [`FileInfo::load`].
    pub track: Option<Track>,
}

/// The check boxes of the form and the registry trims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    /// Copy the video tracks.
    pub video: bool,
    /// Copy the audio tracks.
    pub audio: bool,
    /// Embed the combined GPS / G-sensor track (RVMI data track).
    pub meta: bool,
    /// Registry `trimHeadMS` / `trimTailMS`: never start earlier / end later than this far
    /// from the start / end of a file's video.
    pub trim_head_ms: u32,
    pub trim_tail_ms: u32,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            video: true,
            audio: true,
            meta: true,
            trim_head_ms: 0,
            trim_tail_ms: 0,
        }
    }
}

/// Export stages, in order (the form's "current action" label).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// "Opening files": sources opened, cut and checked (per item).
    Opening,
    /// "Generating meta info (1)": GPS tracks loaded (per item).
    Meta1,
    /// "Generating meta info (2)": GPS tracks cut and joined (per item).
    Meta2,
    /// "Generating meta info (3)": the joined track serialised.
    Meta3,
    /// "Copying frames": the output file is written.
    Copying,
    /// "Writing headers and indexes": the file is complete.
    Finishing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub stage: Stage,
    /// Item being processed (for `Copying`: the item the current output time belongs to).
    pub item: Option<usize>,
    /// Output time reached, in seconds.
    pub output_seconds: f64,
    /// Fraction of the stage done (0..1); for `Copying` the fraction of bytes written.
    pub fraction: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    /// The file could not be opened or parsed.
    CannotOpen,
    /// A track of the file does not match the output (codec, frame size, decoder
    /// configuration), or the file has no video.
    DifferentFormat,
}

/// An item that was skipped (SF-CROPPING-025).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemProblem {
    pub item: usize,
    pub path: PathBuf,
    pub kind: ProblemKind,
    /// Underlying error, for logs.
    pub detail: Option<String>,
}

impl ItemProblem {
    /// The original's (untranslated) line: "Cannot open file <file>" / "Different format <file>".
    pub fn message(&self) -> String {
        let what = match self.kind {
            ProblemKind::CannotOpen => "Cannot open file",
            ProblemKind::DifferentFormat => "Different format",
        };
        format!("{what} {}", self.path.display())
    }
}

/// Where an item landed in the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemSpan {
    pub item: usize,
    /// Output time range in seconds.
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExportReport {
    /// Items skipped with "Cannot open file" / "Different format".
    pub problems: Vec<ItemProblem>,
    /// Items written, with their output time ranges.
    pub items: Vec<ItemSpan>,
    /// Earliest item start and latest item end (wall clock, from the items' `start` and
    /// p1 / p2) for the output's creation / modification times (SF-CROPPING-030).
    pub t_min: Option<NaiveDateTime>,
    pub t_max: Option<NaiveDateTime>,
    /// Output duration in seconds.
    pub duration: f64,
    /// Output size in bytes.
    pub bytes: u64,
    /// Non-fatal notes (e.g. an audio track that could not be copied).
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum ExportError {
    /// The progress callback returned `false`; the partial output was removed.
    Cancelled,
    /// No item could be added (all failed, or no track selected); no file was written.
    NothingToExport { problems: Vec<ItemProblem> },
    /// The output path is one of the source files.
    OutputIsInput(PathBuf),
    /// Writing failed; the partial output was removed.
    Io(io::Error),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Cancelled => write!(f, "cancelled"),
            ExportError::NothingToExport { problems } => {
                write!(f, "nothing to export")?;
                for p in problems {
                    write!(f, "\n{}", p.message())?;
                }
                Ok(())
            }
            ExportError::OutputIsInput(p) => {
                write!(f, "the output file is one of the sources: {}", p.display())
            }
            ExportError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExportError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ExportError {
    fn from(e: io::Error) -> Self {
        ExportError::Io(e)
    }
}

fn seconds(d: TimeDelta) -> f64 {
    d.num_milliseconds() as f64 / 1000.0
}

fn add_seconds(t: NaiveDateTime, s: f64) -> NaiveDateTime {
    TimeDelta::try_milliseconds((s * 1000.0).round() as i64)
        .and_then(|d| t.checked_add_signed(d))
        .unwrap_or(t)
}

/// Per-item data gathered along the way.
#[derive(Default)]
struct ItemState {
    duration_ms: Option<i64>,
    cut: Option<ItemCut>,
    info: Option<FileInfo>,
}

impl ItemState {
    fn info(&mut self, path: &Path) -> &FileInfo {
        self.info.get_or_insert_with(|| FileInfo::load(path))
    }
}

/// Wall-clock start of an item's file (see [`CropItem::start`]).
fn item_start(item: &CropItem, st: &mut ItemState) -> Option<NaiveDateTime> {
    if let Some(t) = item
        .start
        .or_else(|| item.track.as_ref().and_then(|t| t.start))
    {
        return Some(t);
    }
    if let Some(t) = st.info(&item.path).start {
        return Some(t);
    }
    let modified = std::fs::metadata(&item.path)
        .and_then(|m| m.modified())
        .ok()?;
    let local = chrono::DateTime::<chrono::Local>::from(modified).naive_local();
    let dur = st
        .duration_ms
        .unwrap_or_else(|| (st.info(&item.path).duration * 1000.0) as i64);
    Some(local - TimeDelta::try_milliseconds(dur).unwrap_or_default())
}

fn speed_of(item: &CropItem) -> f64 {
    if item.speed.is_finite() && item.speed > 0.0 {
        item.speed
    } else {
        1.0
    }
}

/// Exports `items` into one MOV/MP4 at `output` without re-encoding.
///
/// Stages are reported through `progress`; returning `false` from it cancels: the job stops at
/// the next check (every 128 samples while copying) and any partial output is removed.
/// `output` is created or overwritten (ask the user before). Items that cannot be used are
/// skipped and listed in [`ExportReport::problems`].
pub fn export(
    items: &[CropItem],
    opts: &ExportOptions,
    output: &Path,
    progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<ExportReport, ExportError> {
    if let Ok(out) = std::fs::canonicalize(output) {
        for it in items {
            if std::fs::canonicalize(&it.path).is_ok_and(|p| p == out) {
                return Err(ExportError::OutputIsInput(it.path.clone()));
            }
        }
    }
    let n = items.len().max(1) as f64;
    let mut report = ExportReport::default();
    let mut states: Vec<ItemState> = items.iter().map(|_| ItemState::default()).collect();

    // Stage 1: open, check and cut every item (SF-CROPPING-025).
    let mut group = Group::default();
    let mut referer: Option<Source> = None;
    for (i, item) in items.iter().enumerate() {
        let p = Progress {
            stage: Stage::Opening,
            item: Some(i),
            output_seconds: group.duration(),
            fraction: i as f64 / n,
        };
        if !progress(&p) {
            return Err(ExportError::Cancelled);
        }
        let problem = |kind, detail: Option<String>| ItemProblem {
            item: i,
            path: item.path.clone(),
            kind,
            detail,
        };
        let (mut src, mut r) = match Source::open(&item.path) {
            Ok(s) => s,
            Err(e) => {
                report
                    .problems
                    .push(problem(ProblemKind::CannotOpen, Some(e.to_string())));
                continue;
            }
        };
        states[i].duration_ms = Some(src.duration_ms);
        let cut = Cut {
            p1: item.p1,
            p2: item.p2,
            speed: speed_of(item),
            speed_up_fps: item.speed_up_fps,
        };
        match group.add(i as u32, &mut src, &mut r, cut, opts) {
            Ok(Ok(c)) => {
                states[i].cut = Some(c);
                if referer.is_none() && group.referer == Some(i as u32) {
                    src.tracks.clear();
                    referer = Some(src);
                }
            }
            Ok(Err(AddError::DifferentFormat)) => {
                report
                    .problems
                    .push(problem(ProblemKind::DifferentFormat, None));
            }
            Err(e) => {
                report
                    .problems
                    .push(problem(ProblemKind::CannotOpen, Some(e.to_string())));
            }
        }
    }
    let Some(referer) = referer.filter(|_| group.tracks.iter().any(|t| !t.frames.is_empty()))
    else {
        return Err(ExportError::NothingToExport {
            problems: report.problems,
        });
    };
    if group.skipped_pcm {
        report
            .warnings
            .push("uncompressed (PCM) audio is not supported and was left out".to_string());
    }

    // tMin / tMax from the items' p1 / p2 (a skipped item counts with p2 = p1) (SF-CROPPING-027).
    for (i, item) in items.iter().enumerate() {
        let Some(start) = item_start(item, &mut states[i]) else {
            continue;
        };
        let dur = states[i].duration_ms.unwrap_or(0) as f64 / 1000.0;
        let p2 = if states[i].cut.is_some() {
            item.p2
        } else {
            item.p1
        };
        let (t1, t2) = (
            add_seconds(start, item.p1 * dur),
            add_seconds(start, p2 * dur),
        );
        report.t_min = Some(report.t_min.map_or(t1, |t| t.min(t1)));
        report.t_max = Some(report.t_max.map_or(t2, |t| t.max(t2)));
    }

    // Stages 2-4: the combined GPS / G-sensor track (SF-CROPPING-026…028, 033, 034).
    let records = if opts.meta {
        Some(meta_records(items, &mut states, progress)?)
    } else {
        None
    };

    // Stage 5: copy (SF-CROPPING-029).
    report.items = states
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            s.cut.map(|c| ItemSpan {
                item: i,
                start: c.out_start,
                end: c.out_end,
            })
        })
        .collect();
    report.duration = group.duration();
    let files: Vec<PathBuf> = items.iter().map(|i| i.path.clone()).collect();
    let spans = report.items.clone();
    let item_at = |t: f64| {
        spans
            .iter()
            .find(|s| t < s.end)
            .or(spans.last())
            .map(|s| s.item)
    };
    let p = Progress {
        stage: Stage::Copying,
        item: item_at(0.0),
        output_seconds: 0.0,
        fraction: 0.0,
    };
    if !progress(&p) {
        return Err(ExportError::Cancelled);
    }
    let out = Output {
        tracks: &group.tracks,
        ftyp: referer.ftyp.as_deref(),
        moov: &referer.moov,
        mp4: referer
            .path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mp4")),
        records: records.as_deref(),
        files: &files,
    };
    let written = writer::write(&out, output, &mut |t, fraction| {
        progress(&Progress {
            stage: Stage::Copying,
            item: item_at(t),
            output_seconds: t,
            fraction,
        })
    });
    report.bytes = match written {
        Ok(b) => b,
        Err(e) => {
            let _ = std::fs::remove_file(output);
            return Err(match e {
                WriteError::Cancelled => ExportError::Cancelled,
                WriteError::Io(e) => ExportError::Io(e),
            });
        }
    };

    // Stage 6: done; cancelling here still removes the output, like the original.
    let p = Progress {
        stage: Stage::Finishing,
        item: None,
        output_seconds: report.duration,
        fraction: 1.0,
    };
    if !progress(&p) {
        let _ = std::fs::remove_file(output);
        return Err(ExportError::Cancelled);
    }
    Ok(report)
}

/// Stages 2-4: loads the items' tracks, joins the pieces actually written and serialises them.
fn meta_records(
    items: &[CropItem],
    states: &mut [ItemState],
    progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<Vec<(i64, Vec<u8>)>, ExportError> {
    let n = items.len().max(1) as f64;
    for (i, item) in items.iter().enumerate() {
        let p = Progress {
            stage: Stage::Meta1,
            item: Some(i),
            output_seconds: 0.0,
            fraction: i as f64 / n,
        };
        if !progress(&p) {
            return Err(ExportError::Cancelled);
        }
        if states[i].cut.is_some() && item.track.is_none() {
            states[i].info(&item.path);
        }
    }

    let mut combined = Combined::default();
    let mut epoch: Option<NaiveDateTime> = None;
    for (i, item) in items.iter().enumerate() {
        let p = Progress {
            stage: Stage::Meta2,
            item: Some(i),
            output_seconds: 0.0,
            fraction: i as f64 / n,
        };
        if !progress(&p) {
            return Err(ExportError::Cancelled);
        }
        let Some(cut) = states[i].cut else { continue };
        let Some(start) = item_start(item, &mut states[i]) else {
            continue;
        };
        let epoch = *epoch.get_or_insert(start);
        let speed = speed_of(item);
        // The window of the source actually written: from the item's first frame, for the
        // item's output duration at its speed. Wall clock = file start + source time.
        let item_rel = seconds(start - epoch);
        let t1 = item_rel + cut.src_start;
        let t2 = t1 + (cut.out_end - cut.out_start) * speed;
        let track = item
            .track
            .as_ref()
            .or_else(|| states[i].info.as_ref().map(|f| &f.track));
        let points = match track {
            Some(t) => {
                let base = t.start.map_or(item_rel, |s| seconds(s - epoch));
                export_window(t, base, t1, t2)
            }
            None => Vec::new(),
        };
        combined.append(t1, t2, &points, speed);
    }
    combined.thin();
    let p = Progress {
        stage: Stage::Meta3,
        item: None,
        output_seconds: 0.0,
        fraction: 0.0,
    };
    if !progress(&p) {
        return Err(ExportError::Cancelled);
    }
    Ok(match epoch {
        Some(e) => combined.records(e),
        None => Vec::new(),
    })
}
