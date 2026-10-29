//! "Fast" repair of broken dashcam MOV/MP4 files (Repair dialog, SF-REPAIR-009…017).
//!
//! A camera writes `ftyp`, then the media data (`mdat`), and the header (`moov`) only when the
//! recording is closed; after a power cut the file has no header and does not play. The Fast method
//! rebuilds the header from a good file of the same camera (the *reference*) and writes it into the
//! broken file **in place**:
//!
//! 1. Read the reference's `moov`: track layout, codec configuration, timescales and per-sample
//!    timing (the original's `ReadRef`).
//! 2. Find the broken file's `mdat` (`SeekToMDAT`).
//! 3. Walk the media data for video frames — 4-byte length-prefixed H.264 / H.265 NAL units — and
//!    take the bytes between two frames as audio, split into frames where possible (`ScanFrames`,
//!    `IsVideoFrameStart`, `SplitLastAudio`).
//! 4. Build a new `moov` from the reference's, with sample tables for the found samples (`CreateTrack`,
//!    `TMOVWriter.CopyHeader`), and write it into the space reserved before `mdat` or after the
//!    media data, fixing the `mdat` size (`WriteHeader`).
//!
//! Port of `TFileRecoverHeader` / `TFileRecoverHeaderThread` (UnitFileRecoverHeader) and the parts
//! of UnitSourceMOV it uses. Unlike the original, the file is only written in the last step, after
//! everything has been read and the new header built; a cancelled or failed repair leaves it
//! untouched, and a failed write is backed out.

mod boxes;
mod encrypted;
mod moov;
mod nal;
mod reference;
mod scan;
mod write;

use std::fmt;
use std::fs::{self, File, FileTimes, Metadata, OpenOptions};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

pub use encrypted::is_encrypted;

use reference::Reference;
use scan::{Progress, Scanner};

/// Size granule of the crash heuristic (SF-REPAIR-010): 32 MiB.
pub const SUSPICIOUS_SIZE_GRANULE: u64 = 32 << 20;

/// SF-REPAIR-010: a broken file whose size is an exact (non-zero) multiple of 32 MiB was probably
/// cut by a crash while the file system had only allocated space for it; the dialog suggests
/// `chkdsk <drive>: /f` before repairing.
pub fn is_suspicious_size(len: u64) -> bool {
    len != 0 && len.is_multiple_of(SUSPICIOUS_SIZE_GRANULE)
}

/// Why a repair failed. The broken file is unchanged in every case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The reference file cannot be used: unreadable, not parseable, no `moov`, no video track.
    /// (The original showed "Reference file - Error".)
    Reference(String),
    /// Reading or writing the broken file failed.
    Io(String),
    /// The file cannot be repaired in place: unsupported codec or NAL layout, or media data too
    /// large for its header.
    Unsupported(String),
}

impl Error {
    pub(crate) fn io(e: io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(m) => write!(f, "reference file: {m}"),
            Self::Io(m) => f.write_str(m),
            Self::Unsupported(m) => write!(f, "cannot repair in place: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// Outcome of a finished repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    /// Video frames in the new header; 0 when fewer than two were found and nothing was written.
    pub video_frames: u64,
    /// Audio frames found (an audio sample that could not be split counts as several frames).
    pub audio_frames: u64,
    /// Where the new `moov` was written; `None` when nothing was written.
    pub header_offset: Option<u64>,
}

impl Summary {
    /// Frames as the dialog reports them: video + audio (SF-REPAIR-014/015; < 1 means failure).
    pub fn frames(&self) -> u64 {
        self.video_frames + self.audio_frames
    }
}

/// Snapshot of a running or finished repair (the original's `GetStatus`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Status {
    /// Scan position as a percentage of the file size; 100 once finished successfully.
    pub percent: u8,
    /// Frames found so far (video + audio); when finished, the frames written into the file
    /// (0 after a failure, a cancellation or when fewer than two video frames were found).
    pub frames: u64,
    /// The background work has ended (the original's "not running").
    pub finished: bool,
    /// Finished because of [`Repair::cancel`]; the file is unchanged.
    pub cancelled: bool,
    /// Finished with an error; the file is unchanged.
    pub error: Option<Error>,
}

#[derive(Debug, Clone)]
enum Done {
    Repaired(Summary),
    Cancelled,
    Failed(Error),
}

#[derive(Default)]
struct Shared {
    cancel: AtomicBool,
    percent: AtomicU8,
    frames: AtomicU64,
    len: AtomicU64,
    done: Mutex<Option<Done>>,
}

impl Progress for Shared {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn update(&self, pos: u64, frames: u64) {
        let len = self.len.load(Ordering::Relaxed);
        let percent = if len == 0 {
            0
        } else {
            (u128::from(pos) * 100 / u128::from(len)).min(100) as u8
        };
        self.percent.store(percent, Ordering::Relaxed);
        self.frames.store(frames, Ordering::Relaxed);
    }
}

/// A Fast repair running on a background thread (the original's `TFileRecoverHeader`).
///
/// Dropping it cancels and waits for the thread, like freeing the original's recoverer; once the
/// new header is being written, cancellation is no longer honoured and the drop waits for the
/// write to complete.
pub struct Repair {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Repair {
    /// Starts repairing `broken` in place using `reference`'s header. Errors only if the thread
    /// cannot be started; everything else is reported through [`Repair::status`].
    pub fn start(broken: PathBuf, reference: PathBuf) -> io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let worker = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("rv-repair".into())
            .spawn(move || {
                let done =
                    match catch_unwind(AssertUnwindSafe(|| run(&broken, &reference, &worker))) {
                        Ok(Ok(Some(summary))) => Done::Repaired(summary),
                        Ok(Ok(None)) => Done::Cancelled,
                        Ok(Err(e)) => Done::Failed(e),
                        Err(_) => Done::Failed(Error::Io("internal error".into())),
                    };
                // The file was rewritten in place, its old times put back.
                rv_core::infocache::forget(&broken);
                *worker.done.lock().unwrap_or_else(|e| e.into_inner()) = Some(done);
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub fn status(&self) -> Status {
        let s = &self.shared;
        let percent = s.percent.load(Ordering::Relaxed);
        let done = s.done.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match done {
            None => Status {
                percent,
                frames: s.frames.load(Ordering::Relaxed),
                ..Status::default()
            },
            Some(Done::Repaired(summary)) => Status {
                percent: 100,
                frames: summary.frames(),
                finished: true,
                ..Status::default()
            },
            Some(Done::Cancelled) => Status {
                percent,
                finished: true,
                cancelled: true,
                ..Status::default()
            },
            Some(Done::Failed(e)) => Status {
                percent,
                finished: true,
                error: Some(e),
                ..Status::default()
            },
        }
    }

    /// Asks the scan to stop; the file is left unchanged. No effect once writing has begun.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    /// Waits for the repair to finish.
    pub fn wait(mut self) -> Status {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.status()
    }
}

impl Drop for Repair {
    fn drop(&mut self) {
        if let Some(t) = self.thread.take() {
            self.cancel();
            let _ = t.join();
        }
    }
}

/// Runs a Fast repair on the calling thread.
pub fn repair(broken: &Path, reference: &Path) -> Result<Summary, Error> {
    Ok(run(broken, reference, &Shared::default())?.unwrap_or_default())
}

/// The original's `TFileRecoverHeaderThread.Execute`. `Ok(None)` when cancelled.
fn run(broken: &Path, reference: &Path, shared: &Shared) -> Result<Option<Summary>, Error> {
    let reference = Reference::load(reference)?;
    let meta = fs::metadata(broken).map_err(Error::io)?;
    let len = meta.len();
    shared.len.store(len, Ordering::Relaxed);

    let mut file = File::open(broken).map_err(Error::io)?;
    let mdat = scan::find_mdat(&mut file, len).map_err(Error::io)?;
    let mut scanner = Scanner::new(file, len, &reference.style, reference.avg_audio_size);
    if scanner
        .scan(mdat.payload(), shared)
        .map_err(Error::io)?
        .is_err()
    {
        return Ok(None);
    }
    let mut found = std::mem::take(&mut scanner.found);
    let mut file = scanner.into_inner();
    if shared.cancelled() {
        return Ok(None);
    }
    let Some(plan) = write::plan(&reference, &mdat, &mut found, &mut file)? else {
        return Ok(Some(Summary::default()));
    };
    drop(file);

    // From here on the repair is no longer cancelled: the header is written in one go.
    write_in_place(broken, &meta, &plan)?;
    Ok(Some(Summary {
        video_frames: found.video.len() as u64,
        audio_frames: found.audio_frames,
        header_offset: Some(plan.moov_pos),
    }))
}

/// Applies the plan. Like the original, clears the read-only attribute first (`UnlockReadOnly`)
/// and restores the file's creation and modification times afterwards
/// (`ReadFileTime` / `WriteFileTime`).
fn write_in_place(path: &Path, meta: &Metadata, plan: &write::Plan) -> Result<(), Error> {
    let was_readonly = unlock(path).map_err(Error::io)?;
    let result = (|| {
        let mut f = OpenOptions::new().read(true).write(true).open(path)?;
        if f.metadata()?.len() != meta.len() {
            return Err(io::Error::other(
                "the file changed while it was being repaired",
            ));
        }
        let applied = write::apply(&mut f, meta.len(), plan);
        restore_times(&f, meta);
        applied
    })();
    if result.is_err() && was_readonly {
        let mut p = meta.permissions();
        p.set_readonly(true);
        let _ = fs::set_permissions(path, p);
    }
    result.map_err(Error::io)
}

/// Makes the file writable; returns whether it was read-only.
fn unlock(path: &Path) -> io::Result<bool> {
    let mut p = fs::metadata(path)?.permissions();
    if !p.readonly() {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.set_mode(p.mode() | 0o200);
    }
    #[cfg(not(unix))]
    #[allow(clippy::permissions_set_readonly_false)]
    p.set_readonly(false);
    fs::set_permissions(path, p)?;
    Ok(true)
}

fn restore_times(f: &File, meta: &Metadata) {
    let mut times = FileTimes::new();
    if let Ok(t) = meta.modified() {
        times = times.set_modified(t);
    }
    #[cfg(windows)]
    if let Ok(t) = meta.created() {
        use std::os::windows::fs::FileTimesExt;
        times = times.set_created(t);
    }
    let _ = f.set_times(times);
}

#[cfg(test)]
mod tests;
