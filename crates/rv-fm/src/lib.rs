//! File manager behind the main window's file list (spec `docs/spec/file-list.md`): finds video
//! files and GPS logs, previews them on a background thread, groups consecutive segments and
//! keeps the list sorted.
//!
//! Not implemented yet (needs a GPS log parser): reading sidecar / pool logs. The sidecar
//! finder and the log pool are in place; [`gps::DefaultGpsLoader`] returns `None` for logs, so
//! only GPS tracks embedded in videos are used. Plug a parser in via
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
