//! Finding the media data of the broken file and walking it for frames: the original's
//! `SeekToMDAT`, `ScanFrames`, `IsVideoFrameStart`, `SplitLastAudio`, `AddFrame` and the read
//! window behind `GetCurrentByte` / `DropHead`.

use std::io::{self, Read, Seek, SeekFrom};

use crate::boxes::be32;
use crate::nal::{LENGTH_SIZE, MAX_NAL, StartRule, VideoStyle};

/// A found sample: a video frame, or a run of audio between two video frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Frame {
    pub pos: u64,
    pub size: u32,
    pub key: bool,
    /// Audio frames this sample stands for (`SplitLastAudio` counts unsplittable runs as several);
    /// its duration is multiplied by this.
    pub count: u32,
}

/// Where the media data of the broken file is (`SeekToMDAT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Mdat {
    /// Position of the `mdat` header; `None` when there is none and the whole file is scanned
    /// (a new `ftyp` + `mdat` header is then written over the first 24 bytes).
    pub start: Option<u64>,
    /// Header length: 8, or 16 with a 64-bit size.
    pub header: u64,
    /// End of the media data: the declared end when it is usable, else the end of the file.
    pub end: u64,
    /// The size field must be rewritten (0, too small, or past the end of the file).
    pub size_invalid: bool,
    /// Space between `ftyp` and `mdat` (position, size): other boxes the camera reserved there,
    /// which the new `moov` may overwrite.
    pub free: Option<(u64, u64)>,
}

impl Mdat {
    pub fn payload(&self) -> u64 {
        self.start.map_or(0, |s| s + self.header)
    }
}

/// Walks the top-level boxes from the start of the file to the `mdat`.
///
/// Deviations from the original, all for files it would damage: a 64-bit `mdat` size is honoured
/// (the original overwrote it with a 32-bit size); a declared size past the end of the file is
/// treated like a missing one (the original appended the new header past the end, leaving a gap);
/// 8-byte boxes such as `wide` are skipped (the original stopped and treated the file as raw).
pub(crate) fn find_mdat<R: Read + Seek>(r: &mut R, len: u64) -> io::Result<Mdat> {
    let mut pos = 0u64;
    let mut free_pos = None;
    while pos + 8 <= len {
        let h = read_at(r, pos, 16.min(len - pos) as usize)?;
        let size32 = be32(&h, 0).unwrap_or(0);
        let kind = &h[4..8];
        // The original's free position is 0 when unset, so a box at offset 0 never counts.
        if free_pos.is_none() && pos > 0 && kind != b"ftyp" && kind != b"mdat" {
            free_pos = Some(pos);
        }
        if kind == b"mdat" {
            let free = free_pos.map(|fp| (fp, pos - fp)).filter(|&(_, n)| n > 0);
            let (header, declared) = match size32 {
                1 if h.len() == 16 => (16, u64::from_be_bytes(h[8..16].try_into().unwrap())),
                1 => (8, 0),
                n => (8, u64::from(n)),
            };
            let valid = declared > header && pos + declared <= len;
            return Ok(Mdat {
                start: Some(pos),
                header,
                end: if valid { pos + declared } else { len },
                size_invalid: !valid,
                free,
            });
        }
        let size = match size32 {
            1 if h.len() == 16 => u64::from_be_bytes(h[8..16].try_into().unwrap()),
            n => u64::from(n),
        };
        if size < 8 {
            break;
        }
        pos = pos.saturating_add(size);
    }
    Ok(Mdat {
        start: None,
        header: 0,
        end: len,
        size_invalid: true,
        free: None,
    })
}

pub(crate) fn read_at<R: Read + Seek>(r: &mut R, pos: u64, n: usize) -> io::Result<Vec<u8>> {
    r.seek(SeekFrom::Start(pos))?;
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Read window over the file (the original keeps a 1 MiB buffer and slides it by halves).
/// Bytes past the end of the file read as zero.
pub(crate) struct Window<R> {
    r: R,
    len: u64,
    start: u64,
    buf: Vec<u8>,
}

const WINDOW: u64 = 1 << 20;
/// Bytes guaranteed after a peeked position (the original keeps 32 in its buffer).
const PEEK: usize = 32;

impl<R: Read + Seek> Window<R> {
    pub fn new(r: R, len: u64) -> Self {
        Self {
            r,
            len,
            start: 0,
            buf: Vec::new(),
        }
    }

    /// At least 32 bytes starting at `pos`.
    pub fn peek(&mut self, pos: u64) -> io::Result<&[u8]> {
        let end = self.start + self.buf.len() as u64;
        if pos < self.start || pos + PEEK as u64 > end {
            let n = self.len.saturating_sub(pos).min(WINDOW) as usize;
            self.buf = if n > 0 {
                read_at(&mut self.r, pos, n)?
            } else {
                Vec::new()
            };
            if self.buf.len() < PEEK {
                self.buf.resize(PEEK, 0);
            }
            self.start = pos;
        }
        Ok(&self.buf[(pos - self.start) as usize..])
    }

    pub fn read(&mut self, pos: u64, n: usize) -> io::Result<Vec<u8>> {
        read_at(&mut self.r, pos, n)
    }

    pub fn into_inner(self) -> R {
        self.r
    }
}

/// Scan progress sink and cancellation flag.
pub(crate) trait Progress {
    fn cancelled(&self) -> bool;
    fn update(&self, pos: u64, frames: u64);
}

/// Progress is reported (and cancellation checked) at least this often.
const REPORT_STEP: u64 = 256 << 10;
/// Gap between video frames above which it is taken as audio (the original's `$40`).
const MIN_AUDIO_GAP: u64 = 0x40;
/// Audio runs at least this many median sizes long are split (the original's 1.6).
const SPLIT_FACTOR: f64 = 1.6;
/// Audio runs this long or longer are never split (the original's 1 MiB).
const MAX_SPLIT_RUN: u32 = 0x10_0000;

#[derive(Debug, Default)]
pub(crate) struct Found {
    pub video: Vec<Frame>,
    pub audio: Vec<Frame>,
    /// Audio frames, counting the extra frames of unsplittable runs (`audio.frames`).
    pub audio_frames: u64,
}

impl Found {
    pub fn frames(&self) -> u64 {
        self.video.len() as u64 + self.audio_frames
    }

    /// `AddFrame`: empty samples are ignored.
    fn add_audio(&mut self, pos: u64, size: u64) {
        if let Ok(size) = u32::try_from(size)
            && size > 0
        {
            self.audio.push(Frame {
                pos,
                size,
                key: true,
                count: 1,
            });
            self.audio_frames += 1;
        }
    }
}

/// Scan was cancelled.
#[derive(Debug)]
pub(crate) struct Cancelled;

enum Walk {
    /// A complete frame ending at `end`.
    Frame { end: u64, key: bool },
    /// An implausible NAL length at this position.
    Broken(u64),
    /// The frame runs past the end of the data.
    Incomplete,
}

pub(crate) struct Scanner<'a, R> {
    win: Window<R>,
    style: &'a VideoStyle,
    avg_audio: u32,
    limit: u64,
    pub found: Found,
}

impl<'a, R: Read + Seek> Scanner<'a, R> {
    pub fn new(r: R, len: u64, style: &'a VideoStyle, avg_audio: u32) -> Self {
        Self {
            win: Window::new(r, len),
            style,
            avg_audio,
            limit: len,
            found: Found::default(),
        }
    }

    pub fn into_inner(self) -> R {
        self.win.into_inner()
    }

    /// `ScanFrames`: walks from `start` to the end of the file, collecting video frames and the
    /// audio runs between them.
    ///
    /// Deviations: a frame that runs past the end of the file is not added (the original added it,
    /// pointing into the new header); an audio run starts no earlier than the end of the previous
    /// one (after a false frame start the original added the same bytes again).
    pub fn scan(
        &mut self,
        start: u64,
        progress: &dyn Progress,
    ) -> io::Result<Result<(), Cancelled>> {
        let mut pos = start;
        let mut report = pos;
        let original = self.style.rule == StartRule::Original;
        while pos < self.limit {
            if pos >= report {
                if progress.cancelled() {
                    return Ok(Err(Cancelled));
                }
                progress.update(pos, self.found.frames());
                report = pos + REPORT_STEP;
            }
            let Some(head) = self.frame_start(pos)? else {
                pos += 1;
                continue;
            };
            let body = pos + head;
            // The original takes the audio before a frame as soon as it sees the frame start; a
            // learned start is only a candidate until its NAL chain has been walked.
            if original {
                self.audio_before(pos, head)?;
            }
            match self.walk(body)? {
                Walk::Frame { end, key } => {
                    if !original {
                        self.audio_before(pos, head)?;
                    }
                    self.found.video.push(Frame {
                        pos: body,
                        size: (end - body) as u32,
                        key,
                        count: 1,
                    });
                    pos = end;
                }
                // The original resumes scanning where the frame broke off; a learned start is
                // only a candidate, so scanning resumes right after it.
                Walk::Broken(at) if original => pos = at.max(pos + 1),
                Walk::Incomplete if original => break,
                Walk::Broken(_) | Walk::Incomplete => pos += 1,
            }
        }
        progress.update(pos.min(self.limit), self.found.frames());
        Ok(Ok(()))
    }

    /// The bytes between the previous video frame and a frame (header) starting at `pos` are audio
    /// when there are more than 64 of them. With a 16-byte frame header, the audio run has one too.
    fn audio_before(&mut self, pos: u64, head: u64) -> io::Result<()> {
        let Some(last) = self.found.video.last() else {
            return Ok(()); // audio before the first video frame is not recovered (as the original)
        };
        let mut gap = last.pos + u64::from(last.size) + head;
        if let Some(a) = self.found.audio.last() {
            gap = gap.max(a.pos + u64::from(a.size));
        }
        if pos > gap && pos - gap > MIN_AUDIO_GAP {
            self.found.add_audio(gap, pos - gap);
            self.split_last_audio()?;
        }
        Ok(())
    }

    /// `IsVideoFrameStart`: the length of the frame's header when a frame starts at `pos`.
    fn frame_start(&mut self, pos: u64) -> io::Result<Option<u64>> {
        let style = self.style;
        let p = self.win.peek(pos)?;
        let be = |at| be32(p, at).unwrap_or(0) as i32;
        Ok(match style.rule {
            StartRule::Original => {
                let size = be(8);
                if (0x400..MAX_NAL as i32).contains(&size) && size - 4 == be(16) {
                    Some(16)
                } else if be(0) == 2 && p[4] == 0x09 {
                    Some(0)
                } else {
                    None
                }
            }
            StartRule::HevcAud => style.is_aud(be(0) as u32, style.header(p)).then_some(0),
            StartRule::Learned => style.learned_start(p).then_some(0),
        })
    }

    /// Walks the NAL units of a frame from `pos` through its (first) slice.
    fn walk(&mut self, mut pos: u64) -> io::Result<Walk> {
        let style = self.style;
        let start = pos;
        let prefix = (LENGTH_SIZE + style.header_len()) as u64;
        loop {
            if pos + prefix > self.limit {
                return Ok(Walk::Incomplete);
            }
            let p = self.win.peek(pos)?;
            let len = be32(p, 0).unwrap_or(0);
            let h = style.header(p);
            if !(2..=MAX_NAL).contains(&len) {
                return Ok(Walk::Broken(pos));
            }
            pos += LENGTH_SIZE as u64 + u64::from(len);
            if pos > self.limit {
                return Ok(Walk::Incomplete);
            }
            if !style.is_vcl(h[0]) {
                continue;
            }
            if style.multi_slice {
                // Further slices of the same picture (extension; see `VideoStyle::multi_slice`).
                while pos + prefix < self.limit {
                    let p = self.win.peek(pos)?;
                    let len = be32(p, 0).unwrap_or(0);
                    if !(2..=MAX_NAL).contains(&len)
                        || !style.same_slice_kind(h, style.header(p))
                        || style.first_slice(p)
                    {
                        break;
                    }
                    pos += LENGTH_SIZE as u64 + u64::from(len);
                    if pos > self.limit {
                        return Ok(Walk::Incomplete);
                    }
                }
            }
            if pos - start > u64::from(u32::MAX) {
                return Ok(Walk::Broken(start));
            }
            return Ok(Walk::Frame {
                end: pos,
                key: style.is_key(h[0]),
            });
        }
    }

    /// `SplitLastAudio`: a run of audio between two video frames usually holds several audio
    /// frames. Frames of compressed audio start alike, so the run is cut where its first bytes
    /// repeat, half to one and a half median frame sizes further on. A run that cannot be cut
    /// counts as `trunc(len / median - 0.7)` extra frames.
    fn split_last_audio(&mut self) -> io::Result<()> {
        let avg = self.avg_audio as usize;
        if avg <= 9 {
            return Ok(());
        }
        let min_len = SPLIT_FACTOR * avg as f64;
        let last = *self
            .found
            .audio
            .last()
            .expect("an audio run was just added");
        if last.size >= MAX_SPLIT_RUN || f64::from(last.size) < min_len {
            return Ok(());
        }
        let mut data = self.win.read(last.pos, last.size as usize)?;
        if data[0] == 0 {
            return Ok(());
        }
        let mut start = last.pos;
        while data.len() as f64 >= min_len {
            let (mut max_i, mut max_z) = (0usize, 0usize);
            for i in avg / 2..=avg * 3 / 2 {
                // Leading bytes of the run that repeat from offset i - 1 (at most 10). The
                // original raised a range error past the end of the run; that ends the match here.
                let z = (0..10)
                    .take_while(|&z| data.get(i + z - 1).is_some_and(|&b| b == data[z]))
                    .count();
                if max_z < z {
                    max_z = z;
                    max_i = i - 1;
                }
            }
            if max_z < 2 {
                break;
            }
            let n = self.found.audio.len();
            self.found.audio[n - 1].size = max_i as u32;
            start += max_i as u64;
            self.found.add_audio(start, (data.len() - max_i) as u64);
            data.drain(..max_i);
        }
        let extra = (data.len() as f64 / avg as f64 - 0.7).trunc();
        if extra > 0.0 {
            let extra = extra as u32;
            self.found.audio_frames += u64::from(extra);
            if let Some(last) = self.found.audio.last_mut() {
                last.count += extra;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct Quiet;
    impl Progress for Quiet {
        fn cancelled(&self) -> bool {
            false
        }
        fn update(&self, _: u64, _: u64) {}
    }

    fn mk(kind: &[u8; 4], body_len: usize) -> Vec<u8> {
        let mut v = ((body_len + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.resize(body_len + 8, 0xaa);
        v
    }

    #[test]
    fn finds_mdat_and_free_space() {
        let file = [mk(b"ftyp", 16), mk(b"free", 100), mk(b"mdat", 50)].concat();
        let m = find_mdat(&mut Cursor::new(&file), file.len() as u64).unwrap();
        assert_eq!(m.start, Some(132));
        assert_eq!((m.header, m.end, m.size_invalid), (8, 190, false));
        assert_eq!(m.free, Some((24, 108)));
        assert_eq!(m.payload(), 140);

        // size 0, and a size past the end of the file, are rewritten to the end of the file
        let mut zero = [mk(b"ftyp", 16), mk(b"mdat", 50)].concat();
        zero[24..28].copy_from_slice(&[0; 4]);
        let m = find_mdat(&mut Cursor::new(&zero), zero.len() as u64).unwrap();
        assert_eq!((m.end, m.size_invalid, m.free), (82, true, None));
        let cut = &zero[..60];
        let mut big = cut.to_vec();
        big[24..28].copy_from_slice(&1000u32.to_be_bytes());
        let m = find_mdat(&mut Cursor::new(&big), 60).unwrap();
        assert_eq!((m.end, m.size_invalid), (60, true));

        // 8-byte boxes are skipped; no mdat at all scans the whole file
        let wide = [mk(b"ftyp", 16), mk(b"wide", 0), mk(b"mdat", 10)].concat();
        let m = find_mdat(&mut Cursor::new(&wide), wide.len() as u64).unwrap();
        assert_eq!(m.start, Some(32));
        let raw = vec![0u8; 100];
        let m = find_mdat(&mut Cursor::new(&raw), 100).unwrap();
        assert_eq!((m.start, m.payload(), m.end), (None, 0, 100));

        // 64-bit size
        let mut large = mk(b"ftyp", 16);
        large.extend_from_slice(&[0, 0, 0, 1]);
        large.extend_from_slice(b"mdat");
        large.extend_from_slice(&26u64.to_be_bytes());
        large.extend_from_slice(&[0; 10]);
        let m = find_mdat(&mut Cursor::new(&large), large.len() as u64).unwrap();
        assert_eq!(
            (m.start, m.header, m.end, m.size_invalid),
            (Some(24), 16, 50, false)
        );
    }

    fn nal(header: &[u8], len: usize) -> Vec<u8> {
        let mut v = (len as u32).to_be_bytes().to_vec();
        v.extend_from_slice(header);
        v.resize(4 + len, 0x5a);
        v
    }

    #[test]
    fn scans_aud_frames_and_audio_between_them() {
        let style = VideoStyle::original();
        let v0 = [
            nal(&[9, 0x10], 2),
            nal(&[0x67], 20),
            nal(&[0x65, 0x88], 300),
        ]
        .concat();
        let a0 = vec![0x21u8; 100];
        let v1 = [nal(&[9, 0x30], 2), nal(&[0x41, 0x9a], 200)].concat();
        let tail = nal(&[9, 0x30], 2); // a frame cut off after its delimiter
        let data = [vec![1u8; 10], v0.clone(), a0, v1.clone(), vec![7; 30], tail].concat();
        let mut s = Scanner::new(Cursor::new(&data), data.len() as u64, &style, 0);
        s.scan(0, &Quiet).unwrap().unwrap();
        let v: Vec<(u64, u32, bool)> = s
            .found
            .video
            .iter()
            .map(|f| (f.pos, f.size, f.key))
            .collect();
        let v1_pos = 10 + v0.len() as u64 + 100;
        assert_eq!(
            v,
            [
                (10, v0.len() as u32, true),
                (v1_pos, v1.len() as u32, false)
            ]
        );
        let a: Vec<(u64, u32)> = s.found.audio.iter().map(|f| (f.pos, f.size)).collect();
        assert_eq!(a, [(10 + v0.len() as u64, 100)]); // the 30 bytes before the cut frame are too few
        assert_eq!(s.found.frames(), 3);
    }

    #[test]
    fn splits_audio_runs_at_repeating_frame_headers() {
        let style = VideoStyle::original();
        let frame = |seed: u8, len: usize| {
            let mut f = vec![0x21, 0x1a, 0x0f, 0xff];
            f.extend((0..len - 4).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)));
            f
        };
        let audio = [frame(1, 300), frame(2, 320), frame(3, 290)].concat();
        let v = |t: u8| [nal(&[9, 0x30], 2), nal(&[t, 0x88], 500)].concat();
        let data = [v(0x65), audio.clone(), v(0x41)].concat();
        let mut s = Scanner::new(Cursor::new(&data), data.len() as u64, &style, 300);
        s.scan(0, &Quiet).unwrap().unwrap();
        let start = v(0x65).len() as u64;
        let a: Vec<(u64, u32, u32)> = s
            .found
            .audio
            .iter()
            .map(|f| (f.pos, f.size, f.count))
            .collect();
        assert_eq!(
            a,
            [
                (start, 300, 1),
                (start + 300, 320, 1),
                (start + 620, 290, 1)
            ]
        );
        assert_eq!(s.found.audio_frames, 3);

        // without repeating headers the run counts as several frames
        let noise: Vec<u8> = (0..900u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8 | 1)
            .collect();
        let data = [v(0x65), noise, v(0x41)].concat();
        let mut s = Scanner::new(Cursor::new(&data), data.len() as u64, &style, 300);
        s.scan(0, &Quiet).unwrap().unwrap();
        assert_eq!(s.found.audio.len(), 1);
        assert_eq!(s.found.audio[0].count, 3); // trunc(900 / 300 - 0.7) = 2 extra
        assert_eq!(s.found.audio_frames, 3);
    }

    #[test]
    fn sixteen_byte_frame_header() {
        let style = VideoStyle::original();
        let framed = |t: u8, len: usize| {
            let mut v = vec![0u8; 8];
            v.extend_from_slice(&((len + 4) as u32).to_be_bytes());
            v.extend_from_slice(&[0; 4]);
            v.extend(nal(&[t, 0x88], len));
            v
        };
        let a = [vec![0u8; 16], vec![0x33u8; 80]].concat();
        let data = [framed(0x65, 2000), a, framed(0x41, 1500)].concat();
        let mut s = Scanner::new(Cursor::new(&data), data.len() as u64, &style, 0);
        s.scan(0, &Quiet).unwrap().unwrap();
        let v: Vec<(u64, u32)> = s.found.video.iter().map(|f| (f.pos, f.size)).collect();
        assert_eq!(v, [(16, 2004), (16 + 2004 + 96 + 16, 1504)]);
        // the audio run skips its own 16-byte header
        assert_eq!(
            s.found
                .audio
                .iter()
                .map(|f| (f.pos, f.size))
                .collect::<Vec<_>>(),
            [(16 + 2004 + 16, 80)]
        );
    }
}
