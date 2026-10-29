//! `UnitGps_CamBox` (a `TGpsFormat_GPRMC` descendant): CamBox lines
//! "$G?RMC,...*hh,altitude,x,y,z" plus one trailing character, or G-sensor-only lines
//! "$,x,y,z". The fix is read like the GPRMC format, the altitude (m) from the field after
//! the checksum; the acceleration (g, y negated) from the last three fields.
//!
//! A line is this format when it is longer than 50 characters, starts with '$', has ',' at
//! the 16th and 31st character from the end, ends with a non-digit preceded by a digit.

use crate::gps::format::{Ctx, Format, Item, PT_GSENSOR, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{CharSet, DIGITS, copy, split, str_to_float};

static SPEC: Spec = Spec { title: "CamBox", ..EMPTY_SPEC };

/// Default of the G-sensor fields: out of range, so a sample that does not parse is dropped.
const BAD_G: f64 = 10.0;

pub struct CamBox;

pub fn create() -> Box<dyn Format> {
    Box::new(CamBox)
}

/// `TGpsFormat_GPRMC.LoadItem1` (the parent class).
fn gprmc_load_item1(ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
    crate::gps::formats::gprmc::create().load_item1(ctx, line, it)
}

/// Delphi `Delete(s, index, count)` with a 1-based index.
fn delete(s: &mut Vec<u8>, index: usize, count: usize) {
    if index < 1 || index > s.len() {
        return;
    }
    let end = (index - 1).saturating_add(count).min(s.len());
    s.drain(index - 1..end);
}

impl Format for CamBox {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        let n = line.len();
        n > 50
            && line[0] == b'$'
            && line[n - 16] == b','
            && line[n - 31] == b','
            && !DIGITS.contains(line[n - 1])
            && DIGITS.contains(line[n - 2])
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let (mut res1, mut res2) = (false, false);
        let mut s = line.to_vec();
        if s.len() > 3 && s[1] == b'G' {
            s[2] = b'_';
        }
        if copy(&s, 1, 6) == b"$G_RMC" {
            res1 = gprmc_load_item1(ctx, line, it1);
            // 1-based positions as in the original
            let n = s.len();
            let mut p = 3;
            while p < n && s[p - 1] != b'*' {
                p += 1;
            }
            while p < n && s[p - 1] != b',' {
                p += 1;
            }
            p += 1;
            let p0 = p;
            while p < n && s[p - 1] != b',' {
                p += 1;
            }
            it1.z = str_to_float(copy(&s, p0, p - p0), 0.0);
            // leaves "$" + ",x,y,z..."
            delete(&mut s, 2, p - 2);
        }
        if copy(&s, 1, 2) == b"$," {
            let cols = split(&s, &CharSet::of(b","), 0);
            if cols.len() > 3 {
                it2.pt = PT_GSENSOR;
                it2.time = 0.0;
                it2.x = str_to_float(&cols[1], BAD_G);
                it2.y = -str_to_float(&cols[2], BAD_G);
                it2.z = str_to_float(&cols[3], BAD_G);
                res2 = true;
            }
        }
        (res1, res2)
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::format::Ctx;
    use crate::gps::test_support::*;

    /// "$" + `body` + "*hh" with the NMEA checksum of `body`.
    fn nmea(body: &str) -> String {
        let x = body.bytes().fold(0u8, |a, c| a ^ c);
        format!("${body}*{x:02X}")
    }

    /// The G-sensor tail: x, then y and z of 14 characters each, then a blank.
    fn tail(x: &str, y: f64, z: f64) -> String {
        format!(",{x},{y:+014.10},{z:+014.10} ")
    }

    fn sample_log() -> String {
        let l1 = nmea("GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A");
        let l2 = nmea("GNRMC,130627.000,A,6003.8100,N,03019.6600,E,37.90,112.10,160516,,,A");
        format!(
            "{l1},125.5{}\n{l2},126.0{}\n${}\n",
            tail("+0.01", -0.02, 0.98),
            tail("-0.25", 0.5, 1.0),
            tail("0.125000000000000000", 0.25, 0.75)
        )
    }

    #[test]
    fn rmc_with_altitude_and_gsensor() {
        let log = sample_log();
        let mut f = super::create();
        for l in log.lines() {
            assert!(f.test_load_item(&mut Ctx::default(), l.as_bytes()), "{l}");
        }
        let r = load_log("a.txt", log.as_bytes()).unwrap();
        assert_eq!(r.format, "CamBox");
        let t = &r.track;
        assert_eq!(t.gps.len(), 2);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert!((p.lat - 60.063_626_666).abs() < 1e-8 && (p.lon - 30.327_021_666).abs() < 1e-8);
        assert!((p.speed_kmh - 37.85 * 1.85325).abs() < 1e-9);
        assert_eq!((p.course, p.altitude), (112.0, 125.5));
        assert_eq!((t.gps[1].time, t.gps[1].altitude), (1.0, 126.0));
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.01, 0.02, 0.98), (-0.25, -0.5, 1.0), (0.125, -0.25, 0.75)]);
        let times: Vec<f64> = t.gsensor.iter().map(|s| (s.time * 1000.0).round() / 1000.0).collect();
        assert_eq!(times[..2], [0.0, 1.0]);
    }

    #[test]
    fn line_rules() {
        let mut f = super::create();
        let mut ctx = Ctx::default();
        let rmc = nmea("GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A");
        let good = format!("{rmc},125.5{}", tail("0.1", 0.2, 0.3));
        assert!(f.test_load_item(&mut ctx, good.as_bytes()));
        // ends with a digit / no digit before the last character / commas shifted
        assert!(!f.test_load_item(&mut ctx, good.trim_end().as_bytes()));
        assert!(!f.test_load_item(&mut ctx, format!("{good}x").as_bytes()));
        assert!(!f.test_load_item(&mut ctx, format!("{}0 ", good.trim_end()).as_bytes()));
        // a G-sensor field that does not parse drops the sample, not the fix
        let line2 = sample_log().lines().nth(1).unwrap().to_string();
        let log = format!("{rmc},125.5,0.1,+0.20000000000,+0.30000x00000 \n{line2}\n");
        let r = load_log("a.txt", log.as_bytes()).unwrap();
        assert_eq!(r.format, "CamBox");
        assert_eq!(r.track.gps.len(), 2);
        assert_eq!(r.track.gsensor.len(), 0, "a single sample is dropped");
        // no checksum field: altitude 0, no sample
        let no_star = "$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A,1,2,3,4,5,6,7 ";
        let mut it1 = crate::gps::format::Item::default();
        let mut it2 = crate::gps::format::Item::default();
        assert_eq!(f.load_item2(&mut ctx, no_star.as_bytes(), &mut it1, &mut it2), (false, false));
        assert_eq!(it1.z, 0.0);
    }

    #[test]
    fn fuzz_cambox() {
        fuzz("a.txt", sample_log().as_bytes());
    }
}
