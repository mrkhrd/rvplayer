//! Dashcam file model: MP4/MOV structure, GPS/G-sensor tracks and per-file summary info.

pub mod fileinfo;
pub mod mp4;
pub mod rvmi;
pub mod track;

pub use fileinfo::FileInfo;
pub use track::{GSensorSample, GpsPoint, Track};
