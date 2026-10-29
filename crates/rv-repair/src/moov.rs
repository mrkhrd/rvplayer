//! Building the new `moov` (the original's `CreateTrack` and `TMOVWriter.CopyHeader` /
//! `CopyHeaderFrom` with its `WriteSTxx` table writers): the reference's header is copied box by
//! box, durations are patched and the sample tables are rewritten for the found samples.

use std::io;

use crate::boxes::{BoxWriter, FourCC, atoms, be32};
use crate::reference::{RefTrack, Reference, Template};
use crate::scan::Frame;

/// A rebuilt track (`CreateTrack`).
pub(crate) struct OutTrack<'r> {
    pub src: &'r RefTrack,
    pub timescale: u32,
    /// Duration in media ticks (`durationUnscale`).
    pub duration: u64,
    pub tables: Tables,
}

pub(crate) enum Tables {
    /// One sample per chunk, each with the timing of the reference sample at the same index.
    PerSample(Vec<OutSample>),
    /// Constant-size (PCM) audio: chunks of `stsz` samples, all of one duration.
    Constant {
        /// (file offset, samples in the chunk)
        chunks: Vec<(u64, u32)>,
        samples: u64,
        delta: u32,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OutSample {
    pub offset: u64,
    pub size: u32,
    pub timing: Template,
    pub key: bool,
}

impl OutTrack<'_> {
    fn first_offsets(&self) -> Vec<u64> {
        match &self.tables {
            Tables::PerSample(s) => s.iter().map(|s| s.offset).collect(),
            Tables::Constant { chunks, .. } => chunks.iter().map(|c| c.0).collect(),
        }
    }

    fn seconds(&self) -> f64 {
        self.duration as f64 / f64::from(self.timescale)
    }
}

/// `CreateTrack`: the found samples with timing taken from the reference track. Sample *i* takes
/// the timing of reference sample *i*; past the reference's count, of sample *i mod period*.
/// `None` when nothing was found for the track (the original wrote an empty track, and failed on
/// its empty tables).
///
/// Deviation: the period is the reference's GOP length; the original used the length of its GOP
/// description string, which is one less.
pub(crate) fn create_track<'r>(src: &'r RefTrack, frames: &[Frame]) -> Option<OutTrack<'r>> {
    let timescale = if src.timescale == 0 {
        90_000
    } else {
        src.timescale
    };
    if src.fixed_size != 0 {
        let (bytes, per) = src.packet;
        let chunks: Vec<(u64, u32)> = frames
            .iter()
            .filter_map(|f| {
                let n = u64::from(f.size / bytes.max(1)) * u64::from(per);
                u32::try_from(n).ok().filter(|&n| n > 0).map(|n| (f.pos, n))
            })
            .collect();
        if chunks.is_empty() {
            return None;
        }
        let samples: u64 = chunks.iter().map(|c| u64::from(c.1)).sum();
        let delta = src.templates.first().map_or(1, |t| t.duration).max(1);
        return Some(OutTrack {
            src,
            timescale,
            duration: samples * u64::from(delta),
            tables: Tables::Constant {
                chunks,
                samples,
                delta,
            },
        });
    }
    if frames.is_empty() || src.templates.is_empty() {
        return None;
    }
    let period = src.period.clamp(1, src.templates.len());
    let mut ticks = 0i64;
    let samples = frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut timing = src
                .templates
                .get(i)
                .copied()
                .unwrap_or(src.templates[i % period]);
            if f.count > 1 {
                let d = i64::from(timing.duration as i32) * i64::from(f.count);
                timing.duration = d.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32 as u32;
            }
            ticks += i64::from(timing.duration as i32);
            OutSample {
                offset: f.pos,
                size: f.size,
                timing,
                key: f.key,
            }
        })
        .collect();
    Some(OutTrack {
        src,
        timescale,
        duration: ticks.max(0) as u64,
        tables: Tables::PerSample(samples),
    })
}

/// Delphi `Round`: half to even.
fn round(v: f64) -> u64 {
    v.round_ties_even().max(0.0) as u64
}

fn put32(d: &mut [u8], at: usize, v: u32) {
    if let Some(s) = d.get_mut(at..at + 4) {
        s.copy_from_slice(&v.to_be_bytes());
    }
}

fn put64(d: &mut [u8], at: usize, v: u64) {
    if let Some(s) = d.get_mut(at..at + 8) {
        s.copy_from_slice(&v.to_be_bytes());
    }
}

fn too_long() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "duration does not fit the header",
    )
}

/// `TMOVWriter.CopyHeader`: the new `moov` box. Only boxes the original copies are kept; tracks
/// that were not rebuilt are dropped.
pub(crate) fn build_moov(reference: &Reference, tracks: &[OutTrack]) -> io::Result<Vec<u8>> {
    let mvhd = atoms(&reference.moov).find(|a| &a.kind == b"mvhd");
    let scale = mvhd.and_then(|a| be32(a.body, if a.body[0] == 1 { 20 } else { 12 }));
    let next_id = reference.tracks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    let mut b = Builder {
        tracks,
        current: None,
        gscale: scale.filter(|&s| s > 0).unwrap_or(1000),
        next_id,
        w: BoxWriter::default(),
    };
    b.w.open(b"moov");
    b.copy(&reference.moov)?;
    b.w.close()?;
    Ok(b.w.buf)
}

struct Builder<'a, 'r> {
    tracks: &'a [OutTrack<'r>],
    current: Option<&'a OutTrack<'r>>,
    /// Movie timescale (the original's `gscale`).
    gscale: u32,
    next_id: u32,
    w: BoxWriter,
}

impl Builder<'_, '_> {
    /// `CopyHeaderFrom`.
    fn copy(&mut self, d: &[u8]) -> io::Result<()> {
        for a in atoms(d) {
            let kind: &FourCC = &a.kind;
            match kind {
                b"trak" | b"mdia" | b"minf" | b"stbl" => {
                    let start = self.w.buf.len();
                    self.w.open(kind);
                    self.copy(a.body)?;
                    self.w.close()?;
                    if kind == b"trak" {
                        if self.current.is_none() {
                            self.w.buf.truncate(start);
                        }
                        self.current = None;
                    }
                }
                b"tkhd" => {
                    let id = be32(a.body, if a.body[0] == 1 { 20 } else { 12 });
                    let tracks = self.tracks;
                    self.current = tracks.iter().find(|t| Some(t.src.id) == id);
                    if self.current.is_none() {
                        return Ok(()); // not rebuilt: the whole track is dropped
                    }
                    self.patched(kind, a.body)?;
                }
                b"mvhd" | b"mdhd" | b"udta" | b"tapt" | b"hdlr" | b"vmhd" | b"smhd" | b"stsd"
                | b"dinf" => self.patched(kind, a.body)?,
                _ => {
                    let Some(t) = self.current else { continue };
                    match kind {
                        b"edts" => self.edts(t)?,
                        b"stco" | b"co64" => self.chunk_offsets(t)?,
                        b"stsc" => self.stsc(t)?,
                        b"stss" => self.stss(t)?,
                        b"stsz" => self.stsz(t)?,
                        b"stts" => self.stts(t)?,
                        b"ctts" => self.ctts(t, a.body.first().copied().unwrap_or(0))?,
                        b"sdtp" => self.sdtp(t)?,
                        _ => {} // not copied by the original
                    }
                }
            }
        }
        Ok(())
    }

    /// A copied box with the movie / track durations and timescales patched.
    fn patched(&mut self, kind: &FourCC, body: &[u8]) -> io::Result<()> {
        let mut d = body.to_vec();
        let v1 = d.first() == Some(&1);
        match (kind, self.current) {
            (b"mvhd", _) => {
                let secs = self
                    .tracks
                    .iter()
                    .map(OutTrack::seconds)
                    .fold(0.0, f64::max);
                if !v1 {
                    while secs * f64::from(self.gscale) >= 2_147_483_648.0 && self.gscale > 1 {
                        self.gscale >>= 1;
                    }
                }
                let duration = round(secs * f64::from(self.gscale));
                if v1 {
                    put32(&mut d, 20, self.gscale);
                    put64(&mut d, 24, duration);
                    put32(&mut d, 108, self.next_id);
                } else {
                    put32(&mut d, 12, self.gscale);
                    put32(&mut d, 16, duration as u32);
                    put32(&mut d, 96, self.next_id);
                }
            }
            (b"tkhd", Some(t)) => {
                let duration = round(t.seconds() * f64::from(self.gscale));
                if v1 {
                    put64(&mut d, 28, duration);
                } else {
                    put32(&mut d, 20, u32::try_from(duration).map_err(|_| too_long())?);
                }
            }
            (b"mdhd", Some(t)) => {
                if v1 {
                    put32(&mut d, 20, t.timescale);
                    put64(&mut d, 24, t.duration);
                } else {
                    put32(&mut d, 12, t.timescale);
                    put32(
                        &mut d,
                        16,
                        u32::try_from(t.duration).map_err(|_| too_long())?,
                    );
                }
            }
            _ => {}
        }
        self.w.atom(kind, &d)
    }

    /// A one-entry edit list over the whole track. Deviation: the media time is the reference's
    /// (its first non-empty edit); the original always used the first sample's duration, which is
    /// what its own exports contain.
    fn edts(&mut self, t: &OutTrack) -> io::Result<()> {
        let media_time = t.src.edit_media_time.unwrap_or_else(|| match &t.tables {
            Tables::PerSample(s) => i64::from(s[0].timing.duration as i32),
            Tables::Constant { chunks, delta, .. } => i64::from(chunks[0].1) * i64::from(*delta),
        });
        self.w.open(b"edts");
        self.w.open_full(b"elst", 0, 0);
        self.w.u32(1);
        let segment = round(t.seconds() * f64::from(self.gscale));
        self.w.u32(u32::try_from(segment).map_err(|_| too_long())?);
        self.w
            .u32(i32::try_from(media_time).map_err(|_| too_long())? as u32);
        self.w.u32(0x0001_0000);
        self.w.close()?;
        self.w.close()
    }

    /// Chunk offsets; 64-bit when an offset does not fit a signed 32-bit value. (The original always
    /// wrote `stco` and failed on files over 2 GiB.)
    fn chunk_offsets(&mut self, t: &OutTrack) -> io::Result<()> {
        let offsets = t.first_offsets();
        let wide = offsets.iter().any(|&o| o > i32::MAX as u64);
        self.w.open_full(if wide { b"co64" } else { b"stco" }, 0, 0);
        self.w.u32(offsets.len() as u32);
        for o in offsets {
            if wide {
                self.w.u64(o);
            } else {
                self.w.u32(o as u32);
            }
        }
        self.w.close()
    }

    /// One sample per chunk (the original's single entry), or per-chunk runs for constant-size
    /// tracks (the original wrote the first chunk's count for all chunks).
    fn stsc(&mut self, t: &OutTrack) -> io::Result<()> {
        let mut entries: Vec<(u32, u32)> = Vec::new();
        match &t.tables {
            Tables::PerSample(_) => entries.push((1, 1)),
            Tables::Constant { chunks, .. } => {
                for (i, c) in chunks.iter().enumerate() {
                    if entries.last().is_none_or(|e| e.1 != c.1) {
                        entries.push((i as u32 + 1, c.1));
                    }
                }
            }
        }
        self.w.open_full(b"stsc", 0, 0);
        self.w.u32(entries.len() as u32);
        for (first, n) in entries {
            self.w.u32(first);
            self.w.u32(n);
            self.w.u32(1);
        }
        self.w.close()
    }

    fn stss(&mut self, t: &OutTrack) -> io::Result<()> {
        let Tables::PerSample(s) = &t.tables else {
            return Ok(()); // every PCM sample is a sync sample
        };
        let keys: Vec<u32> = (1..)
            .zip(s)
            .filter(|(_, s)| s.key)
            .map(|(i, _)| i)
            .collect();
        self.w.open_full(b"stss", 0, 0);
        self.w.u32(keys.len() as u32);
        keys.into_iter().for_each(|k| self.w.u32(k));
        self.w.close()
    }

    fn stsz(&mut self, t: &OutTrack) -> io::Result<()> {
        self.w.open_full(b"stsz", 0, 0);
        match &t.tables {
            Tables::PerSample(s) => {
                let first = s.first().map_or(0, |s| s.size);
                if s.iter().all(|s| s.size == first) {
                    self.w.u32(first);
                    self.w.u32(s.len() as u32);
                } else {
                    self.w.u32(0);
                    self.w.u32(s.len() as u32);
                    s.iter().for_each(|s| self.w.u32(s.size));
                }
            }
            Tables::Constant { samples, .. } => {
                // The reference's own constant size (the original wrote 1).
                self.w.u32(t.src.fixed_size);
                self.w.u32(u32::try_from(*samples).map_err(|_| too_long())?);
            }
        }
        self.w.close()
    }

    fn stts(&mut self, t: &OutTrack) -> io::Result<()> {
        let runs = match &t.tables {
            Tables::PerSample(s) => run_lengths(s.iter().map(|s| s.timing.duration)),
            Tables::Constant { samples, delta, .. } => {
                vec![(u32::try_from(*samples).map_err(|_| too_long())?, *delta)]
            }
        };
        self.table(b"stts", 0, &runs)
    }

    /// Composition offsets, run-length coded (the original wrote one entry per sample).
    fn ctts(&mut self, t: &OutTrack, version: u8) -> io::Result<()> {
        let Tables::PerSample(s) = &t.tables else {
            return Ok(());
        };
        self.table(
            b"ctts",
            version,
            &run_lengths(s.iter().map(|s| s.timing.ctts)),
        )
    }

    fn sdtp(&mut self, t: &OutTrack) -> io::Result<()> {
        let Tables::PerSample(s) = &t.tables else {
            return Ok(());
        };
        self.w.open_full(b"sdtp", 0, 0);
        s.iter().for_each(|s| self.w.buf.push(s.timing.sdtp));
        self.w.close()
    }

    fn table(&mut self, kind: &FourCC, version: u8, runs: &[(u32, u32)]) -> io::Result<()> {
        self.w.open_full(kind, version, 0);
        self.w.u32(runs.len() as u32);
        for &(n, v) in runs {
            self.w.u32(n);
            self.w.u32(v);
        }
        self.w.close()
    }
}

fn run_lengths(values: impl Iterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for v in values {
        match runs.last_mut() {
            Some(r) if r.1 == v => r.0 += 1,
            _ => runs.push((1, v)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(templates: Vec<Template>, period: usize) -> RefTrack {
        RefTrack {
            id: 1,
            handler: *b"vide",
            timescale: 0,
            sample_count: templates.len(),
            templates,
            sizes: Vec::new(),
            fixed_size: 0,
            packet: (1, 1),
            period,
            edit_media_time: None,
        }
    }

    fn frame(pos: u64, count: u32) -> Frame {
        Frame {
            pos,
            size: 10,
            key: false,
            count,
        }
    }

    #[test]
    fn timing_repeats_reference_gop_past_its_end() {
        let t = |d: u32, c: u32| Template {
            duration: d,
            ctts: c,
            sdtp: 0,
        };
        let src = track(vec![t(10, 0), t(20, 1), t(30, 2)], 2);
        let frames: Vec<Frame> = (0..6)
            .map(|i| frame(i * 10, if i == 1 { 3 } else { 1 }))
            .collect();
        let out = create_track(&src, &frames).unwrap();
        assert_eq!(out.timescale, 90_000);
        let Tables::PerSample(s) = &out.tables else {
            panic!()
        };
        let d: Vec<u32> = s.iter().map(|s| s.timing.duration).collect();
        assert_eq!(d, [10, 60, 30, 20, 10, 20]);
        assert_eq!(out.duration, 150);
        assert!(create_track(&src, &[]).is_none());
    }

    #[test]
    fn constant_size_tracks_count_samples_per_chunk() {
        let mut src = track(
            vec![Template {
                duration: 1,
                ..Default::default()
            }],
            1,
        );
        src.fixed_size = 1;
        src.packet = (4, 1);
        let frames = [
            frame(0, 1),
            Frame {
                size: 2,
                ..frame(50, 1)
            },
            Frame {
                size: 400,
                ..frame(90, 1)
            },
        ];
        let out = create_track(&src, &frames).unwrap();
        let Tables::Constant {
            chunks,
            samples,
            delta,
        } = &out.tables
        else {
            panic!()
        };
        assert_eq!(chunks, &[(0, 2), (90, 100)]);
        assert_eq!((*samples, *delta, out.duration), (102, 1, 102));
    }

    #[test]
    fn run_length_coding() {
        assert_eq!(
            run_lengths([5, 5, 7, 5].into_iter()),
            [(2, 5), (1, 7), (1, 5)]
        );
        assert!(run_lengths(std::iter::empty()).is_empty());
    }
}
