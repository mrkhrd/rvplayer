//! Video decoding for the player: MP4/MOV samples are read with `rv-core`, decoded by FilmCraft's
//! pure-Rust H.264 / H.265 decoders and converted to RGBA frames for display.

pub mod player;
mod yuv;

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{Duration, Instant};

use rv_core::mp4::{self, Sample};

pub use yuv::Matrix;

/// A decoded picture ready for display.
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Presentation time in seconds from the start of the video.
    pub time: f64,
    pub key: bool,
    /// Packed RGBA, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
}

/// Time spent per stage since the stream was opened.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    pub read: Duration,
    pub decode: Duration,
    pub convert: Duration,
    pub frames: u64,
}

enum Decoder {
    H264(Box<filmcraft_h264::Decoder>),
    Hevc(Box<filmcraft_hevc::Decoder>),
}

/// Microsecond timestamps carry presentation time through the decoders' reordering.
const PTS_PER_SECOND: f64 = 1_000_000.0;

pub struct VideoStream {
    file: BufReader<File>,
    samples: Vec<Sample>,
    config: Vec<u8>,
    decoder: Decoder,
    /// Next sample to feed to the decoder.
    next: usize,
    ready: VecDeque<Frame>,
    flushed: bool,
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub stats: Stats,
}

impl VideoStream {
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut file = BufReader::with_capacity(1 << 20, File::open(path)?);
        let movie = mp4::read_movie(&mut file)?;
        let track = movie
            .track_by_handler(b"vide")
            .ok_or_else(|| invalid("no video track"))?;
        let codec = match &track.codec {
            b"avc1" | b"avc3" => Codec::H264,
            b"hvc1" | b"hev1" => Codec::Hevc,
            other => {
                return Err(invalid(&format!(
                    "unsupported codec {}",
                    String::from_utf8_lossy(other)
                )));
            }
        };
        let config = track
            .codec_config
            .clone()
            .ok_or_else(|| invalid("video track has no decoder configuration"))?;
        let decoder = new_decoder(codec, &config)?;
        Ok(Self {
            file,
            samples: track.samples(),
            config,
            decoder,
            next: 0,
            ready: VecDeque::new(),
            flushed: false,
            codec,
            width: track.width.round() as u32,
            height: track.height.round() as u32,
            duration: track.duration_secs(),
            stats: Stats::default(),
        })
    }

    /// Average frame rate from the sample count.
    pub fn frame_rate(&self) -> f64 {
        if self.duration > 0.0 {
            self.samples.len() as f64 / self.duration
        } else {
            0.0
        }
    }

    /// Next frame in presentation order, or `None` at the end of the stream.
    pub fn next_frame(&mut self) -> io::Result<Option<Frame>> {
        loop {
            if let Some(f) = self.ready.pop_front() {
                return Ok(Some(f));
            }
            if self.next < self.samples.len() {
                self.feed_next()?;
            } else if !self.flushed {
                self.flushed = true;
                let t = Instant::now();
                let pics = match &mut self.decoder {
                    Decoder::H264(d) => {
                        d.flush().into_iter().map(Picture::H264).collect::<Vec<_>>()
                    }
                    Decoder::Hevc(d) => d.flush().into_iter().map(Picture::Hevc).collect(),
                };
                self.stats.decode += t.elapsed();
                self.convert(pics);
            } else {
                return Ok(None);
            }
        }
    }

    /// Positions the stream so the next frame is the first one at or after `time`.
    /// Decoding restarts from the preceding key frame; frames before `time` are dropped.
    pub fn seek(&mut self, time: f64) -> io::Result<Option<Frame>> {
        let target = self
            .samples
            .iter()
            .rposition(|s| s.sync && s.time <= time + 1e-6)
            .unwrap_or(0);
        self.decoder = new_decoder(self.codec, &self.config)?;
        self.next = target;
        self.ready.clear();
        self.flushed = false;
        while let Some(f) = self.next_frame()? {
            if f.time + 1e-6 >= time {
                return Ok(Some(f));
            }
        }
        Ok(None)
    }

    fn feed_next(&mut self) -> io::Result<()> {
        let s = self.samples[self.next];
        self.next += 1;
        let t = Instant::now();
        let mut data = vec![0u8; s.size as usize];
        self.file.seek(SeekFrom::Start(s.offset))?;
        self.file.read_exact(&mut data)?;
        self.stats.read += t.elapsed();

        let t = Instant::now();
        let pts = (s.time * PTS_PER_SECOND).round() as i64;
        let pics = match &mut self.decoder {
            Decoder::H264(d) => d
                .decode(&data, pts)
                .map_err(decode_error)?
                .into_iter()
                .map(Picture::H264)
                .collect::<Vec<_>>(),
            Decoder::Hevc(d) => d
                .decode(&data, pts)
                .map_err(decode_error)?
                .into_iter()
                .map(Picture::Hevc)
                .collect(),
        };
        self.stats.decode += t.elapsed();
        self.convert(pics);
        Ok(())
    }

    fn convert(&mut self, pics: Vec<Picture>) {
        for p in pics {
            let t = Instant::now();
            let frame = p.to_frame(self.height);
            self.stats.convert += t.elapsed();
            self.stats.frames += 1;
            self.ready.push_back(frame);
        }
    }
}

enum Picture {
    H264(filmcraft_h264::Picture),
    Hevc(filmcraft_hevc::Picture),
}

impl Picture {
    fn to_frame(&self, track_height: u32) -> Frame {
        match self {
            Picture::H264(p) => {
                let m = Matrix::from_h273(p.color.matrix, track_height);
                let planes = yuv::Planes {
                    y: &p.y,
                    u: &p.u,
                    v: &p.v,
                    y_stride: p.y_stride,
                    uv_stride: p.uv_stride,
                };
                Frame {
                    width: p.width,
                    height: p.height,
                    time: p.pts as f64 / PTS_PER_SECOND,
                    key: p.key,
                    rgba: yuv::to_rgba(&planes, p.width, p.height, m, p.color.full_range),
                }
            }
            Picture::Hevc(p) => {
                let m = Matrix::from_h273(p.color.matrix, track_height);
                let shift = p.bit_depth.saturating_sub(8);
                let (y, u, v) = (to_u8(&p.y, shift), to_u8(&p.u, shift), to_u8(&p.v, shift));
                let planes = yuv::Planes {
                    y: &y,
                    u: &u,
                    v: &v,
                    y_stride: p.y_stride,
                    uv_stride: p.uv_stride,
                };
                Frame {
                    width: p.width,
                    height: p.height,
                    time: p.pts as f64 / PTS_PER_SECOND,
                    key: p.key,
                    rgba: yuv::to_rgba(&planes, p.width, p.height, m, p.color.full_range),
                }
            }
        }
    }
}

/// HEVC planes as 8-bit samples (high bit depths are shifted down for display).
fn to_u8(plane: &filmcraft_hevc::Plane, shift: u32) -> std::borrow::Cow<'_, [u8]> {
    match plane {
        filmcraft_hevc::Plane::U8(v) => std::borrow::Cow::Borrowed(v),
        filmcraft_hevc::Plane::U16(v) => {
            std::borrow::Cow::Owned(v.iter().map(|&s| (s >> shift).min(255) as u8).collect())
        }
    }
}

fn new_decoder(codec: Codec, config: &[u8]) -> io::Result<Decoder> {
    Ok(match codec {
        Codec::H264 => Decoder::H264(Box::new(
            filmcraft_h264::Decoder::from_avcc(config).map_err(decode_error)?,
        )),
        Codec::Hevc => Decoder::Hevc(Box::new(
            filmcraft_hevc::Decoder::from_hvcc(config).map_err(decode_error)?,
        )),
    })
}

fn decode_error(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("decode: {e}"))
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}
