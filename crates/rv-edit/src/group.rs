//! The source group: output tracks assembled from cut, sped-up pieces of the sources
//! (a port of `TSourceGroup.Add`, UnitSources L382-L671).

use std::io::{self, Read, Seek, SeekFrom};

use crate::ExportOptions;
use crate::source::{Frame, Kind, Source, SrcTrack, round, same_dsi};

/// One sample of an output track: where its bytes are and when it plays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OutFrame {
    /// Index of the source file (the crop item) holding the bytes.
    pub file: u32,
    pub offset: u64,
    pub size: u32,
    /// Decode time in track ticks.
    pub time: f64,
    pub dur: f64,
    pub ctts: i32,
    pub key: bool,
    pub sdtp: u8,
}

#[derive(Debug, Clone)]
pub(crate) struct GroupTrack {
    pub kind: Kind,
    pub fourcc: [u8; 4],
    pub format: [u32; 3],
    pub dsi: Vec<u8>,
    /// Track ID in the first source; the output keeps it.
    pub id: u32,
    pub scale: u32,
    pub frames: Vec<OutFrame>,
    /// Ticks to the end of the last frame (`durationUnscale`).
    pub duration_unscale: i64,
    pub duration_ms: i64,
}

impl GroupTrack {
    fn update_scale(&mut self, new_scale: u32) {
        let old = self.scale;
        if new_scale == old || old == 0 {
            return;
        }
        let t0 = self.frames.first().map_or(0.0, |f| f.time);
        for f in &mut self.frames {
            rescale(&mut f.time, &mut f.dur, &mut f.ctts, t0, old, new_scale);
        }
        self.duration_unscale = self.duration_unscale * i64::from(new_scale) / i64::from(old);
        self.scale = new_scale;
    }
}

/// `ISourceTrack.UpdateScale` for one frame: times stretched around the first frame's time,
/// composition offsets in whole ticks (truncated, as the original's integer division).
fn rescale(time: &mut f64, dur: &mut f64, ctts: &mut i32, t0: f64, old: u32, new: u32) {
    let k = f64::from(new) / f64::from(old);
    *dur *= k;
    *ctts = (i64::from(*ctts) * i64::from(new) / i64::from(old)) as i32;
    *time = (*time - t0) * k + t0;
}

/// What a successfully added item contributed, for the meta track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ItemCut {
    /// Source time (seconds) of the item's first frame of the reference track.
    pub src_start: f64,
    /// Output time span of the item (seconds).
    pub out_start: f64,
    pub out_end: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AddError {
    /// No matching track for an output track, or no video (result 2 of the original).
    DifferentFormat,
}

/// Parameters of one crop item (the arguments of `TSourceGroup.Add`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cut {
    pub p1: f64,
    pub p2: f64,
    pub speed: f64,
    pub speed_up_fps: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Group {
    pub tracks: Vec<GroupTrack>,
    /// File index of the first successfully added source: its `ftyp` / `moov` are the templates.
    pub referer: Option<u32>,
    /// Audio tracks of unsupported layout (PCM) skipped when the tracks were created.
    pub skipped_pcm: bool,
}

/// A source track prepared for cutting (possibly synthesized), in group ticks.
#[derive(Clone)]
struct CutTrack {
    file: u32,
    scale: u32,
    duration_ms: i64,
    frames: Vec<Frame>,
    kind: Kind,
}

impl CutTrack {
    fn from_source(file: u32, t: &SrcTrack) -> Self {
        CutTrack {
            file,
            scale: t.scale,
            duration_ms: t.duration_ms(),
            frames: t.frames.clone(),
            kind: t.kind,
        }
    }

    fn update_scale(&mut self, new_scale: u32) {
        let old = self.scale;
        if new_scale == old || old == 0 {
            return;
        }
        let t0 = self.frames.first().map_or(0.0, |f| f.time);
        for f in &mut self.frames {
            rescale(&mut f.time, &mut f.dur, &mut f.ctts, t0, old, new_scale);
        }
        self.scale = new_scale;
    }
}

/// `ISourceTrack.SeekFrameIndexByTime`: the last frame whose rounded decode time is at most
/// `ticks`, optionally moved back to the preceding key frame.
pub(crate) fn seek(frames: &[Frame], ticks: i64, back_to_key: bool) -> usize {
    let n = frames.len();
    let pos = ticks as f64;
    let mut result = if n < 2 || frames[0].time >= pos {
        0
    } else if pos >= frames[n - 1].time {
        n - 1
    } else {
        let (mut l, mut r) = (0usize, n - 1);
        loop {
            if r - l < 2 {
                break l;
            }
            let c = (l + r) / 2;
            let ct = round(frames[c].time);
            if ct < ticks {
                l = c;
            } else if ticks < ct {
                r = c;
            } else {
                break c;
            }
        }
    };
    if back_to_key {
        while result > 0 && !frames[result].key {
            result -= 1;
        }
    }
    result
}

/// `GetGOPSize`: index of the second key frame.
fn gop_size(frames: &[Frame]) -> usize {
    (1..frames.len()).find(|&i| frames[i].key).unwrap_or(0)
}

/// `GetGOPInfo`: picture types of the first GOP from H.264 access unit delimiters
/// ('I', 'P', 'B', '?' for another AUD, '.' without one).
fn gop_info<R: Read + Seek>(r: &mut R, frames: &[Frame]) -> io::Result<Vec<u8>> {
    let mut gop = Vec::new();
    let last = 1000.min(frames.len().saturating_sub(1));
    for f in frames.iter().take(last + 1).skip(1) {
        if f.key {
            break;
        }
        let mut b = [0u8; 6];
        let have = f.size >= 6 && {
            r.seek(SeekFrom::Start(f.offset))?;
            r.read_exact(&mut b)?;
            true
        };
        gop.push(if have && b[..5] == [0, 0, 0, 2, 9] {
            match b[5] {
                0x50 => b'B',
                0x30 => b'P',
                0x10 => b'I',
                _ => b'?',
            }
        } else {
            b'.'
        });
    }
    Ok(gop)
}

/// `DetectFrameTypes` for one video track.
fn detect_frame_types<R: Read + Seek>(r: &mut R, t: &mut SrcTrack) -> io::Result<()> {
    if t.frames.first().is_none_or(|f| f.ftype != 0) {
        return Ok(());
    }
    let gop = if t.is_h264() {
        gop_info(r, &t.frames)?
    } else {
        Vec::new()
    };
    let mut last_key = 0;
    for (i, f) in t.frames.iter_mut().enumerate() {
        f.ftype = if f.key {
            last_key = i;
            1
        } else {
            let q = i - last_key;
            match gop.get(q - 1) {
                Some(b'I') => 1,
                Some(b'B') => 3,
                _ => 2,
            }
        };
    }
    Ok(())
}

impl Group {
    /// Index of the track whose end positions the next item (`GetVideoTrack` of the group,
    /// else the first track).
    pub fn reference_track(&self) -> Option<usize> {
        self.tracks
            .iter()
            .position(|t| t.kind == Kind::Video)
            .or_else(|| (!self.tracks.is_empty()).then_some(0))
    }

    /// Output duration so far, in seconds (the end of the reference track, rounded to ms).
    pub fn duration(&self) -> f64 {
        self.reference_track()
            .map_or(0.0, |i| self.tracks[i].duration_ms as f64 / 1000.0)
    }

    /// Appends the `cut` piece of `src` (file index `file`). `r` reads the source's data
    /// (for the picture types of the key-frame-only speed-up).
    pub fn add<R: Read + Seek>(
        &mut self,
        file: u32,
        src: &mut Source,
        r: &mut R,
        cut: Cut,
        opts: &ExportOptions,
    ) -> io::Result<Result<ItemCut, AddError>> {
        // A source without video cannot be added (L472-L475); checked first so that a failed
        // first source does not define the output tracks.
        if src.video_track().is_none() && self.tracks.is_empty() {
            return Ok(Err(AddError::DifferentFormat));
        }
        let time_start = if self.tracks.is_empty() {
            for t in &src.tracks {
                let wanted = match t.kind {
                    Kind::Video => opts.video,
                    Kind::Audio if t.is_pcm() => {
                        self.skipped_pcm |= opts.audio;
                        false
                    }
                    Kind::Audio => opts.audio,
                    Kind::Other => false,
                };
                if wanted {
                    self.tracks.push(GroupTrack {
                        kind: t.kind,
                        fourcc: t.fourcc,
                        format: t.format,
                        dsi: t.dsi.clone(),
                        id: t.id,
                        scale: t.scale,
                        frames: Vec::new(),
                        duration_unscale: 0,
                        duration_ms: 0,
                    });
                }
            }
            0.0
        } else {
            self.duration()
        };

        // Match every output track with a source track (L424-L451).
        let speed = cut.speed;
        let key_frames_only = speed > 1.0 && !cut.speed_up_fps;
        let mut used = vec![false; src.tracks.len()];
        let mut matched: Vec<Option<usize>> = Vec::with_capacity(self.tracks.len());
        let mut cut_tracks: Vec<CutTrack> = Vec::with_capacity(self.tracks.len());
        for (i, g) in self.tracks.iter().enumerate() {
            let found = src.tracks.iter().enumerate().position(|(z, t)| {
                !used[z]
                    && t.kind == g.kind
                    && t.fourcc == g.fourcc
                    && t.format == g.format
                    && same_dsi(t, &g.dsi, &t.dsi)
            });
            match found {
                Some(z) => {
                    used[z] = true;
                    if key_frames_only && src.tracks[z].kind == Kind::Video {
                        detect_frame_types(r, &mut src.tracks[z])?;
                    }
                    matched.push(Some(z));
                    cut_tracks.push(CutTrack::from_source(file, &src.tracks[z]));
                }
                // A missing audio track (not the first output track) is filled by repeating
                // the output's next-to-last audio frame for the duration of the source (L440-L444).
                None if i > 0 && g.kind == Kind::Audio => {
                    let Some(&pf) = g.frames.get(g.frames.len().saturating_sub(2)) else {
                        return Ok(Err(AddError::DifferentFormat));
                    };
                    let dt_ms = cut_tracks[0].duration_ms;
                    let count = if pf.dur > 0.0 {
                        (f64::from(g.scale) * (dt_ms as f64 / 1000.0) / pf.dur).ceil() as usize
                    } else {
                        0
                    };
                    let frames = (0..count)
                        .map(|k| Frame {
                            offset: pf.offset,
                            size: pf.size,
                            time: k as f64 * pf.dur,
                            dur: pf.dur,
                            ctts: pf.ctts,
                            key: pf.key,
                            sdtp: pf.sdtp,
                            ftype: 1,
                        })
                        .collect();
                    matched.push(None);
                    cut_tracks.push(CutTrack {
                        file: pf.file,
                        scale: g.scale,
                        duration_ms: round(count as f64 * pf.dur * 1000.0 / f64::from(g.scale)),
                        frames,
                        kind: Kind::Audio,
                    });
                }
                None => return Ok(Err(AddError::DifferentFormat)),
            }
        }
        // Source tracks take the time scale of their output tracks (L462-L466).
        for (g, t) in self.tracks.iter().zip(cut_tracks.iter_mut()) {
            t.update_scale(g.scale);
        }

        // The track that positions the start: the source's first video track (rescaled if it
        // is copied), else the source track of the first output track (L471-L475).
        let video_track = match src.video_track() {
            Some(v) => match matched.iter().position(|&m| m == Some(v)) {
                Some(i) => cut_tracks[i].clone(),
                None => CutTrack::from_source(file, &src.tracks[v]),
            },
            None => match cut_tracks.first() {
                Some(t) => t.clone(),
                None => return Ok(Err(AddError::DifferentFormat)),
            },
        };

        // Start and end in seconds of the source (L469-L494).
        let dur_s = src.duration_ms as f64 / 1000.0;
        let mut p1 = cut.p1 * dur_s;
        let mut p2 = cut.p2 * dur_s;
        if !video_track.frames.is_empty() && video_track.scale > 0 {
            let scale = f64::from(video_track.scale);
            let k = seek(&video_track.frames, round(scale * p1), true);
            p1 = round(video_track.frames[k].time) as f64 / scale;
        }
        p1 = p1.max(f64::from(opts.trim_head_ms) / 1000.0);
        p2 = p2.min((video_track.duration_ms - i64::from(opts.trim_tail_ms)) as f64 / 1000.0);

        // Frame ranges per track and the common span (L498-L520).
        let n_tracks = self.tracks.len();
        let mut i1 = vec![0usize; n_tracks];
        let mut i2 = vec![0usize; n_tracks];
        let mut span = vec![0.0f64; n_tracks];
        let mut dt = -1.0f64;
        for (i, t) in cut_tracks.iter().enumerate() {
            if t.frames.is_empty() || t.scale == 0 {
                continue;
            }
            let scale = f64::from(t.scale);
            i1[i] = seek(&t.frames, round(scale * p1) + 1, true);
            i2[i] = seek(&t.frames, round(scale * p2) + 1, false);
            let (a, b) = (&t.frames[i1[i]], &t.frames[i2[i]]);
            span[i] = (b.dur + b.time - a.time) / scale;
            if dt < 0.0 || span[i] < dt {
                dt = span[i];
            }
        }
        for (i, t) in cut_tracks.iter().enumerate() {
            if t.frames.is_empty() || t.scale == 0 {
                continue;
            }
            while dt < span[i] && i1[i] < i2[i] {
                let new_dur = (t.frames[i2[i]].time - t.frames[i1[i]].time) / f64::from(t.scale);
                if new_dur < dt {
                    break;
                }
                span[i] = new_dur;
                i2[i] -= 1;
            }
        }
        // Every track ends with the first audio track (L523-L531).
        let q = cut_tracks
            .iter()
            .position(|t| t.kind == Kind::Audio)
            .unwrap_or(0);
        let adj: Vec<f64> = (0..n_tracks).map(|i| span[q] - span[i]).collect();

        let speed_k = if speed < 1.0 {
            round(1.0 / speed)
        } else {
            round(speed)
        }
        .max(1) as usize;

        let mut ref_src_start = None;
        let ref_track = self.reference_track();
        for (i, (g, t)) in self.tracks.iter_mut().zip(&cut_tracks).enumerate() {
            if t.frames.is_empty() || i2[i] < i1[i] {
                continue;
            }
            let n = g.frames.len();
            let (a, b) = (i1[i], i2[i]);
            let file = t.file;
            let out = |f: &Frame, time: f64, dur: f64, ctts: i32| OutFrame {
                file,
                offset: f.offset,
                size: f.size,
                time,
                dur,
                ctts,
                key: f.key,
                sdtp: f.sdtp,
            };
            // ctts follows the time scale of the speed change (the original leaves it as is,
            // which scrambles B-frame order at speed != 1).
            let ctts = |f: &Frame| {
                if speed != 1.0 {
                    round(f64::from(f.ctts) / speed) as i32
                } else {
                    f.ctts
                }
            };
            if g.kind == Kind::Video {
                if !key_frames_only {
                    // All frames, time and duration scaled (L601-L610).
                    for f in &t.frames[a..=b] {
                        let (time, dur) = if speed != 1.0 {
                            (f.time / speed, f.dur / speed)
                        } else {
                            (f.time, f.dur)
                        };
                        g.frames.push(out(f, time, dur, ctts(f)));
                    }
                } else {
                    // Key frames (and droppable-safe P frames) only (L561-L598).
                    let mut skip = speed_k;
                    let mut skipped_p = false;
                    let mut last_key_z = a;
                    let mut last_key_n = n;
                    let mut prev_dkey = gop_size(&t.frames);
                    for z in a..=b {
                        let fr = &t.frames[z];
                        let n2 = g.frames.len();
                        let mut keep = n2 == n
                            || (skip >= speed_k && fr.ftype == 1)
                            || (skip >= speed_k && !skipped_p && fr.ftype == 2);
                        if !keep && fr.key && n2 - last_key_n > 0x41 {
                            keep = true;
                        }
                        if !keep && fr.key && last_key_z + prev_dkey != z {
                            keep = true;
                        }
                        if keep {
                            if fr.key {
                                last_key_n = n2;
                            }
                            g.frames
                                .push(out(fr, fr.time / speed, fr.dur / speed, ctts(fr)));
                            skip = 1;
                            skipped_p = false;
                        } else {
                            skip += 1;
                            if fr.ftype == 2 {
                                skipped_p = true;
                            }
                            g.frames[n2 - 1].dur += fr.dur / speed;
                        }
                        if fr.key {
                            if last_key_z < z {
                                prev_dkey = z - last_key_z;
                            }
                            last_key_z = z;
                        }
                    }
                }
                // Whole-tick durations, the fraction carried to the next frame (L613-L622).
                if speed > 1.0 {
                    let n2 = g.frames.len();
                    for z in n..n2.saturating_sub(1) {
                        let d = g.frames[z].dur;
                        let fra = d.fract();
                        if fra > 0.0 {
                            g.frames[z].dur = d.trunc();
                            g.frames[z + 1].dur += fra;
                        }
                    }
                }
            } else {
                // Audio: frames picked at `speed` steps, decimated or repeated (L625-L637).
                let count = ((b - a) as f64 / speed).trunc() as usize + 1;
                let rept = round(1.0 / speed).max(1) as usize;
                for k in 0..count {
                    let qi = (a + (k as f64 * speed).trunc() as usize).min(b);
                    let f = &t.frames[qi];
                    let mut time = if speed != 1.0 { f.time / speed } else { f.time };
                    if speed < 1.0 {
                        time += (k % rept) as f64 * f.dur;
                    }
                    g.frames.push(out(f, time, f.dur, f.ctts));
                }
            }
            let scale = f64::from(g.scale);
            // The last frame absorbs the difference to the audio span (L639).
            if let Some(last) = g.frames.last_mut() {
                last.dur += scale * adj[i];
            }
            // Times relative to the item start in the output (L643-L648).
            let d = scale * time_start - t.frames[a].time / speed;
            for f in &mut g.frames[n..] {
                f.time += d;
            }
            if Some(i) == ref_track {
                ref_src_start = Some(t.frames[a].time / scale);
            }
            let last = g.frames[g.frames.len() - 1];
            g.duration_unscale = round(last.time + last.dur);
            g.duration_ms = round(g.duration_unscale as f64 * 1000.0 / scale);
        }

        // Keep durations below 2^31 ticks (L664-L671).
        for g in &mut self.tracks {
            let mut l = 1i64;
            while g.duration_unscale >= (1i64 << 31) * l {
                l *= 2;
            }
            if l > 1 {
                g.update_scale((i64::from(g.scale) / l) as u32);
            }
        }
        if self.referer.is_none() {
            self.referer = Some(file);
        }
        Ok(Ok(ItemCut {
            src_start: ref_src_start.unwrap_or(p1),
            out_start: time_start,
            out_end: self.duration(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(times: &[f64], keys: &[usize]) -> Vec<Frame> {
        times
            .iter()
            .enumerate()
            .map(|(i, &t)| Frame {
                offset: i as u64,
                size: 1,
                time: t,
                dur: 10.0,
                ctts: 0,
                key: keys.contains(&i),
                sdtp: 0,
                ftype: 0,
            })
            .collect()
    }

    #[test]
    fn seek_finds_last_frame_at_or_before() {
        let f = frames(&[0.0, 10.0, 20.0, 30.0, 40.0, 50.0], &[0, 3]);
        assert_eq!(seek(&f, 25, false), 2);
        assert_eq!(seek(&f, 30, false), 3);
        assert_eq!(seek(&f, 31, false), 3);
        assert_eq!(seek(&f, 0, false), 0);
        assert_eq!(seek(&f, 99, false), 5);
        assert_eq!(seek(&f, 25, true), 0);
        assert_eq!(seek(&f, 45, true), 3);
    }
}
