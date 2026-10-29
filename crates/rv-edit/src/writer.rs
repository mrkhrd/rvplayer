//! The MOV/MP4 writer: `ftyp`, `moov`, then the media data in `mdat` sections
//! (a port of `TSourceWriter.Save`, UnitSources L856-L975, and `TMOVWriter`, UnitSourceMOV
//! L1308-L1878).
//!
//! The header is built in memory from the first source's `moov` (copied boxes, regenerated
//! sample tables, the RVMI data track appended); chunk offsets are known before any media byte
//! is written because every sample is its own chunk and the `mdat` layout is planned up front.
//! Media data is then streamed sample by sample from the sources.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::group::{GroupTrack, OutFrame};
use crate::meta::RVMI_SCALE;
use crate::source::{Kind, be32, boxes, round};

/// Largest `mdat` body (`GetMaxDataSectionSize`): sections stay below 2 GiB.
const MAX_SECTION: u64 = 0x7fff_fff7;
/// 64-bit chunk offsets once the data plus this margin passes 2 GiB.
const CO64_MARGIN: u64 = 16 << 20;
/// Progress is reported every this many samples.
const PROGRESS_EVERY: usize = 128;
/// File index marking frames of the data track (their `offset` is the record index).
const DATA_FILE: u32 = u32::MAX;

/// Why writing stopped.
#[derive(Debug)]
pub(crate) enum WriteError {
    Cancelled,
    Io(io::Error),
}

impl From<io::Error> for WriteError {
    fn from(e: io::Error) -> Self {
        WriteError::Io(e)
    }
}

/// Everything the writer needs.
pub(crate) struct Output<'a> {
    pub tracks: &'a [GroupTrack],
    /// `ftyp` body of the first source.
    pub ftyp: Option<&'a [u8]>,
    /// `moov` body of the first source (the header template).
    pub moov: &'a [u8],
    /// Whether the first source is an `.mp4` (default `ftyp` when it has none).
    pub mp4: bool,
    /// RVMI records: (time in 1/50 s, bytes).
    pub records: Option<&'a [(i64, Vec<u8>)]>,
    /// Source paths by file index.
    pub files: &'a [PathBuf],
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    track: u32,
    frame: u32,
    /// Bytes reserved in `mdat` (the sample size, padded for IMA ADPCM).
    size: u32,
}

struct Plan {
    tracks: Vec<GroupTrack>,
    sections: Vec<(Vec<Entry>, u64)>,
    co64: bool,
}

/// `TSourceWriter.Save`: the data track and the `mdat` layout.
fn plan(out: &Output) -> Plan {
    let mut tracks: Vec<GroupTrack> = out.tracks.to_vec();
    let media = tracks.len();
    // IMA ADPCM frames are padded to a power of two (L866-L872).
    let mut audio_align = 0i64;
    for t in &tracks {
        if t.kind == Kind::Audio && &t.fourcc == b"ms\0\x11" {
            let first = t.frames.first().map_or(0, |f| i64::from(f.size));
            audio_align = 1;
            while audio_align < first {
                audio_align *= 2;
            }
        }
    }
    let padded = |t: &GroupTrack, size: u32| -> u32 {
        if t.kind == Kind::Audio && audio_align > 0 {
            let sz = i64::from(size);
            (((sz - 1) / audio_align + 1) * audio_align) as u32
        } else {
            size
        }
    };

    let mut sections: Vec<(Vec<Entry>, u64)> = Vec::new();
    // The data track goes first, at the start of the first section (L875-L915).
    if let Some(records) = out.records.filter(|r| !r.is_empty()) {
        let id = tracks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let mut frames: Vec<OutFrame> = records
            .iter()
            .enumerate()
            .map(|(i, (time, rec))| OutFrame {
                file: DATA_FILE,
                offset: i as u64,
                size: rec.len() as u32,
                time: *time as f64,
                dur: 0.0,
                ctts: 0,
                key: true,
                sdtp: 0,
            })
            .collect();
        for i in 0..frames.len().saturating_sub(1) {
            frames[i].dur = frames[i + 1].time - frames[i].time;
        }
        let last = frames[frames.len() - 1];
        let duration_unscale = round(last.time + last.dur);
        let entries = frames
            .iter()
            .enumerate()
            .map(|(i, f)| Entry {
                track: media as u32,
                frame: i as u32,
                size: f.size,
            })
            .collect::<Vec<_>>();
        let bytes = entries.iter().map(|e| u64::from(e.size)).sum();
        sections.push((entries, bytes));
        tracks.push(GroupTrack {
            kind: Kind::Other,
            fourcc: *b"RVMI",
            format: [0; 3],
            dsi: Vec::new(),
            id,
            scale: RVMI_SCALE,
            frames,
            duration_unscale,
            duration_ms: duration_unscale * 1000 / i64::from(RVMI_SCALE),
        });
    }

    // Interleave by time: the track with the earliest next sample, ties to the lower index
    // (L918-L961). All tracks start "at 0" before their first sample is taken.
    let mut cursor = vec![0usize; media];
    let mut next = vec![0.0f64; media];
    loop {
        let mut pick: Option<usize> = None;
        for i in 0..media {
            if cursor[i] < tracks[i].frames.len() && pick.is_none_or(|p| next[i] < next[p]) {
                pick = Some(i);
            }
        }
        let Some(t) = pick else { break };
        let track = &tracks[t];
        let f = track.frames[cursor[t]];
        let size = padded(track, f.size);
        if sections
            .last()
            .is_none_or(|(_, bytes)| bytes + u64::from(size) > MAX_SECTION)
        {
            sections.push((Vec::new(), 0));
        }
        let (entries, bytes) = sections.last_mut().unwrap();
        entries.push(Entry {
            track: t as u32,
            frame: cursor[t] as u32,
            size,
        });
        *bytes += u64::from(size);
        let scale = f64::from(track.scale.max(1));
        next[t] = match track.frames.get(cursor[t] + 1) {
            Some(n) => n.time / scale,
            None => (f.time + f.dur) / scale,
        };
        cursor[t] += 1;
    }
    let data: u64 = sections.iter().map(|(_, b)| *b).sum();
    Plan {
        tracks,
        sections,
        co64: data + CO64_MARGIN > 0x7fff_ffff,
    }
}

/// Builds the header (`ftyp` + `moov`) with the final chunk offsets.
fn header(out: &Output, plan: &Plan) -> Vec<u8> {
    let mut h = Header {
        buf: Vec::with_capacity(out.moov.len() + 4096),
        tracks: &plan.tracks,
        media: out.tracks.len(),
        current: None,
        gscale: 1000,
        co64: plan.co64,
        stubs: Vec::new(),
    };
    // ftyp: copied, else a default by the source's extension (L1377-L1388).
    let ftyp: Vec<u8> = match out.ftyp {
        Some(b) => b.to_vec(),
        None if out.mp4 => [&b"mp41"[..], &[0; 4], b"mp41isom"].concat(),
        None => [&b"qt  "[..], &[0; 4], b"qt  ", &[0; 4]].concat(),
    };
    put_box(&mut h.buf, b"ftyp", &ftyp);
    let moov = h.open(b"moov");
    h.copy_from(out.moov);
    if plan.tracks.len() > h.media {
        h.data_trak(h.media);
    }
    h.close(moov);

    // Chunk offsets: one sample per chunk, sections after the header (L1700-L1729).
    let mut offsets: Vec<Vec<u64>> = plan
        .tracks
        .iter()
        .map(|t| vec![0; t.frames.len()])
        .collect();
    let mut pos = h.buf.len() as u64;
    for (entries, _) in &plan.sections {
        pos += 8;
        for e in entries {
            offsets[e.track as usize][e.frame as usize] = pos;
            pos += u64::from(e.size);
        }
    }
    let stubs = std::mem::take(&mut h.stubs);
    let mut buf = h.buf;
    for (track, at) in stubs {
        for (k, &o) in offsets[track].iter().enumerate() {
            if plan.co64 {
                buf[at + 8 * k..at + 8 * k + 8].copy_from_slice(&o.to_be_bytes());
            } else {
                buf[at + 4 * k..at + 4 * k + 4].copy_from_slice(&(o as u32).to_be_bytes());
            }
        }
    }
    buf
}

fn put_box(buf: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    buf.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    buf.extend_from_slice(kind);
    buf.extend_from_slice(body);
}

fn put32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}

struct Header<'a> {
    buf: Vec<u8>,
    tracks: &'a [GroupTrack],
    /// Media tracks come first in `tracks`; the data track (if any) follows.
    media: usize,
    current: Option<usize>,
    /// Movie time scale (`gscale`).
    gscale: u32,
    co64: bool,
    /// (track, buffer position of the first chunk offset) of every `stco` / `co64` written.
    stubs: Vec<(usize, usize)>,
}

impl Header<'_> {
    fn open(&mut self, kind: &[u8; 4]) -> usize {
        let at = self.buf.len();
        self.buf.extend_from_slice(&[0; 4]);
        self.buf.extend_from_slice(kind);
        at
    }

    fn close(&mut self, at: usize) {
        let size = (self.buf.len() - at) as u32;
        self.buf[at..at + 4].copy_from_slice(&size.to_be_bytes());
    }

    /// Track duration in the movie time scale.
    fn movie_duration(&self, t: &GroupTrack) -> i64 {
        if t.scale == 0 {
            0
        } else {
            round(t.duration_unscale as f64 / f64::from(t.scale) * f64::from(self.gscale))
        }
    }

    /// `CopyHeaderFrom`: walks the template `moov`, keeps the tracks that are written,
    /// copies their descriptive boxes and regenerates the sample tables (L1485-L1600).
    fn copy_from(&mut self, d: &[u8]) {
        for b in boxes(d) {
            let body = &d[b.body..b.end];
            if &b.kind == b"tkhd" {
                let id_at = if body.first() == Some(&1) { 20 } else { 12 };
                let id = (body.len() >= id_at + 4).then(|| be32(body, id_at));
                self.current = self.tracks[..self.media]
                    .iter()
                    .position(|t| Some(t.id) == id);
                if self.current.is_none() {
                    return; // the rest of this trak is skipped; the trak is dropped below
                }
            }
            match &b.kind {
                b"trak" | b"mdia" | b"minf" | b"stbl" => {
                    let at = self.open(&b.kind);
                    self.copy_from(body);
                    self.close(at);
                    if &b.kind == b"trak" {
                        if self.current.is_none() {
                            self.buf.truncate(at);
                        }
                        self.current = None;
                    }
                }
                b"edts" => {
                    if let Some(t) = self.current {
                        self.edts(t);
                    }
                }
                b"stco" | b"co64" | b"stsc" | b"stss" | b"stsz" | b"stts" | b"ctts" | b"sdtp" => {
                    if let Some(t) = self.current {
                        self.table(&b.kind, t);
                    }
                }
                b"mdhd" | b"mvhd" | b"tkhd" | b"udta" | b"tapt" | b"hdlr" | b"vmhd" | b"smhd"
                | b"stsd" | b"dinf" => {
                    let mut body = body.to_vec();
                    self.patch(&b.kind, &mut body);
                    put_box(&mut self.buf, &b.kind, &body);
                }
                _ => {}
            }
        }
    }

    fn patch(&mut self, kind: &[u8; 4], body: &mut [u8]) {
        let v1 = body.first() == Some(&1);
        let set32 = |body: &mut [u8], at: usize, v: u32| {
            if let Some(s) = body.get_mut(at..at + 4) {
                s.copy_from_slice(&v.to_be_bytes());
            }
        };
        let set64 = |body: &mut [u8], at: usize, v: u64| {
            if let Some(s) = body.get_mut(at..at + 8) {
                s.copy_from_slice(&v.to_be_bytes());
            }
        };
        match kind {
            b"mvhd" => {
                let scale_at = if v1 { 20 } else { 12 };
                if body.len() >= scale_at + 4 {
                    self.gscale = be32(body, scale_at);
                }
                if self.gscale == 0 {
                    self.gscale = 1000;
                }
                let du = self.tracks[..self.media]
                    .iter()
                    .filter(|t| t.scale > 0)
                    .map(|t| t.duration_unscale as f64 / f64::from(t.scale))
                    .fold(0.0, f64::max);
                while du * f64::from(self.gscale) >= 2_147_483_648.0 && self.gscale > 1 {
                    self.gscale >>= 1;
                }
                let duration = round(du * f64::from(self.gscale));
                let next_id = self.tracks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                set32(body, scale_at, self.gscale);
                if v1 {
                    set64(body, 24, duration as u64);
                    set32(body, 108, next_id);
                } else {
                    set32(body, 16, duration as u32);
                    set32(body, 96, next_id);
                }
            }
            b"mdhd" => {
                if let Some(t) = self.current.map(|i| &self.tracks[i]) {
                    if v1 {
                        set32(body, 20, t.scale);
                        set64(body, 24, t.duration_unscale as u64);
                    } else {
                        set32(body, 12, t.scale);
                        set32(body, 16, t.duration_unscale as u32);
                    }
                }
            }
            b"tkhd" => {
                if let Some(t) = self.current.map(|i| &self.tracks[i])
                    && t.scale > 0
                {
                    let d = self.movie_duration(t);
                    if v1 {
                        set64(body, 28, d as u64);
                    } else {
                        set32(body, 20, d as u32);
                    }
                }
            }
            _ => {}
        }
    }

    /// One edit covering the track, starting one frame in (L1525-L1537).
    fn edts(&mut self, t: usize) {
        let track = &self.tracks[t];
        let segment = self.movie_duration(track) as u32;
        let media_time = track.frames.first().map_or(3003, |f| round(f.dur)) as u32;
        let edts = self.open(b"edts");
        let mut elst = vec![0u8; 4];
        for v in [1, segment, media_time, 0x0001_0000] {
            put32(&mut elst, v);
        }
        put_box(&mut self.buf, b"elst", &elst);
        self.close(edts);
    }

    fn table(&mut self, kind: &[u8; 4], t: usize) {
        let frames = &self.tracks[t].frames;
        let mut b = vec![0u8; 4]; // version 0, flags 0
        match kind {
            b"stco" | b"co64" => {
                let kind = if self.co64 { b"co64" } else { b"stco" };
                put32(&mut b, frames.len() as u32);
                let width = if self.co64 { 8 } else { 4 };
                let at = self.buf.len() + 8 + b.len();
                b.resize(b.len() + width * frames.len(), 0);
                self.stubs.push((t, at));
                put_box(&mut self.buf, kind, &b);
                return;
            }
            b"stsc" => {
                for v in [1, 1, 1, 1] {
                    put32(&mut b, v);
                }
            }
            b"stss" => {
                let keys: Vec<u32> = (0..frames.len())
                    .filter(|&i| frames[i].key)
                    .map(|i| i as u32 + 1)
                    .collect();
                put32(&mut b, keys.len() as u32);
                for k in keys {
                    put32(&mut b, k);
                }
            }
            b"stsz" => {
                let all_eq = frames.windows(2).all(|w| w[0].size == w[1].size);
                if all_eq {
                    put32(&mut b, frames.first().map_or(0, |f| f.size));
                    put32(&mut b, frames.len() as u32);
                } else {
                    put32(&mut b, 0);
                    put32(&mut b, frames.len() as u32);
                    for f in frames {
                        put32(&mut b, f.size);
                    }
                }
            }
            b"stts" => {
                let mut runs: Vec<(u32, i64)> = Vec::new();
                for f in frames {
                    let d = round(f.dur);
                    match runs.last_mut() {
                        Some((n, v)) if *v == d => *n += 1,
                        _ => runs.push((1, d)),
                    }
                }
                put32(&mut b, runs.len() as u32);
                for (n, d) in runs {
                    put32(&mut b, n);
                    put32(&mut b, d as i32 as u32);
                }
            }
            b"ctts" => {
                put32(&mut b, frames.len() as u32);
                for f in frames {
                    put32(&mut b, 1);
                    put32(&mut b, f.ctts as u32);
                }
            }
            b"sdtp" => b.extend(frames.iter().map(|f| f.sdtp)),
            _ => return,
        }
        put_box(&mut self.buf, kind, &b);
    }

    /// `WriteTextTrackMoov`: the RVMI data track (L1612-L1665).
    fn data_trak(&mut self, t: usize) {
        let track = &self.tracks[t];
        let trak = self.open(b"trak");
        let mut tkhd = Vec::with_capacity(84);
        for v in [0, 0, 0, track.id, 0, self.movie_duration(track) as u32] {
            put32(&mut tkhd, v);
        }
        tkhd.resize(84, 0); // reserved, layer, group, volume, zero matrix, zero size
        put_box(&mut self.buf, b"tkhd", &tkhd);
        let mdia = self.open(b"mdia");
        let mut mdhd = Vec::with_capacity(24);
        for v in [0, 0, 0, track.scale, track.duration_unscale as u32, 0] {
            put32(&mut mdhd, v);
        }
        put_box(&mut self.buf, b"mdhd", &mdhd);
        let mut hdlr = vec![0u8; 8];
        hdlr.extend_from_slice(b"data");
        hdlr.extend_from_slice(&[0; 12]);
        hdlr.extend_from_slice(b" GPS and G-Sensor");
        put_box(&mut self.buf, b"hdlr", &hdlr);
        let minf = self.open(b"minf");
        let stbl = self.open(b"stbl");
        let mut stsd = vec![0u8; 4];
        put32(&mut stsd, 1);
        put_box(&mut stsd, b"RVMI", &[]);
        put_box(&mut self.buf, b"stsd", &stsd);
        self.current = Some(t);
        for kind in [b"stts", b"stsc", b"stsz", b"stco"] {
            self.table(kind, t);
        }
        self.current = None;
        self.close(stbl);
        self.close(minf);
        self.close(mdia);
        self.close(trak);
    }
}

/// Source files opened on demand while copying.
struct Readers<'a> {
    files: &'a [PathBuf],
    open: HashMap<u32, (BufReader<File>, u64)>,
}

impl Readers<'_> {
    fn read(&mut self, file: u32, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        if !self.open.contains_key(&file) {
            if self.open.len() >= 8 {
                self.open.clear();
            }
            let path = &self.files[file as usize];
            let f = File::open(path).map_err(|e| with_path(e, path))?;
            self.open
                .insert(file, (BufReader::with_capacity(1 << 20, f), u64::MAX));
        }
        let (r, pos) = self.open.get_mut(&file).unwrap();
        if *pos != offset {
            if *pos == u64::MAX {
                r.seek(SeekFrom::Start(offset))?;
            } else {
                r.seek_relative(offset as i64 - *pos as i64)?;
            }
        }
        *pos = u64::MAX;
        r.read_exact(buf)
            .map_err(|e| with_path(e, &self.files[file as usize]))?;
        *pos = offset + buf.len() as u64;
        Ok(())
    }
}

fn with_path(e: io::Error, path: &Path) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

/// Writes the output file. `progress(output_seconds, fraction)` is called every 128 samples
/// and at the end of every section; returning `false` cancels (the caller removes the file).
/// Returns the output size in bytes.
pub(crate) fn write(
    out: &Output,
    path: &Path,
    progress: &mut dyn FnMut(f64, f64) -> bool,
) -> Result<u64, WriteError> {
    let plan = plan(out);
    let header = header(out, &plan);
    let data: u64 = plan.sections.iter().map(|(_, b)| 8 + b).sum();
    let total = header.len() as u64 + data;

    let mut w = BufWriter::with_capacity(4 << 20, File::create(path)?);
    w.write_all(&header)?;
    let mut readers = Readers {
        files: out.files,
        open: HashMap::new(),
    };
    let mut buf = Vec::new();
    let mut written = header.len() as u64;
    let mut time = 0.0;
    let mut since_report = 0usize;
    for (entries, bytes) in &plan.sections {
        w.write_all(&((bytes + 8) as u32).to_be_bytes())?;
        w.write_all(b"mdat")?;
        written += 8;
        for e in entries {
            let track = &plan.tracks[e.track as usize];
            let f = &track.frames[e.frame as usize];
            buf.clear();
            buf.resize(f.size as usize, 0);
            if f.file == DATA_FILE {
                if let Some(rec) = out.records.and_then(|r| r.get(f.offset as usize)) {
                    buf.copy_from_slice(&rec.1);
                }
            } else {
                readers.read(f.file, f.offset, &mut buf)?;
                time = f.time / f64::from(track.scale.max(1));
            }
            buf.resize(e.size as usize, 0);
            w.write_all(&buf)?;
            written += u64::from(e.size);
            since_report += 1;
            if since_report >= PROGRESS_EVERY {
                since_report = 0;
                if !progress(time, written as f64 / total as f64) {
                    return Err(WriteError::Cancelled);
                }
            }
        }
        if !progress(time, written as f64 / total as f64) {
            return Err(WriteError::Cancelled);
        }
    }
    w.flush()?;
    w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: u32, scale: u32, times: &[f64], size: u32) -> GroupTrack {
        GroupTrack {
            kind: Kind::Video,
            fourcc: *b"avc1",
            format: [0; 3],
            dsi: Vec::new(),
            id,
            scale,
            frames: times
                .iter()
                .map(|&t| OutFrame {
                    file: 0,
                    offset: 0,
                    size,
                    time: t,
                    dur: 1.0,
                    ctts: 0,
                    key: true,
                    sdtp: 0,
                })
                .collect(),
            duration_unscale: 0,
            duration_ms: 0,
        }
    }

    #[test]
    fn interleaves_by_time_with_ties_to_lower_track() {
        let tracks = [
            track(1, 10, &[0.0, 1.0, 2.0], 5),
            track(2, 20, &[0.0, 1.0, 2.0, 3.0], 3),
        ];
        let out = Output {
            tracks: &tracks,
            ftyp: None,
            moov: &[],
            mp4: false,
            records: None,
            files: &[],
        };
        let p = plan(&out);
        let order: Vec<(u32, u32)> = p.sections[0].0.iter().map(|e| (e.track, e.frame)).collect();
        // track 0 at 0, 0.1, 0.2; track 1 at 0, 0.05, 0.1, 0.15
        assert_eq!(
            order,
            [(0, 0), (1, 0), (1, 1), (0, 1), (1, 2), (1, 3), (0, 2)]
        );
        assert!(!p.co64);
    }

    #[test]
    fn large_outputs_use_64_bit_offsets_and_split_sections() {
        let tracks = [track(1, 1, &[0.0, 1.0, 2.0], 0x5000_0000)];
        let out = Output {
            tracks: &tracks,
            ftyp: None,
            moov: &[],
            mp4: false,
            records: None,
            files: &[],
        };
        let p = plan(&out);
        assert!(p.co64);
        assert_eq!(p.sections.len(), 3, "a section holds less than 2 GiB");
    }
}
