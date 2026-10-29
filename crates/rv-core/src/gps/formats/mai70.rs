//! 70mai dashcams (A810 and others of the Xiaomi family): GPS in the Novatek-style `freeGPS`
//! blocks the `gps ` index of `moov` lists, one block per video frame. Not a format of the
//! original; decoded from the cameras' files. Payload (after the 16-byte block header),
//! little-endian:
//!
//! | offset | field |
//! |---|---|
//! | 0 | u16 layout version (3) |
//! | 2 | u32 payload length |
//! | 6, 10 | u32 video width, height |
//! | 14 | status 'A' (fix) / 'V' (no fix: the rest is zero) |
//! | 15, 19 | i32 latitude, longitude as NMEA DDMM.MMMMM x 100000 (no hemisphere flag; a negative value is taken as S / W) |
//! | 23 | u32 course (degrees) |
//! | 27 | u32 speed (km/h) |
//!
//! The GPS fields change about once a second, so the block is the same for runs of frames:
//! a run gives one point, at its first frame. The blocks carry no time of day; points are
//! placed by their frame times from the file's own times. After power-on the camera repeats
//! the last fix it had, speed included, until the receiver has a new one (about half a
//! minute): a fix that stays the same for longer than [`STALE`] while moving is marked
//! uncertain, which keeps it off the route.

use super::dod::LINE_PREFIX;
use crate::gps::format::{Ctx, DataTest, Format, Item, Spec, default_test_load_item};
use crate::gps::util::{SEC, le_i32, le_u16, le_u32};

static SPEC: Spec = Spec { title: "70mai", format_id: "70mai", line_prefix: LINE_PREFIX, ..super::EMPTY_SPEC };

/// A fix unchanged for longer than this while moving is a repeated old one (the receiver
/// updates about once a second, with gaps of a few seconds).
const STALE: f64 = 10.0 * SEC;
/// Speed (km/h) from which an unchanged fix counts as moving.
const MOVING: f64 = 5.0;

/// What a block says: `None` inside when it has no fix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fix {
    pub lat: f64,
    pub lon: f64,
    pub course: f64,
    pub speed_kmh: f64,
}

/// `DDMM.MMMMM x 100000` to degrees; `None` when the minutes are 60 or more.
fn coordinate(v: i32) -> Option<f64> {
    let (sign, v) = if v < 0 { (-1.0, -i64::from(v)) } else { (1.0, i64::from(v)) };
    let (deg, min) = (v / 10_000_000, (v % 10_000_000) as f64 / 100_000.0);
    (min < 60.0).then(|| sign * (deg as f64 + min / 60.0))
}

/// Decodes a block payload; `None` unless it has this layout, `Some(None)` for "no fix".
pub fn decode(p: &[u8]) -> Option<Option<Fix>> {
    let (version, len, width, height) = (le_u16(p, 0)?, le_u32(p, 2)?, le_u32(p, 6)?, le_u32(p, 10)?);
    if version != 3 || !(31..=0x10000).contains(&len) || !(1..=16384).contains(&width) || !(1..=16384).contains(&height) {
        return None;
    }
    match *p.get(14)? {
        b'V' => Some(None),
        b'A' => {
            let (lat, lon) = (coordinate(le_i32(p, 15)?)?, coordinate(le_i32(p, 19)?)?);
            let (course, speed) = (le_u32(p, 23)?, le_u32(p, 27)?);
            if lat.abs() > 90.0 || lon.abs() > 180.0 || course > 360 || speed > 500 {
                return None;
            }
            Some(Some(Fix { lat, lon, course: f64::from(course), speed_kmh: f64::from(speed) }))
        }
        _ => None,
    }
}

#[derive(Default)]
pub struct Mai70 {
    /// The fix of the current run of identical blocks, the index of its point and its first frame.
    run: Option<(Fix, usize, f64)>,
}

impl Mai70 {
    /// Ends the current run at frame time `end`: a fix held too long while moving was an old
    /// one. Runs are judged when they end, as identical records may not reach the format.
    fn end_run(&mut self, ctx: &mut Ctx, end: f64) {
        if let Some((fix, at, since)) = self.run.take()
            && fix.speed_kmh >= MOVING
            && end - since > STALE
            && let Some(p) = ctx.points.get_mut(at)
        {
            p.warn = 2;
        }
    }
}

pub fn create() -> Box<dyn Format> {
    Box::<Mai70>::default()
}

impl Format for Mai70 {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, _data: &[u8]) -> DataTest {
        DataTest::No
    }

    /// A run still going at the last block ends with the video.
    fn after_load_data(&mut self, ctx: &mut Ctx) {
        let end = ctx.items.iter().map(|i| i.frame_time).fold(ctx.duration, f64::max);
        self.end_run(ctx, end);
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        default_test_load_item(&SPEC, line) && decode(&line[LINE_PREFIX.len()..]).is_some()
    }

    /// Every block of this layout is taken (no fix and repeats too), so that no other format
    /// reads it; only the first block of a run adds a point.
    fn load_item(&mut self, ctx: &mut Ctx, line: &[u8]) -> bool {
        let Some(decoded) = line.get(LINE_PREFIX.len()..).and_then(decode) else { return false };
        let frame = ctx.frame_time();
        let Some(fix) = decoded else {
            self.end_run(ctx, frame);
            return true;
        };
        if self.run.is_some_and(|(same, ..)| same == fix) {
            return true;
        }
        self.end_run(ctx, frame);
        let mut it = Item::gps(-1.0, fix.lon, fix.lat);
        it.angle = fix.course;
        it.speed = fix.speed_kmh;
        if it.is_valid() {
            ctx.last_gps_index = Some(ctx.points.len());
            self.run = Some((fix, ctx.points.len(), frame));
            ctx.points.push(it);
        }
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A whole `freeGPS` block (header and the 493-byte payload of the A810) with a fix, or
    /// without one when `fix` is `None`.
    pub fn block(fix: Option<(i32, i32, u32, u32)>) -> Vec<u8> {
        let mut a = vec![0u8; 16 + 493];
        a[0..4].copy_from_slice(&0x4000u32.to_be_bytes());
        a[4..8].copy_from_slice(b"free");
        a[8..12].copy_from_slice(b"GPS ");
        a[12..16].copy_from_slice(&493u32.to_le_bytes());
        let p = &mut a[16..];
        p[0..2].copy_from_slice(&3u16.to_le_bytes());
        p[2..6].copy_from_slice(&493u32.to_le_bytes());
        p[6..10].copy_from_slice(&3840u32.to_le_bytes());
        p[10..14].copy_from_slice(&2160u32.to_le_bytes());
        match fix {
            Some((lat, lon, course, speed)) => {
                p[14] = b'A';
                p[15..19].copy_from_slice(&lat.to_le_bytes());
                p[19..23].copy_from_slice(&lon.to_le_bytes());
                p[23..27].copy_from_slice(&course.to_le_bytes());
                p[27..31].copy_from_slice(&speed.to_le_bytes());
            }
            None => p[14] = b'V',
        }
        a
    }

    #[test]
    fn decodes_the_a810_layout() {
        // From NO20261024-154056-012644F.MP4: 60 deg 06.95419 min N, 31 deg 22.31780 min E.
        let b = block(Some((600_695_419, 312_231_780, 259, 125)));
        let f = decode(&b[16..]).unwrap().unwrap();
        assert!((f.lat - 60.115_903).abs() < 1e-6, "{}", f.lat);
        assert!((f.lon - 31.371_963).abs() < 1e-6, "{}", f.lon);
        assert_eq!((f.course, f.speed_kmh), (259.0, 125.0));
        assert_eq!(decode(&block(None)[16..]), Some(None));
        // Minutes of 60 or more are not a coordinate; another layout version is not this format.
        assert_eq!(decode(&block(Some((596_011_430, 302_053_045, 0, 0)))[16..]), None);
        let mut other = block(None);
        other[16] = 2;
        assert_eq!(decode(&other[16..]), None);
    }
}
