//! File manager behind the main window's file list (spec `docs/spec/file-list.md`): finds video
//! files and GPS logs, previews them on a background thread, groups consecutive segments and
//! keeps the list sorted.
//!
//! GPS data comes from [`gps::DefaultGpsLoader`]: tracks embedded in videos and sidecar / pool
//! logs in the original's formats (`rv_core::gps`). Another loader can be plugged in via
//! [`FileManager::with_loader`].

pub mod gps;
pub mod grouping;
pub mod manager;
pub mod model;
mod platform;
pub mod sidecar;
pub mod time;

pub use gps::{DefaultGpsLoader, GpsLoader};
pub use manager::{FileManager, Row, Rows};
pub use model::{FileEntry, GpsInfo, Item, TimeBlock, VideoFile};
