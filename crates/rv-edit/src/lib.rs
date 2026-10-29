//! Export of video fragments without re-encoding: the engine behind "Save selected fragment"
//! (TFormCropping, SF-CROPPING-025…034).
//!
//! [`export`] cuts one or more crop items out of their source files, applies each item's speed,
//! joins them into one MOV/MP4 and optionally embeds the combined GPS / G-sensor track as the
//! Registrator Viewer RVMI data track. Packets are copied as they are (H.264 / H.265 video,
//! AAC audio); sample descriptions, user data and composition offsets are carried over from
//! the first usable source. Media data is streamed sample by sample; only the sample tables
//! are held in memory.
//!
//! Behaviour follows the original (`TSourceGroup.Add`, `TSourceWriter.Save`, `TMOVWriter`,
//! `AppendGPS` / `FilterForSpeedUp`): start snapped back to a key frame, every track cut to
//! the same span and ended with the first audio track, speed-up by key frames only or by a
//! higher frame rate, audio decimated / repeated at speed ≠ 1, output `ftyp`, `moov`, `mdat`
//! sections interleaved by time, RVMI samples first. Deliberate differences:
//! - the meta track follows the frames actually written (the key-frame start and the item's
//!   output duration) instead of the unsnapped p1 / p2, so GPS stays in sync with the video;
//!   items that failed contribute nothing to it;
//! - composition offsets (`ctts`) are scaled with the speed (the original keeps them, which
//!   breaks B-frame order at speed ≠ 1);
//! - cancelling removes the partial output; writing over a source is refused;
//! - a source track is matched at most once (two identical video tracks map to two tracks),
//!   `next_track_ID` is the largest ID + 1, `mvhd` / `mdhd` / `tkhd` version 1 are patched at
//!   their own offsets;
//! - H.265 decoder configurations must match exactly to join (the original only knew H.264,
//!   where an equal PPS is enough); uncompressed PCM audio is left out (reported as a warning).

mod export;
mod group;
mod meta;
mod source;
mod writer;

pub use export::{
    CropItem, ExportError, ExportOptions, ExportReport, ItemProblem, ItemSpan, ProblemKind,
    Progress, Stage, export,
};
