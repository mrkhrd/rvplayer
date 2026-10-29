//! `UnitGps_PTEKD`: G-sensor sentences "$PTEKD,..." of Vico cameras, usually interleaved with
//! NMEA sentences (those are read by the NMEA format). The sentence is split at ',' and '*'
//! (empty fields dropped); fields 4, 5 and 6 are y, x and z in g. No time: the samples are
//! placed between the GPS fixes around them.

use crate::gps::format::{Ctx, Format, Item, PT_GSENSOR, Spec, default_test_load_item};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{CharSet, pos, split, str_to_float};

/// `magic`.
const MAGIC: &[u8] = b"$PTEKD";

static SPEC: Spec = Spec {
    title: "Vico",
    // `[',', '.', '$'] + [magic[1], magic[2]]`
    must_be_chars_in_line: CharSet::of(b"$,.P"),
    must_be_strings_in_data: &["$ptekd"],
    ..EMPTY_SPEC
};

pub struct Ptekd;

pub fn create() -> Box<dyn Format> {
    Box::new(Ptekd)
}

impl Format for Ptekd {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        pos(MAGIC, line).is_some() && default_test_load_item(&SPEC, line)
    }

    fn load_item1(&mut self, _ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let Some(i) = pos(MAGIC, line) else { return false };
        let cols = split(&line[i..], &CharSet::of(b"*,"), 0);
        if cols.len() < 7 || cols[0] != MAGIC {
            return false;
        }
        it.pt = PT_GSENSOR;
        it.y = str_to_float(&cols[4], 0.0);
        it.x = str_to_float(&cols[5], 0.0);
        it.z = str_to_float(&cols[6], 0.0);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;
    use crate::gps::util::SEC;

    const T0: f64 = 42_506.5;

    #[test]
    fn gsensor_sentences_between_rmc_fixes() {
        let log = fix_checksums(
            "$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,10.00,90.00,160516,,,A*00
$PTEKD,130626,A,1,0.12,-0.05,0.98*3C
$PTEKD,130626,A,2,0.10,-0.06,0.97*3C
$GPRMC,130627.000,A,6003.8176,N,03019.6413,E,10.00,90.00,160516,,,A*00
$PTEKD,130627,A,1,0.11,-0.04,0.99*3C
$PTEKD,130627,A,2,0.13,-0.03,1.01*3C
",
            &[0, 3],
        );
        let r = load_log("trip.nmea", log.as_bytes()).unwrap();
        // the format of the last record read names the log
        assert_eq!(r.format, "Vico");
        assert_eq!(r.track.gps.len(), 2);
        let g: Vec<(f64, f64, f64)> = r.track.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(-0.05, 0.12, 0.98), (-0.06, 0.10, 0.97), (-0.04, 0.11, 0.99), (-0.03, 0.13, 1.01)]);
        let times: Vec<f64> = r.track.gsensor.iter().map(|s| (s.time * 1000.0).round() / 1000.0).collect();
        assert_eq!(times, [0.0, 0.5, 1.0, 1.495]);
    }

    #[test]
    fn field_rules() {
        // empty fields are dropped before counting; a line without '.' is not a PTEKD line;
        // the magic may follow other text; fewer than 7 fields is not a sample
        let log = "\
xx$PTEKD,,1,2,3,0.5,0.25,1.0*00
$PTEKD,1,2,3,1,1,1*00
$PTEKD,0.1,2,0.5,0.5*00
$PTEKD,1,2,3,0.75,-0.5,1.25
";
        let r = load_log_at("a.log", log.as_bytes(), (T0, T0 + 10.0 * SEC)).unwrap();
        assert_eq!(r.format, "Vico");
        let g: Vec<(f64, f64, f64)> = r.track.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.25, 0.5, 1.0), (-0.5, 0.75, 1.25)]);
        assert_eq!(r.track.gsensor[1].time, 10.0);
        // "$PTEKD" must occur (any case) in the first 4096 bytes
        let mut late = vec![b' '; 5000];
        late.extend_from_slice(b"\n$PTEKD,1,2,3,0.75,-0.5,1.25\n$PTEKD,1,2,3,0.5,-0.5,1.25\n");
        assert!(load_log_at("a.log", &late, (T0, T0 + 10.0 * SEC)).is_err());
    }

    #[test]
    fn fuzz_ptekd() {
        let log = fix_checksums(
            "$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,10.00,90.00,160516,,,A*00
$PTEKD,130626,A,1,0.12,-0.05,0.98*3C
$PTEKD,130626,A,2,0.10,-0.06,0.97*3C
$GPRMC,130627.000,A,6003.8176,N,03019.6413,E,10.00,90.00,160516,,,A*00
",
            &[0, 3],
        );
        fuzz("a.nmea", log.as_bytes());
    }
}
