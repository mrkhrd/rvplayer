//! `UnitGps_DatDOD`: DOD GS600 / Carcam Q3 ".dat" files. Tab-separated lines:
//! `[G]\tYYYY-MM-DD hh:mm:ss[.fff]\tN|S<lat>\tE|W<lon>[\tspeed[\tcourse]]` (local time, degrees,
//! km/h) and `[S]\tx\ty\tz` (G-sensor in 1/1000 g); `[B]` lines are accepted but carry nothing.
//! A coordinate whose first character is not the positive hemisphere letter is negative.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, Spec, default_test_load_item};
use crate::gps::util::{CharSet, SPLIT_KEEP_EMPTY, split, str_date_time_to_date_time, str_to_float};

static SPEC: Spec = Spec {
    title: "GS600, Q3",
    def_ext: "dat",
    format_id: "dat600",
    must_be_chars_in_data: CharSet::of(b"\t[]"),
    bad_chars: CharSet::ranges(&[(0, 0), (b',', b','), (b';', b';'), (b'<', b'<'), (b'>', b'>'), (b'a', b'z')]),
    ..crate::gps::formats::EMPTY_SPEC
};

pub struct DatDod;

pub fn create() -> Box<dyn Format> {
    Box::new(DatDod)
}

/// `gmStrPositionToFloat` (UnitUtilsMap): the first character becomes '+' when it is
/// `positive` and '-' otherwise, then the text is read as a number; `def` when shorter than
/// 3 characters or not a number.
pub(crate) fn str_position_to_float(s: &[u8], positive: u8, def: f64) -> f64 {
    let Some((&first, rest)) = s.split_first().filter(|_| s.len() >= 3) else { return def };
    let mut t = Vec::with_capacity(s.len());
    t.push(if first == positive { b'+' } else { b'-' });
    t.extend_from_slice(rest);
    str_to_float(&t, def)
}

impl Format for DatDod {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        matches!(line, [b'[', b'B' | b'G' | b'S', b']', b'\t', ..]) && default_test_load_item(&SPEC, line)
    }

    fn load_item1(&mut self, _ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let cols = split(line, &CharSet::of(b"\t"), SPLIT_KEEP_EMPTY);
        let n = cols.len();
        let head = cols.first().map_or(&[][..], |c| c.as_slice());
        if head == b"[S]" && n > 3 {
            let g = |i: usize| str_to_float(&cols[i], 0.0) / 1000.0;
            *it = Item::gsensor(0.0, g(1), g(2), g(3));
            true
        } else if head == b"[G]" && n > 3 {
            it.pt = PT_GPS;
            it.time = str_date_time_to_date_time(&cols[1]);
            it.y = str_position_to_float(&cols[2], b'N', 0.0);
            it.x = str_position_to_float(&cols[3], b'E', 0.0);
            it.speed = if n < 5 { -1.0 } else { str_to_float(&cols[4], 0.0) };
            it.angle = if n < 6 { -1.0 } else { str_to_float(&cols[5], 0.0) };
            true
        } else {
            false
        }
    }

    fn is_allow_frac_seconds(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::str_position_to_float;
    use crate::gps::test_support::*;

    const LOG: &str = "\
[B]\t1\t2\r
[G]\t2016-05-16 13:06:26\tN55.502057\tE37.502057\t45.5\t90.5\r
[S]\t12\t-34\t980\r
[G]\t2016-05-16 13:06:27\tS55.502157\tW37.502157\t46.5\r
[S]\t10\t-30\t990\r
[G]\t2016-05-16 13:06:28.500\tN55.502257\tE37.502257\r
";

    #[test]
    fn position_text() {
        assert_eq!(str_position_to_float(b"N55.5", b'N', 0.0), 55.5);
        assert_eq!(str_position_to_float(b"S55.5", b'N', 0.0), -55.5);
        assert_eq!(str_position_to_float(b"55.5", b'N', 0.0), -5.5);
        assert_eq!(str_position_to_float(b"N5", b'N', 7.0), 7.0);
        assert_eq!(str_position_to_float(b"Nxx", b'N', 7.0), 7.0);
    }

    #[test]
    fn fixes_and_gsensor() {
        let r = load_log("a.dat", LOG.as_bytes()).unwrap();
        assert_eq!(r.format, "dat600");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        assert_eq!(t.gps.len(), 3);
        let p = t.gps[0];
        assert_eq!((p.lat, p.lon, p.speed_kmh, p.course), (55.502057, 37.502057, 45.5, 90.5));
        let p = t.gps[1];
        assert_eq!((p.time, p.lat, p.lon, p.speed_kmh), (1.0, -55.502157, -37.502157, 46.5));
        assert!((t.gps[2].time - 2.5).abs() < 1e-6);
        let g = &t.gsensor;
        assert_eq!(g.len(), 2);
        assert_eq!((g[0].x, g[0].y, g[0].z), (0.012, -0.034, 0.98));
    }

    #[test]
    fn detection_rules() {
        // lower-case letters are bad characters
        assert!(load_log("a.dat", LOG.replace("[B]\t1", "[B]\tx").as_bytes()).is_err());
        // brackets are required
        assert!(load_log("a.dat", LOG.replace('[', "(").as_bytes()).is_err());
    }

    #[test]
    fn fuzz_datdod() {
        fuzz("a.dat", LOG.as_bytes());
    }
}
