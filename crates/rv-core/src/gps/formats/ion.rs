//! `UnitGps_Ion`: iON Adventure logs. Lines start with "\0iON Adventure" and one more
//! character, then comma-separated fields:
//! `YYYY-MM-DD,hh:mm:ss,lat,N|S,lon,E|W,speed,course,alt[,gx,gz,gy]`. The time is local, the
//! coordinates are degrees, speed km/h (unknown when empty), course an integer (0 = unknown).
//! The optional G-sensor fields are x, z, y in 1/24 g.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::util::{
    CharSet, SPLIT_KEEP_EMPTY, copy, half_sphere_x, half_sphere_y, split, str_date_time_to_date_time, str_to_float,
    str_to_int, trim,
};

/// `magic`: the line prefix.
const MAGIC: &[u8] = b"\0iON Adventure";

static SPEC: Spec = Spec {
    title: "Ion",
    line_prefix: MAGIC,
    must_be_chars_in_line: CharSet::of(b",."),
    ..crate::gps::formats::EMPTY_SPEC
};

/// G-sensor units per g.
const G_UNITS: f64 = 24.0;

pub struct Ion;

pub fn create() -> Box<dyn Format> {
    Box::new(Ion)
}

impl Format for Ion {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn load_item2(&mut self, _ctx: &mut Ctx, line: &[u8], it: &mut Item, it2: &mut Item) -> (bool, bool) {
        let cols = split(copy(line, MAGIC.len() + 2, line.len()), &CharSet::of(b","), SPLIT_KEEP_EMPTY);
        let n = cols.len();
        let mut res1 = false;
        if n > 8 {
            // An empty hemisphere field is a range error in the original: the record fails.
            let (Some(&ns), Some(&ew)) = (cols[3].first(), cols[5].first()) else {
                return (false, false);
            };
            it.pt = PT_GPS;
            it.time = str_date_time_to_date_time(&[&cols[0][..], b" ", &cols[1][..]].concat());
            it.y = str_to_float(&cols[2], 0.0);
            half_sphere_y(&mut it.y, &[ns]);
            it.x = str_to_float(&cols[4], 0.0);
            half_sphere_x(&mut it.x, &[ew]);
            it.speed = str_to_float(&cols[6], -1.0);
            it.angle = f64::from(str_to_int(trim(&cols[7]), -1));
            if it.angle == 0.0 {
                it.angle = -1.0;
            }
            it.z = str_to_float(&cols[8], 0.0);
            res1 = true;
        }
        if n > 11 {
            let g = |i: usize| str_to_float(&cols[i], 0.0) / G_UNITS;
            *it2 = Item { pt: PT_GSENSOR, time: 0.0, x: g(9), z: g(10), y: g(11), ..*it2 };
            return (res1, true);
        }
        (res1, false)
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    fn log() -> Vec<u8> {
        let lines = [
            "2016-05-16,13:06:26,55.502057,N,37.502057,E,45.5,90,150.5,24,-12,6",
            "2016-05-16,13:06:27,55.502157,S,37.502157,W,,0,151.5,12,24,-6",
            "2016-05-16,13:06:28,55.502257,N,37.502257,E,47.5, 92 ,152.5",
            "2016-05-16,13:06:29,55.502357,,37.502357,E,47.5,92,152.5",
        ];
        let mut d = Vec::new();
        for l in lines {
            d.extend_from_slice(b"\0iON Adventure,");
            d.extend_from_slice(l.as_bytes());
            d.extend_from_slice(b"\r\n");
        }
        d
    }

    #[test]
    fn local_time_degrees_and_gsensor() {
        let r = load_log("ion.txt", &log()).unwrap();
        assert_eq!(r.format, "Ion");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        assert_eq!(t.gps.len(), 3, "the record with an empty hemisphere fails");
        let p = t.gps[0];
        assert_eq!((p.lat, p.lon, p.speed_kmh, p.course, p.altitude), (55.502057, 37.502057, 45.5, 90.0, 150.5));
        let p = t.gps[1];
        assert_eq!((p.time, p.lat, p.lon, p.altitude), (1.0, -55.502157, -37.502157, 151.5));
        assert!(p.speed_kmh > 0.0, "unknown speed is computed from the distance");
        assert_eq!(t.gps[2].course, 92.0);
        let g = &t.gsensor;
        assert_eq!(g.len(), 2);
        // fields 10, 11, 12 are x, z, y
        assert_eq!((g[0].x, g[0].y, g[0].z), (1.0, 0.25, -0.5));
        assert_eq!((g[1].x, g[1].y, g[1].z), (0.5, -0.25, 1.0));
    }

    #[test]
    fn needs_the_prefix() {
        let d: Vec<u8> = log().into_iter().filter(|&c| c != 0).collect();
        assert!(load_log("ion.txt", &d).is_err());
    }

    #[test]
    fn fuzz_ion() {
        fuzz("ion.txt", &log());
    }
}
