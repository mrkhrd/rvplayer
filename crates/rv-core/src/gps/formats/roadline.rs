//! `UnitGps_RoadLine` (a `TGpsFormat_GPRMC` descendant): RoadLine lines made of blocks that
//! start with "$BEGIN": "$BEGIN$GPRMC,...*hh..." (a fix) and "$BEGINGSENSOR;x;z;y..." (fields
//! split at '$', ':' and ';'; acceleration in g, y negated). Up to two records per line, in
//! block order.
//!
//! As in the original, a "$GPRMC," block is read by the GPRMC parser from the whole line, so a
//! second fix block in the same line repeats the line's first fix. A "GSENSOR" block is split
//! together with the rest of the line: a missing field takes the next block's text.

use crate::gps::format::{Ctx, Format, Item, PT_GSENSOR, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{CharSet, copy, pos, split, str_to_float};

static SPEC: Spec = Spec { title: "RL", ..EMPTY_SPEC };

const BEGIN: &[u8] = b"$BEGIN";

pub struct RoadLine;

pub fn create() -> Box<dyn Format> {
    Box::new(RoadLine)
}

/// `TGpsFormat_GPRMC.LoadItem1` (the parent class).
fn gprmc_load_item1(ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
    crate::gps::formats::gprmc::create().load_item1(ctx, line, it)
}

impl Format for RoadLine {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        pos(BEGIN, line).is_some()
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let mut res = [false, false];
        let mut s = line.to_vec();
        while let Some(p) = pos(BEGIN, &s) {
            // Copy(s, po + 6, Length(s)) with the 1-based po = p + 1
            s = copy(&s, p + 1 + BEGIN.len(), s.len()).to_vec();
            let slot = if !res[0] {
                0
            } else if !res[1] {
                1
            } else {
                break;
            };
            let it = if slot == 0 { &mut *it1 } else { &mut *it2 };
            let id = copy(&s, 1, 7);
            if id == b"GSENSOR" {
                let cols = split(&s, &CharSet::of(b"$:;"), 0);
                if cols.len() > 3 {
                    it.pt = PT_GSENSOR;
                    it.x = str_to_float(&cols[1], 0.0);
                    it.z = str_to_float(&cols[2], 0.0);
                    it.y = -str_to_float(&cols[3], 0.0);
                    it.time = 0.0;
                    res[slot] = true;
                }
            } else if id == b"$GPRMC," {
                res[slot] = gprmc_load_item1(ctx, line, it);
            }
        }
        (res[0], res[1])
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    /// "$" + `body` + "*hh" with the NMEA checksum of `body`.
    fn nmea(body: &str) -> String {
        let x = body.bytes().fold(0u8, |a, c| a ^ c);
        format!("${body}*{x:02X}")
    }

    fn sample_log() -> String {
        let rmc1 = nmea("GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A");
        let rmc2 = nmea("GPRMC,130627.000,A,6003.8100,N,03019.6600,E,37.90,112.10,160516,,,A");
        format!(
            "$BEGIN{rmc1}$END$BEGINGSENSOR;0.12;0.98;-0.05$END\n\
             $BEGINGSENSOR;0.10;0.97;-0.04$END\n\
             $BEGIN{rmc2}$END$BEGINGSENSOR;0.11;0.99;0.02$END\n"
        )
    }

    #[test]
    fn rmc_and_gsensor_blocks() {
        let r = load_log("a.log", sample_log().as_bytes()).unwrap();
        assert_eq!(r.format, "RL");
        let t = &r.track;
        assert_eq!(t.gps.len(), 2);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert!((p.lat - 60.063_626_666).abs() < 1e-8 && (p.lon - 30.327_021_666).abs() < 1e-8);
        assert!((p.speed_kmh - 37.85 * 1.85325).abs() < 1e-9);
        assert_eq!((p.course, t.gps[1].time), (112.0, 1.0));
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.12, 0.05, 0.98), (0.10, 0.04, 0.97), (0.11, -0.02, 0.99)]);
        let times: Vec<f64> = t.gsensor.iter().map(|s| (s.time * 1000.0).round() / 1000.0).collect();
        assert_eq!(times, [0.0, 0.5, 1.0]);
    }

    #[test]
    fn block_rules() {
        let rmc1 = nmea("GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A");
        let rmc2 = nmea("GPRMC,130627.000,A,6003.8100,N,03019.6600,E,37.90,112.10,160516,,,A");
        // an unknown block is skipped; G-sensor then fix; at most two records per line
        let log = format!(
            "$BEGINXYZ$BEGINGSENSOR;0.25;1.0;0.5$BEGIN{rmc1}$BEGINGSENSOR;0.1;0.1;0.1\n\
             $BEGINGSENSOR;0.5;1.0$BEGIN{rmc2}\n"
        );
        let r = load_log("a.log", log.as_bytes()).unwrap();
        assert_eq!(r.format, "RL");
        assert_eq!(r.track.gps.len(), 2);
        let g: Vec<(f64, f64, f64)> = r.track.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        // the second sample's y is the next block's "BEGIN" (0)
        assert_eq!(g, [(0.25, -0.5, 1.0), (0.5, 0.0, 1.0)]);
        assert!(load_log("a.log", b"$BEGINXYZ;1;2;3\n$BEGIN$GPRMC,1,2,3\n").is_err());
    }

    #[test]
    fn fuzz_roadline() {
        fuzz("a.log", sample_log().as_bytes());
    }
}
