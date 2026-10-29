//! `UnitGps_RoadHawk` (a `TGpsFormat_GPRMC` descendant): RoadHawk lines obfuscated with a
//! character substitution (`DecodeChar`; the text after the first '*', i.e. the checksum, is
//! left as is). A line is this format when it contains the encoded "$GPRMC" ("HO71G>") and its
//! decoded text contains "$GPRMC"; `TestLoadItem` then replaces the record's line with the
//! decoded text. The decoded line holds a G-sensor sample in fixed columns (characters 2-10
//! = y, 12-20 = -x, 22-30 = z, in g) and an RMC sentence read like the GPRMC format.

use crate::gps::format::{Ctx, Format, Item, PT_GSENSOR, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{copy, pos, str_to_float};

static SPEC: Spec = Spec { title: "RH", ..EMPTY_SPEC };

/// Default of the G-sensor fields: out of range, so a sample that does not parse is dropped.
const BAD_G: f64 = -100.0;

pub struct RoadHawk;

pub fn create() -> Box<dyn Format> {
    Box::new(RoadHawk)
}

/// `TGpsFormat_GPRMC.LoadItem1` (the parent class).
fn gprmc_load_item1(ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
    crate::gps::formats::gprmc::create().load_item1(ctx, line, it)
}

/// `DecodeChar`.
pub fn decode_char(c: u8) -> u8 {
    match c {
        b';' => b'0',
        b'=' => b'1',
        b'?' => b'2',
        b'J' => b'3',
        b'L' => b'4',
        b'N' => b'5',
        b'P' => b'6',
        b'R' => b'7',
        b'-' => b'8',
        b'B' => b'9',
        b'H' => b'$',
        b'F' => b',',
        b'D' => b'.',
        b'+' => b'-',
        b':' => b'A',
        b'G' => b'M',
        b'0' => b'W',
        b'O' => b'G',
        b'1' => b'R',
        b'3' => b'N',
        b'7' => b'P',
        b'2' => b'V',
        b'>' => b'C',
        b'I' => b'D',
        b'K' => b'E',
        _ => c,
    }
}

/// `DecodeStr`: decodes up to the first '*'.
pub fn decode_str(s: &[u8]) -> Vec<u8> {
    let mut r = s.to_vec();
    for c in r.iter_mut() {
        if *c == b'*' {
            break;
        }
        *c = decode_char(*c);
    }
    r
}

impl Format for RoadHawk {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, ctx: &mut Ctx, line: &[u8]) -> bool {
        if pos(b"HO71G>", line).is_none() {
            return false;
        }
        let s = decode_str(line);
        if pos(b"$GPRMC", &s).is_none() {
            return false;
        }
        // `context.item.line := s`: LoadItem2 reads the decoded record.
        if let Some(item) = ctx.items.get_mut(ctx.index) {
            item.line = s;
        }
        true
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let s = ctx.items.get(ctx.index).map_or_else(|| line.to_vec(), |i| i.line.clone());
        it2.pt = PT_GSENSOR;
        it2.y = str_to_float(copy(&s, 2, 9), BAD_G);
        it2.x = -str_to_float(copy(&s, 12, 9), BAD_G);
        it2.z = str_to_float(copy(&s, 22, 9), BAD_G);
        it2.time = 0.0;
        let res1 = gprmc_load_item1(ctx, &s, it1);
        (res1, true)
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_char, decode_str};
    use crate::gps::test_support::*;

    /// The inverse of `decode_char` for the characters used in the tests (up to the '*').
    fn encode(plain: &str) -> String {
        let mut out = String::new();
        let mut after_star = false;
        for c in plain.bytes() {
            after_star |= c == b'*';
            if after_star {
                out.push(c as char);
                continue;
            }
            let e = (0..=255u8).find(|&e| e != c && decode_char(e) == c).unwrap_or(c);
            assert_eq!(decode_char(e), c, "{} is not encodable", c as char);
            out.push(e as char);
        }
        out
    }

    /// `body` + "*hh" with the NMEA checksum of the text after the first '$'.
    fn nmea(body: &str) -> String {
        let start = body.find('$').map_or(0, |p| p + 1);
        let x = body[start..].bytes().fold(0u8, |a, c| a ^ c);
        format!("{body}*{x:02X}")
    }

    fn sample_log() -> String {
        let l1 =
            nmea("X0.0100000,-0.020000,0.9800000,$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A");
        let l2 =
            nmea("X0.0200000,0.0400000,1.0100000,$GPRMC,130627.000,A,6003.8100,N,03019.6600,E,37.90,112.10,160516,,,A");
        format!("{}\r\n{}\r\n", encode(&l1), encode(&l2))
    }

    #[test]
    fn decoding_is_a_substitution_up_to_the_star() {
        assert_eq!(decode_str(b"HO71G>F;=?JLNPR-BDIK+:03*1;"), b"$GPRMC,0123456789.DE-AWN*1;");
        assert_eq!(decode_str(b"xyz S"), b"xyz S");
        let all: Vec<u8> = (0..=255u8).filter(|&c| c != b'*').collect();
        let changed = all.iter().filter(|&&c| decode_char(c) != c).count();
        assert_eq!(changed, 25);
    }

    #[test]
    fn encoded_lines() {
        let log = sample_log();
        assert!(log.contains("HO71G>") && !log.contains("GPRMC"));
        let r = load_log("a.txt", log.as_bytes()).unwrap();
        assert_eq!(r.format, "RH");
        let t = &r.track;
        assert_eq!(t.gps.len(), 2);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert!((p.lat - 60.063_626_666).abs() < 1e-8 && (p.lon - 30.327_021_666).abs() < 1e-8);
        assert!((p.speed_kmh - 37.85 * 1.85325).abs() < 1e-9);
        assert_eq!((p.course, t.gps[1].time), (112.0, 1.0));
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.02, 0.01, 0.98), (-0.04, 0.02, 1.01)]);
        assert_eq!((t.gsensor[0].time, t.gsensor[1].time), (0.0, 1.0));
    }

    #[test]
    fn line_rules() {
        // a G-sensor field that does not parse drops the sample, not the fix
        let l1 =
            nmea("X0.0100000,-0.020000,0.98x0000,$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A");
        // a wrong checksum drops the fix, not the sample
        let l2 =
            nmea("X0.0200000,0.0400000,1.0100000,$GPRMC,130627.000,A,6003.8100,N,03019.6600,E,37.90,112.10,160516,,,A")
                .replace("*", "*0");
        let l3 =
            nmea("X0.0300000,0.0500000,1.0200000,$GPRMC,130628.000,A,6003.8000,N,03019.7000,E,38.00,112.20,160516,,,A");
        let log = format!("{}\n{}\n{}\n", encode(&l1), encode(&l2), encode(&l3));
        let r = load_log("a.txt", log.as_bytes()).unwrap();
        assert_eq!(r.format, "RH");
        let times: Vec<f64> = r.track.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [0.0, 2.0]);
        let g: Vec<(f64, f64, f64)> = r.track.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(-0.04, 0.02, 1.01), (-0.05, 0.03, 1.02)]);
        // plain (not encoded) RMC text is not this format
        let plain =
            nmea("X0.0100000,-0.020000,0.9800000,$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A");
        let r = load_log("a.txt", format!("{plain}\n").as_bytes());
        assert!(r.is_err() || r.unwrap().format != "RH");
    }

    #[test]
    fn fuzz_roadhawk() {
        fuzz("a.txt", sample_log().as_bytes());
    }
}
