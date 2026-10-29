//! `UnitGps_TxtVIOFO`: VIOFO text log. Lines of at least 10 fields separated by blanks and
//! commas: `DD?MM?YY hh?mm?ss status lat N|S lon E|W gx gz gy` with the date and time read at
//! fixed positions (UTC), coordinates as degrees.minutes/100 ("55.521234" = 55°52.1234'),
//! status "A" = good fix, and a G-sensor sample (g) per fix. The data must contain blank,
//! ',', '.', ':' and no lower-case letters, '/', '<', '>', '[', ']', NUL or tab.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::util::{
    CharSet, copy, gp_int_to_coordinate, gp_str_to_date_time, half_sphere_x, half_sphere_y, split, str_to_float,
};

static SPEC: Spec = Spec {
    title: "VIOFO",
    def_ext: "txt",
    format_id: "txtVIOFO",
    must_be_chars_in_data: CharSet::of(b" ,.:"),
    bad_chars: CharSet::ranges(&[
        (0, 0),
        (9, 9),
        (b'/', b'/'),
        (b'<', b'<'),
        (b'>', b'>'),
        (b'[', b'['),
        (b']', b']'),
        (b'a', b'z'),
    ]),
    ..super::EMPTY_SPEC
};

pub struct TxtViofo;

pub fn create() -> Box<dyn Format> {
    Box::new(TxtViofo)
}

impl Format for TxtViofo {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item, it2: &mut Item) -> (bool, bool) {
        let cols = split(line, &CharSet::of(b" ,"), 0);
        if cols.len() < 10 {
            return (false, false);
        }
        let pick = |a: usize, b: usize, c: usize| [copy(line, a, 2), copy(line, b, 2), copy(line, c, 2)].concat();
        it.pt = PT_GPS;
        it.time = gp_str_to_date_time(&pick(1, 4, 7), &pick(10, 13, 16), ctx.tz);
        if it.time <= 2.0 {
            return (false, false);
        }
        it.y = gp_int_to_coordinate(str_to_float(&cols[3], 0.0) * 100.0);
        it.x = gp_int_to_coordinate(str_to_float(&cols[5], 0.0) * 100.0);
        half_sphere_y(&mut it.y, &cols[4]);
        half_sphere_x(&mut it.x, &cols[6]);
        if cols[2] != b"A" {
            it.warn = 2;
        }
        it.z = 0.0;
        it.speed = -1.0;
        it.angle = -1.0;
        it2.pt = PT_GSENSOR;
        it2.time = 0.0;
        it2.x = str_to_float(&cols[7], 0.0);
        it2.z = str_to_float(&cols[8], 0.0);
        it2.y = str_to_float(&cols[9], 0.0);
        (true, true)
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    const LOG: &str = "16.05.16 13:06:26,A,60.038176,N,030.196213,E,0.02,0.98,-0.05\r\n\
16.05.16 13:06:27,A,60.038076,N,030.196613,E,0.03,0.97,-0.04\r\n\
16.05.16 13:06:28,V,60.037976,N,030.197013,E,0.04,0.96,-0.03\r\n\
16.05.16 13:06:29,A,60.037876,S,030.197413,W,0.05,0.95,-0.02\r\n";

    #[test]
    fn fixes_with_gsensor() {
        let r = load_log("2016_0516_130626.TXT", LOG.as_bytes()).unwrap();
        assert_eq!(r.format, "txtVIOFO");
        let t = &r.track;
        assert_eq!(t.gps.len(), 4);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert!((p.lat - 60.063_626_666).abs() < 1e-8 && (p.lon - 30.327_021_666).abs() < 1e-8);
        assert_eq!((t.gps[2].warning, t.gps[3].time), (2, 3.0));
        assert!(t.gps[3].lat < 0.0 && t.gps[3].lon < 0.0);
        // one G-sensor sample per fix, at the fix time: x, z, y order in the line
        assert_eq!(t.gsensor.len(), 4);
        let g = t.gsensor[1];
        assert_eq!((g.time, g.x, g.y, g.z), (1.0, 0.03, -0.04, 0.97));
        // a lower-case letter anywhere disqualifies the file
        assert!(load_log("a.txt", format!("{LOG}note\r\n").as_bytes()).is_err());
        fuzz("a.txt", LOG.as_bytes());
    }
}
