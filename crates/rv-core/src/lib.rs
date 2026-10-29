//! Dashcam file model: MP4/MOV structure, GPS/G-sensor tracks (embedded and log formats, see
//! [`gps`]) and per-file summary info.

pub mod fileinfo;
pub mod gps;
pub mod mp4;
pub mod rvmi;
#[cfg(any(test, feature = "testutil"))]
pub mod testutil;
pub mod track;

pub use fileinfo::FileInfo;
pub use track::{GSensorSample, GpsPoint, Track};
