//! `UnitGps_Txt1000A` (class `TGpsFromat_TXT`): GPS1000A text logs, 20 comma-separated
//! fields per line: 0 = local date and time "YYYY?MM?DD?hh?mm?ss[.fff]", 1 = longitude,
//! 2 = latitude (decimal degrees), 3, 4, 5 = G-sensor z, x, -y in 1/1000 g, 14 = speed (km/h).
//! No altitude or course. The G-sensor sample is read only with a timed fix.
//!
//! The data must contain all of ",-./:" and none of TAB, NUL, '<', '>', '[', ']', upper-case
//! letters or lower-case letters other than 'a' and 'n'.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{CharSet, SPLIT_KEEP_EMPTY, split, str_date_time_to_date_time, str_to_float};

static SPEC: Spec = Spec {
    title: "GPS1000A",
    def_ext: "txt",
    format_id: "txt",
    must_be_chars_in_data: CharSet::of(b",-./:"),
    bad_chars: CharSet::ranges(&[
        (0, 0),
        (9, 9),
        (b'<', b'<'),
        (b'>', b'>'),
        (b'A', b'Z'),
        (b'[', b'['),
        (b']', b']'),
        (b'b', b'm'),
        (b'o', b'z'),
    ]),
    ..EMPTY_SPEC
};

/// Default of the G-sensor fields (99 g after scaling: out of range, the sample is dropped).
const BAD_G: f64 = 99_000.0;

pub struct Txt1000a;

pub fn create() -> Box<dyn Format> {
    Box::new(Txt1000a)
}

impl Format for Txt1000a {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn load_item2(&mut self, _ctx: &mut Ctx, line: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let cols = split(line, &CharSet::of(b","), SPLIT_KEEP_EMPTY);
        if cols.len() != 20 {
            return (false, false);
        }
        it1.pt = PT_GPS;
        it1.time = str_date_time_to_date_time(&cols[0]);
        if it1.time <= 2.0 {
            return (false, false);
        }
        it1.x = str_to_float(&cols[1], 0.0);
        it1.y = str_to_float(&cols[2], 0.0);
        it1.z = 0.0;
        it1.speed = str_to_float(&cols[14], -1.0);
        it1.angle = -1.0;
        it2.pt = PT_GSENSOR;
        it2.time = 0.0;
        it2.z = str_to_float(&cols[3], BAD_G) / 1000.0;
        it2.x = str_to_float(&cols[4], BAD_G) / 1000.0;
        it2.y = -str_to_float(&cols[5], BAD_G) / 1000.0;
        (true, true)
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    fn line(time: &str, lon: &str, lat: &str, g: [&str; 3], speed: &str) -> String {
        let mut f = vec![time, lon, lat, g[0], g[1], g[2]];
        f.extend(["0"; 8]);
        f.push(speed);
        f.extend(["0"; 5]);
        assert_eq!(f.len(), 20);
        f.join(",")
    }

    fn sample_log() -> String {
        [
            line("2016/05/16 13:06:26", "30.327021", "60.063626", ["980", "12", "-34"], "45.5"),
            line("2016/05/16 13:06:27", "30.327221", "60.063726", ["990", "-20", "40"], "46"),
            line("2016/05/16 13:06:28.5", "30.327421", "60.063826", ["1010", "5", "0"], "47.25"),
        ]
        .join("\r\n")
    }

    #[test]
    fn fixes_with_gsensor() {
        let r = load_log("a.txt", sample_log().as_bytes()).unwrap();
        assert_eq!(r.format, "txt");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p: Vec<(f64, f64, f64, f64)> = t.gps.iter().map(|p| (p.time, p.lon, p.lat, p.speed_kmh)).collect();
        assert_eq!(
            p,
            [(0.0, 30.327021, 60.063626, 45.5), (1.0, 30.327221, 60.063726, 46.0), (2.5, 30.327421, 60.063826, 47.25)]
        );
        assert_eq!(t.gps[0].altitude, 0.0);
        let g: Vec<(f64, f64, f64, f64)> = t.gsensor.iter().map(|s| (s.time, s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.0, 0.012, 0.034, 0.98), (1.0, -0.02, -0.04, 0.99), (2.5, 0.005, -0.0, 1.01)]);
    }

    #[test]
    fn line_and_data_rules() {
        let log = [
            line("2016/05/16 13:06:26", "30.327021", "60.063626", ["", "12", "-34"], ""),
            line("2016/05/16 13:06:61", "30.327121", "60.063626", ["980", "12", "-34"], "1"), // bad time
            line("2016/05/16 13:06:27", "30.327221", "60.063726", ["990", "-20", "40"], "46") + ",1", // 21 fields
            line("2016/05/16 13:06:28", "30.327421", "60.063826", ["1010", "5", "1"], "47") + ",", // trailing ','
        ]
        .join("\n");
        let r = load_log("a.txt", log.as_bytes()).unwrap();
        assert_eq!(r.format, "txt");
        let times: Vec<f64> = r.track.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [0.0, 2.0]);
        // no speed: computed from the distance
        assert!(r.track.gps[0].speed_kmh > 0.0);
        assert_eq!(r.track.gsensor.len(), 0, "the first sample has no z; a single sample is dropped");
        // upper-case letters anywhere, or a missing '/', reject the whole data
        assert!(load_log("a.txt", format!("{}\nX", sample_log()).as_bytes()).is_err());
        assert!(load_log("a.txt", sample_log().replace('/', "-").as_bytes()).is_err());
    }

    #[test]
    fn fuzz_txt1000a() {
        fuzz("a.txt", sample_log().as_bytes());
    }
}
