//! `UnitGps_MagicEye` (a `TGpsFormat_GPRMC` descendant): records of MagicEye cameras.
//! - "M" + 14 bytes: a G-sensor sample, bytes 1, 2, 3 = z, y, -x as `(b - 128) / 64` g;
//! - "G$GPRMC,...": an RMC sentence read like the GPRMC format. When the second-to-last
//!   character is '*' (a one-digit checksum) the last two characters are cut off first, so
//!   such a sentence is read without its checksum.

use crate::gps::format::{Ctx, Format, Item, PT_GSENSOR, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::copy;

static SPEC: Spec = Spec { title: "ME", ..EMPTY_SPEC };

/// Length of a G-sensor record.
const GSENSOR_LEN: usize = 15;

pub struct MagicEye;

pub fn create() -> Box<dyn Format> {
    Box::new(MagicEye)
}

/// `TGpsFormat_GPRMC.LoadItem1` (the parent class).
fn gprmc_load_item1(ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
    crate::gps::formats::gprmc::create().load_item1(ctx, line, it)
}

/// `(b - $80) / 64` (an integer converted to float, then divided).
fn g(v: i32) -> f64 {
    f64::from(v) / 64.0
}

impl Format for MagicEye {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        match line.first() {
            Some(b'M') => line.len() == GSENSOR_LEN,
            Some(b'G') => copy(line, 2, 6) == b"$GPRMC",
            _ => false,
        }
    }

    fn load_item1(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        match line.first() {
            Some(b'M') => {
                let (Some(&b1), Some(&b2), Some(&b3)) = (line.get(1), line.get(2), line.get(3)) else {
                    return false;
                };
                it.pt = PT_GSENSOR;
                it.time = 0.0;
                it.x = g(-(i32::from(b3) - 0x80));
                it.y = g(i32::from(b2) - 0x80);
                it.z = g(i32::from(b1) - 0x80);
                true
            }
            Some(b'G') => {
                // The original deletes the two characters from the record itself.
                let n = line.len();
                let s = if n >= 2 && line[n - 2] == b'*' { &line[..n - 2] } else { line };
                gprmc_load_item1(ctx, s, it)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::build::to_track;
    use crate::gps::format::RawItem;
    use crate::gps::manager::{Env, load_items};
    use crate::gps::test_support::*;
    use crate::gps::util::SEC;
    use crate::track::Track;

    /// "$" + `body` + "*hh" with the NMEA checksum of `body`.
    fn nmea(body: &str) -> String {
        let x = body.bytes().fold(0u8, |a, c| a ^ c);
        format!("${body}*{x:02X}")
    }

    fn gsensor(z: u8, y: u8, x: u8) -> Vec<u8> {
        let mut r = vec![b'M', z, y, x];
        r.resize(15, b' ');
        r
    }

    fn load_mov(records: Vec<(f64, Vec<u8>)>) -> Option<(String, Track)> {
        let mut formats = crate::gps::formats::all();
        let env = Env { name: "x.mov".to_string(), file_times: (0.0, 0.0), tz: 0.0, companion: &|_| None };
        let duration = records.iter().map(|r| r.0).fold(0.0, f64::max) * SEC;
        let items = records.into_iter().map(|(t, b)| RawItem::timed(t * SEC, b)).collect();
        let loaded = load_items(&mut formats, None, items, 0.0, "mov", duration, &env)?;
        let track = to_track(&loaded.points, None, (0.0, 0.0), !loaded.info.do_not_post_process_fix);
        Some((loaded.info.format_id, track))
    }

    fn sample_log() -> Vec<u8> {
        let mut log = Vec::new();
        // a one-digit "checksum" is cut off: read without a checksum (13 fields, mode 'A')
        log.extend_from_slice(b"G$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,20.00,45.00,160516,,,A*7\r\n");
        log.extend(gsensor(0xc0, 0x90, 0x60));
        log.extend_from_slice(b"\r\n");
        log.extend(gsensor(0x80, 0x70, 0xa0));
        log.extend_from_slice(b"\r\n");
        log.push(b'G');
        log.extend_from_slice(nmea("GPRMC,130627.000,A,6003.8276,N,03019.6213,E,21.00,46.00,160516,,,A").as_bytes());
        log.extend_from_slice(b"\r\n");
        log
    }

    #[test]
    fn rmc_and_gsensor_records() {
        let r = load_log("a.txt", &sample_log()).unwrap();
        assert_eq!(r.format, "ME");
        let t = &r.track;
        assert_eq!(t.gps.len(), 2);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert!((p.lat - 60.063_626_666).abs() < 1e-8 && (p.lon - 30.327_021_666).abs() < 1e-8);
        assert!((p.speed_kmh - 20.0 * 1.85325).abs() < 1e-9);
        assert_eq!((p.course, t.gps[1].time), (45.0, 1.0));
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.5, 0.25, 1.0), (-0.5, -0.25, 0.0)]);
        assert_eq!(t.gsensor[0].time, 0.0);
    }

    #[test]
    fn record_rules() {
        let rmc = nmea("GPRMC,130626.000,A,6003.8176,N,03019.6213,E,20.00,45.00,160516,,,A");
        let mut bad = rmc.clone().into_bytes();
        let n = bad.len();
        bad[n - 1] = if bad[n - 1] == b'0' { b'1' } else { b'0' };
        let records = vec![
            (0.0, [b"G".to_vec(), rmc.clone().into_bytes()].concat()),
            (0.5, gsensor(0xc0, 0x90, 0x60)),
            (0.7, [gsensor(0xc0, 0x90, 0x61), b"x".to_vec()].concat()), // 16 bytes: not a record
            (0.9, [b"G".to_vec(), bad].concat()),                       // wrong checksum
            (1.2, gsensor(0x81, 0x80, 0x80)),
        ];
        let (id, t) = load_mov(records).expect("loaded");
        assert_eq!(id, "movME");
        assert_eq!(t.gps.len(), 1);
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.5, 0.25, 1.0), (0.0, 0.0, 1.0 / 64.0)]);

        let mut f = super::create();
        let mut ctx = crate::gps::format::Ctx::default();
        assert!(f.test_load_item(&mut ctx, &[b"G".to_vec(), rmc.clone().into_bytes()].concat()));
        assert!(!f.test_load_item(&mut ctx, &[b"X".to_vec(), rmc.clone().into_bytes()].concat()));
        assert!(!f.test_load_item(&mut ctx, &[b"G ".to_vec(), rmc.into_bytes()].concat()));
        assert!(!f.test_load_item(&mut ctx, &gsensor(1, 2, 3)[..14]));
        assert!(f.test_load_item(&mut ctx, &gsensor(1, 2, 3)));
    }

    #[test]
    fn fuzz_magiceye() {
        fuzz("a.txt", &sample_log());
    }
}
