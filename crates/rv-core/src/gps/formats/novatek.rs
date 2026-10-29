//! Novatek GPS blocks in the layout most Novatek dashcams use (VIOFO A119 V3/A129 and many
//! others; documented by ExifTool `QuickTimeStream.pl` and `nvtk_mp42gpx`). Not a format of
//! the original, which only knew the older DOD layouts ([`super::dod`]); records come from the
//! same `free`/"GPS " blocks ("MyDoDprfx" + atom from byte 16), so this format is tried first
//! and only accepts blocks that match its layout exactly:
//!
//! | atom offset | field |
//! |---|---|
//! | 16..40 | u32 LE hour, minute, second, year (since 2000, or full), month, day (UTC) |
//! | 40 | status 'A' (fix) / 'V' (no fix) |
//! | 41, 42 | 'N'/'S', 'E'/'W' |
//! | 44, 48 | f32 LE latitude, longitude as NMEA DDMM.MMMM |
//! | 52, 56 | f32 LE speed (knots), bearing (degrees) |

use super::dod::LINE_PREFIX;
use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, Spec, default_test_load_item};
use crate::gps::util::{KNOTS_TO_KMH, encode_date_time, gp_int_to_coordinate, le_f32, le_u32};

static SPEC: Spec = Spec { title: "Novatek", format_id: "Novatek", line_prefix: LINE_PREFIX, ..super::EMPTY_SPEC };

pub struct Novatek;

pub fn create() -> Box<dyn Format> {
    Box::new(Novatek)
}

/// A fix decoded from the standard layout; `p` is the atom from byte 16.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fix {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub active: bool,
    pub lat: f64,
    pub lon: f64,
    pub speed_kmh: f64,
    pub bearing: f64,
}

/// Decodes the standard layout, `None` unless every field is plausible.
pub fn decode(p: &[u8]) -> Option<Fix> {
    let u = |at: usize| le_u32(p, at);
    let (hour, minute, second, year, month, day) = (u(0)?, u(4)?, u(8)?, u(12)?, u(16)?, u(20)?);
    let year = match year {
        0..=99 => 2000 + year as i32,
        2000..=2099 => year as i32,
        _ => return None,
    };
    if hour > 23 || minute > 59 || second > 59 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let (status, ns, ew) = (*p.get(24)?, *p.get(25)?, *p.get(26)?);
    if !matches!(status, b'A' | b'V') || !matches!(ns, b'N' | b'S') || !matches!(ew, b'E' | b'W') {
        return None;
    }
    let (lat, lon, speed, bearing) = (le_f32(p, 28)?, le_f32(p, 32)?, le_f32(p, 36)?, le_f32(p, 40)?);
    let ok = |v: f32, max: f32| v.is_finite() && (0.0..=max).contains(&v);
    if !ok(lat, 9000.0) || !ok(lon, 18000.0) || !speed.is_finite() || !bearing.is_finite() {
        return None;
    }
    let sign = |c: u8, neg: u8| if c == neg { -1.0 } else { 1.0 };
    Some(Fix {
        year,
        month,
        day,
        hour,
        minute,
        second,
        active: status == b'A',
        lat: sign(ns, b'S') * gp_int_to_coordinate(f64::from(lat)),
        lon: sign(ew, b'W') * gp_int_to_coordinate(f64::from(lon)),
        speed_kmh: f64::from(speed.max(0.0)) * KNOTS_TO_KMH,
        bearing: f64::from(bearing),
    })
}

impl Format for Novatek {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, _data: &[u8]) -> DataTest {
        DataTest::No
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        default_test_load_item(&SPEC, line) && decode(&line[LINE_PREFIX.len()..]).is_some()
    }

    fn load_item1(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let Some(f) = line.get(LINE_PREFIX.len()..).and_then(decode) else { return false };
        let Some(t) = encode_date_time(f.year, f.month, f.day, f.hour, f.minute, f.second, 0) else { return false };
        *it = Item {
            pt: PT_GPS,
            time: t + ctx.tz,
            x: f.lon,
            y: f.lat,
            z: 0.0,
            angle: f.bearing,
            speed: f.speed_kmh,
            warn: if f.active { 0 } else { 2 },
        };
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A standard Novatek `free` atom (whole atom, 0x4c + 16 bytes).
    #[allow(clippy::too_many_arguments)]
    pub fn atom(
        h: u32,
        m: u32,
        s: u32,
        status: u8,
        lat: f32,
        ns: u8,
        lon: f32,
        ew: u8,
        knots: f32,
        bearing: f32,
    ) -> Vec<u8> {
        let mut a = vec![0u8; 16 + 0x4c];
        let n = a.len() as u32;
        a[0..4].copy_from_slice(&n.to_be_bytes());
        a[4..8].copy_from_slice(b"free");
        a[8..12].copy_from_slice(b"GPS ");
        a[12..16].copy_from_slice(&0x4cu32.to_le_bytes());
        for (k, v) in [h, m, s, 16, 5, 16].into_iter().enumerate() {
            a[16 + 4 * k..20 + 4 * k].copy_from_slice(&v.to_le_bytes());
        }
        a[40] = status;
        a[41] = ns;
        a[42] = ew;
        a[44..48].copy_from_slice(&lat.to_le_bytes());
        a[48..52].copy_from_slice(&lon.to_le_bytes());
        a[52..56].copy_from_slice(&knots.to_le_bytes());
        a[56..60].copy_from_slice(&bearing.to_le_bytes());
        a
    }

    #[test]
    fn decodes_the_standard_layout() {
        let a = atom(13, 6, 26, b'A', 6003.8176, b'N', 3019.6213, b'W', 10.0, 45.0);
        let f = decode(&a[16..]).unwrap();
        assert_eq!((f.year, f.month, f.day, f.hour, f.minute, f.second), (2016, 5, 16, 13, 6, 26));
        assert!((f.lat - 60.063_626).abs() < 1e-5 && (f.lon + 30.327_021).abs() < 1e-5);
        assert!((f.speed_kmh - 18.5325).abs() < 1e-9);
        let mut bad = a.clone();
        bad[40] = b'X';
        assert!(decode(&bad[16..]).is_none());
        assert!(decode(&a[16..40]).is_none());
        for n in 0..a.len() - 16 {
            let _ = decode(&a[16..16 + n]);
        }
    }
}
