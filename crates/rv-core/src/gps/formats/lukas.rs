//! `UnitGps_Lukas`: Lukas logs. Every line is a 28-byte binary header followed by an
//! NMEA-like sentence with a checksum at byte 29: `$GA,HHMMSS.sss,status,lat,lon,speed,N|S,E|W*hh`
//! (fix; NMEA coordinates, speed in km/h, UTC time) or `$GC,?,DDMMYY,...*hh` (sets the date of
//! the following fixes; "010112" until the first one).
//!
//! The header holds the G-sensor sample of the line: little-endian f32 x, z, y in g at offsets
//! 16, 20, 24. It is read from every accepted line, with or without a valid sentence.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, Spec};
use crate::gps::util::{
    COORD_NMEA, CharSet, SPLIT_KEEP_EMPTY, copy, good_gps_checksum, gp_str_to_coordinate, gp_str_to_date_time,
    half_sphere_x, half_sphere_y, le_f32, split, str_to_float,
};

static SPEC: Spec = Spec { title: "Lukas", ..crate::gps::formats::EMPTY_SPEC };

#[derive(Default)]
pub struct Lukas {
    /// `lastDate`: "DDMMYY" of the last `$GC` sentence.
    last_date: Vec<u8>,
}

pub fn create() -> Box<dyn Format> {
    Box::new(Lukas::default())
}

impl Format for Lukas {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        line.len() > 36 && matches!(copy(line, 29, 4), b"$GA," | b"$GC,")
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {
        self.last_date = b"010112".to_vec();
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item, it2: &mut Item) -> (bool, bool) {
        let mut res1 = false;
        let cols = split(copy(line, 29, line.len().min(100)), &CharSet::of(b","), SPLIT_KEEP_EMPTY);
        if cols.len() > 7 && good_gps_checksum(&cols) {
            if cols[0] == b"$GA" {
                it.pt = PT_GPS;
                it.time = gp_str_to_date_time(&self.last_date, &cols[1], ctx.tz);
                if it.time > 2.0 {
                    it.x = gp_str_to_coordinate(&cols[4], COORD_NMEA, 0.0);
                    it.y = gp_str_to_coordinate(&cols[3], COORD_NMEA, 0.0);
                    if let Some(&c) = cols[7].first() {
                        half_sphere_x(&mut it.x, &[c]);
                    }
                    if let Some(&c) = cols[6].first() {
                        half_sphere_y(&mut it.y, &[c]);
                    }
                    it.speed = str_to_float(&cols[5], 0.0);
                    it.angle = -1.0;
                    if cols[2] != b"A" {
                        it.warn = 2;
                    }
                    res1 = true;
                }
            } else if cols[0] == b"$GC" {
                self.last_date = cols[2].clone();
            }
        }
        // The G-sensor sample of the binary header (the original reads it unchecked; accepted
        // lines are longer than 36 bytes).
        let (Some(x), Some(z), Some(y)) = (le_f32(line, 0x10), le_f32(line, 0x14), le_f32(line, 0x18)) else {
            return (res1, false);
        };
        *it2 = Item::gsensor(0.0, f64::from(x), f64::from(y), f64::from(z));
        (res1, true)
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    /// A record: the header with the G-sensor floats and the sentence with its checksum.
    fn rec(g: (f32, f32, f32), sentence: &str) -> Vec<u8> {
        let mut out = vec![0x11u8; 16];
        out.extend_from_slice(&g.0.to_le_bytes());
        out.extend_from_slice(&g.2.to_le_bytes());
        out.extend_from_slice(&g.1.to_le_bytes());
        let body = sentence.trim_start_matches('$');
        let x = body.bytes().fold(0u8, |a, c| a ^ c);
        out.extend_from_slice(format!("${body}*{x:02X}\r\n").as_bytes());
        out
    }

    fn log() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend(rec((0.0, 0.0, 0.0), "$GC,1,160516,2,3,4,5,6"));
        d.extend(rec((0.5, -0.25, 1.0), "$GA,130626.000,A,5530.1234,03730.1234,45.5,N,E"));
        d.extend(rec((0.5, -0.5, 1.0), "$GA,130627.000,A,5530.1334,03730.1434,46.5,S,W"));
        d.extend(rec((0.25, -0.5, 1.0), "$GA,130628.000,V,5530.1434,03730.1634,47.5,N,E"));
        d
    }

    #[test]
    fn fixes_and_header_gsensor() {
        let r = load_log("a.dat", &log()).unwrap();
        assert_eq!(r.format, "Lukas");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        assert_eq!(t.gps.len(), 3);
        let p = t.gps[0];
        assert!((p.lat - (55.0 + 30.1234 / 60.0)).abs() < 1e-9 && (p.lon - (37.0 + 30.1234 / 60.0)).abs() < 1e-9);
        assert_eq!((p.speed_kmh, p.warning), (45.5, 0));
        assert!(t.gps[1].lat < 0.0 && t.gps[1].lon < 0.0);
        assert_eq!((t.gps[2].time, t.gps[2].warning), (2.0, 2));
        let g = &t.gsensor;
        assert_eq!(g.len(), 3, "the $GC header has no sample");
        assert_eq!((g[0].x, g[0].y, g[0].z), (0.5, -0.25, 1.0));
    }

    #[test]
    fn date_defaults_and_checksum() {
        // before any $GC the date is 2012-01-01
        let mut d = rec((0.5, 0.0, 1.0), "$GA,130626.000,A,5530.1234,03730.1234,45.5,N,E");
        d.extend(rec((0.5, 0.0, 1.0), "$GA,130627.000,A,5530.1334,03730.1434,45.5,N,E"));
        let r = load_log("a.dat", &d).unwrap();
        assert_eq!(r.track.start.unwrap().format("%Y-%m-%d").to_string(), "2012-01-01");
        // a bad checksum drops the fix but keeps the header sample
        let mut bad = log();
        let star = bad.iter().rposition(|&c| c == b'*').unwrap();
        bad[star + 1] = b'0';
        bad[star + 2] = if bad[star + 2] == b'0' { b'1' } else { b'0' };
        let r = load_log("a.dat", &bad).unwrap();
        assert_eq!((r.track.gps.len(), r.track.gsensor.len()), (2, 3));
    }

    #[test]
    fn fuzz_lukas() {
        fuzz("a.dat", &log());
    }
}
