//! `UnitGps_LogDVR`: "GPS Log (DVR DS103, GPS100)". Text lines of fields separated by ',' or
//! ';': `status,DDMMYY,HHMMSS,lat,N|S,lon,E|W[,speed[,...]]` with ',' at the 2nd and 9th
//! character. Time is UTC (plus the local zone offset); status other than "A" marks the fix.
//!
//! Two layouts:
//! - 12 fields whose 9th field ends with a digit ("version 2"): coordinates in degrees or NMEA
//!   (detected), speed in km/h, course in the 9th field, G-sensor x, z, y in g (fields 10-12);
//! - otherwise: NMEA coordinates, speed in knots, no course; with 11 or more fields the last
//!   three are G-sensor x, y, z in units of 1/6.72 g.
//!
//! A hemisphere other than "N" / "E" negates a non-zero coordinate.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::util::{
    COORD_AUTO, COORD_NMEA, CharSet, DIGITS, KNOTS_TO_KMH, SPLIT_KEEP_EMPTY, gp_str_to_coordinate, gp_str_to_date_time,
    split, str_to_float,
};

static SPEC: Spec = Spec {
    title: "GPS Log (DVR DS103, GPS100)",
    def_ext: "log",
    format_id: "logDVR",
    must_be_chars_in_data: CharSet::of(b",.;"),
    bad_chars: CharSet::ranges(&[
        (0, 0),
        (b'\t', b'\t'),
        (b'$', b'$'),
        (b':', b':'),
        (b'<', b'<'),
        (b'>', b'>'),
        (b'[', b'['),
        (b']', b']'),
        (b'a', b'z'),
    ]),
    ..crate::gps::formats::EMPTY_SPEC
};

/// G-sensor units per g of the first layout.
const G_UNITS: f64 = 6.72;

pub struct LogDvr;

pub fn create() -> Box<dyn Format> {
    Box::new(LogDvr)
}

impl Format for LogDvr {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item, it2: &mut Item) -> (bool, bool) {
        if line.len() < 11 || line[1] != b',' || line[8] != b',' {
            return (false, false);
        }
        let cols = split(line, &CharSet::of(b",;"), SPLIT_KEEP_EMPTY);
        let n = cols.len();
        let col = |i: usize| cols.get(i).map_or(&[][..], |c| c.as_slice());
        let ver2 = n == 12 && col(8).last().is_some_and(|&c| DIGITS.contains(c));
        let coord_fmt = if ver2 { COORD_AUTO } else { COORD_NMEA };
        let mut res1 = false;
        let mut res2 = false;
        if n > 6 {
            it.pt = PT_GPS;
            it.time = gp_str_to_date_time(col(1), col(2), ctx.tz);
            it.y = gp_str_to_coordinate(col(3), coord_fmt, 0.0);
            if it.y != 0.0 && col(4) != b"N" {
                it.y = -it.y;
            }
            it.x = gp_str_to_coordinate(col(5), coord_fmt, 0.0);
            if it.x != 0.0 && col(6) != b"E" {
                it.x = -it.x;
            }
            it.z = 0.0;
            if n < 8 {
                it.speed = -1.0;
            } else {
                it.speed = str_to_float(col(7), -1.0);
                if it.speed > 0.0 && !ver2 {
                    it.speed *= KNOTS_TO_KMH;
                }
            }
            if col(0) != b"A" {
                it.warn = 2;
            }
            it.angle = if ver2 { str_to_float(col(8), -1.0) } else { -1.0 };
            res1 = true;
        }
        if n > 10 {
            it2.pt = PT_GSENSOR;
            it2.time = 0.0;
            if ver2 {
                it2.x = str_to_float(col(9), 0.0);
                it2.z = str_to_float(col(10), 0.0);
                it2.y = str_to_float(col(11), 0.0);
            } else {
                it2.x = str_to_float(col(n - 3), 0.0) / G_UNITS;
                it2.y = str_to_float(col(n - 2), 0.0) / G_UNITS;
                it2.z = str_to_float(col(n - 1), 0.0) / G_UNITS;
            }
            res2 = true;
        }
        (res1, res2)
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    const LOG: &str = "\
A,160516,130626,5530.1234,N,03730.1234,E,10.0;6.72;-3.36;13.44
A,160516,130627,5530.1334,N,03730.1434,E,10.5;0;0;6.72
V,160516,130628,5530.1434,S,03730.1634,W,0.0
A,160516
";

    #[test]
    fn nmea_layout_with_knots_and_gsensor() {
        let r = load_log("trip.log", LOG.as_bytes()).unwrap();
        assert_eq!(r.format, "logDVR");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        assert_eq!(t.gps.len(), 3);
        let p = t.gps[0];
        assert!((p.lat - (55.0 + 30.1234 / 60.0)).abs() < 1e-9 && (p.lon - (37.0 + 30.1234 / 60.0)).abs() < 1e-9);
        assert!((p.speed_kmh - 10.0 * 1.85325).abs() < 1e-9);
        assert_eq!((t.gps[1].time, t.gps[2].time), (1.0, 2.0));
        // S / W negate, V warns
        let p = t.gps[2];
        assert!(p.lat < -55.5 && p.lon < -37.5);
        assert_eq!(p.warning, 2);
        assert_eq!(p.speed_kmh, 0.0);
        let g = &r.track.gsensor;
        assert_eq!(g.len(), 2);
        assert!((g[0].x - 1.0).abs() < 1e-12 && (g[0].y + 0.5).abs() < 1e-12 && (g[0].z - 2.0).abs() < 1e-12);
        assert_eq!((g[1].x, g[1].y), (0.0, 0.0));
        assert!((g[1].z - 1.0).abs() < 1e-12);
    }

    #[test]
    fn version_2_layout() {
        let log = "A,160516,130626,55.502057,N,37.502057,E,45.5,112.5,0.01,0.98,-0.02\n\
                   A,160516,130627,55.502157,N,37.502157,E,46.5,113.5;0.02;0.97;-0.03\n";
        let r = load_log("trip.log", log.as_bytes()).unwrap();
        assert_eq!(r.format, "logDVR");
        let p = r.track.gps[0];
        assert!((p.lat - 55.502057).abs() < 1e-12 && (p.lon - 37.502057).abs() < 1e-12);
        assert_eq!((p.speed_kmh, p.course), (45.5, 112.5));
        let g = &r.track.gsensor;
        assert_eq!(g.len(), 2);
        // fields 10, 11, 12 are x, z, y
        assert_eq!((g[0].x, g[0].y, g[0].z), (0.01, -0.02, 0.98));
    }

    #[test]
    fn detection_needs_semicolon_and_no_letters() {
        let no_semicolon =
            "A,160516,130626,5530.1234,N,03730.1234,E,10.0\nA,160516,130627,5530.1334,N,03730.1434,E,10.5\n";
        assert!(load_log("a.log", no_semicolon.as_bytes()).is_err());
        let lower = LOG.replace("V,", "v,");
        assert!(load_log("a.log", lower.as_bytes()).is_err());
    }

    #[test]
    fn fuzz_logdvr() {
        fuzz("a.log", LOG.as_bytes());
    }
}
