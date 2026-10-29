//! `UnitGps_LogDOD`: "GPS Log (DOD GS600, Carcam Q3)". Tab-separated lines
//! `YYYY-MM-DD hh:mm:ss[.fff]\tN|S<lat>\tE|W<lon>[\talt[\tspeed[\tcourse]]]`: local time,
//! degrees, metres, km/h. A coordinate whose first character is not the positive hemisphere
//! letter is negative.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, Spec};
use crate::gps::formats::datdod::str_position_to_float;
use crate::gps::util::{CharSet, SPLIT_KEEP_EMPTY, split, str_date_time_to_date_time, str_to_float};

static SPEC: Spec = Spec {
    title: "GPS Log (DOD GS600, Carcam Q3)",
    def_ext: "log",
    format_id: "log600",
    must_be_chars_in_data: CharSet::of(b"\t-."),
    bad_chars: CharSet::ranges(&[
        (0, 0),
        (b',', b','),
        (b';', b';'),
        (b'<', b'<'),
        (b'>', b'>'),
        (b'[', b'['),
        (b']', b']'),
        (b'a', b'z'),
    ]),
    ..crate::gps::formats::EMPTY_SPEC
};

pub struct LogDod;

pub fn create() -> Box<dyn Format> {
    Box::new(LogDod)
}

impl Format for LogDod {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn load_item1(&mut self, _ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let cols = split(line, &CharSet::of(b"\t"), SPLIT_KEEP_EMPTY);
        let n = cols.len();
        if n <= 2 {
            return false;
        }
        it.pt = PT_GPS;
        it.time = str_date_time_to_date_time(&cols[0]);
        it.y = str_position_to_float(&cols[1], b'N', 0.0);
        it.x = str_position_to_float(&cols[2], b'E', 0.0);
        if n > 3 {
            it.z = str_to_float(&cols[3], 0.0);
        }
        it.speed = if n < 5 { -1.0 } else { str_to_float(&cols[4], -1.0) };
        it.angle = if n < 6 { -1.0 } else { str_to_float(&cols[5], -1.0) };
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    const LOG: &str = "\
2016-05-16 13:06:26\tN55.502057\tE37.502057\t150.5\t45.5\t90.5
2016-05-16 13:06:27\tS55.502157\tW37.502157\t151.5\t46.5
2016-05-16 13:06:28\tN55.502257\tE37.502257\t152.5\tx\t-
2016-05-16 13:06:29\tN55.502357\tE37.502357
";

    #[test]
    fn fixes() {
        let r = load_log("a.log", LOG.replace("\tx\t-", "\t\t").as_bytes()).unwrap();
        assert_eq!(r.format, "log600");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        assert_eq!(t.gps.len(), 4);
        let p = t.gps[0];
        assert_eq!((p.lat, p.lon, p.altitude, p.speed_kmh, p.course), (55.502057, 37.502057, 150.5, 45.5, 90.5));
        let p = t.gps[1];
        assert_eq!((p.time, p.lat, p.lon, p.speed_kmh), (1.0, -55.502157, -37.502157, 46.5));
        // empty speed: unknown, computed from the distance
        assert!(t.gps[2].speed_kmh > 0.0);
        // no altitude: the neighbour's
        assert_eq!(t.gps[3].altitude, 152.5);
    }

    #[test]
    fn detection_rules() {
        // lower-case letters and brackets are bad characters
        assert!(load_log("a.log", LOG.as_bytes()).is_err());
        assert!(load_log("a.log", LOG.replace("\tx\t-", "\t[1]\t").as_bytes()).is_err());
    }

    #[test]
    fn fuzz_logdod() {
        fuzz("a.log", LOG.replace("\tx\t-", "\t\t").as_bytes());
    }
}
