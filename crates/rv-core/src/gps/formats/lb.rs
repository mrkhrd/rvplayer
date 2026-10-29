//! `UnitGps_LB`: 24-byte binary G-sensor records "GSEN" + z, x, y (little-endian `f32`, g) +
//! year - 2000, month, day, hour, minute, second (bytes, the camera's local clock) + 2 bytes.
//! The original raises on an invalid date; here such a record is skipped.

use crate::gps::format::{Ctx, Format, Item, PT_GSENSOR, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{copy, encode_date_time, le_f32};

static SPEC: Spec = Spec { title: "LB", ..EMPTY_SPEC };

/// Size of a record.
const RECORD_LEN: usize = 0x18;

pub struct Lb;

pub fn create() -> Box<dyn Format> {
    Box::new(Lb)
}

impl Format for Lb {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        line.len() == RECORD_LEN && copy(line, 1, 4) == b"GSEN"
    }

    fn load_item1(&mut self, _ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        // `gIt := @line[5]`: the record body follows the 4-byte tag
        let Some(r) = line.get(4..) else { return false };
        it.pt = PT_GSENSOR;
        let Some(&[y, mo, d, h, mi, s]) = r.get(12..18) else { return false };
        let Some(time) = encode_date_time(i32::from(y) + 2000, mo.into(), d.into(), h.into(), mi.into(), s.into(), 0)
        else {
            return false;
        };
        let (Some(z), Some(x), Some(gy)) = (le_f32(r, 0), le_f32(r, 4), le_f32(r, 8)) else { return false };
        it.time = time;
        it.x = f64::from(x);
        it.y = f64::from(gy);
        it.z = f64::from(z);
        true
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

    fn record(x: f32, y: f32, z: f32, date: [u8; 6]) -> Vec<u8> {
        let mut r = b"GSEN".to_vec();
        for v in [z, x, y] {
            r.extend_from_slice(&v.to_le_bytes());
        }
        r.extend_from_slice(&date);
        r.extend_from_slice(&[0, 0]);
        assert!(!r.contains(&b'\r') && !r.contains(&b'\n'));
        r
    }

    fn sample() -> Vec<u8> {
        [
            record(0.5, -0.25, 1.0, [16, 5, 16, 11, 0, 0]),
            record(0.25, 0.5, 1.0, [16, 14, 16, 11, 0, 1]), // month 14
            record(-0.5, 0.75, 1.25, [16, 5, 16, 11, 0, 1]),
            record(0.125, 0.5, 0.75, [16, 5, 16, 11, 0, 2]),
        ]
        .join(&b"\r\n"[..])
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

    #[test]
    fn timed_gsensor_records() {
        let r = load_log("a.dat", &sample()).unwrap();
        assert_eq!(r.format, "LB");
        let t = &r.track;
        assert!(t.gps.is_empty());
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 11:00:00");
        let g: Vec<(f64, f64, f64, f64)> = t.gsensor.iter().map(|s| (s.time, s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.0, 0.5, -0.25, 1.0), (1.0, -0.5, 0.75, 1.25), (2.0, 0.125, 0.5, 0.75)]);
    }

    #[test]
    fn records_from_video() {
        let records = vec![
            (0.0, record(0.5, -0.25, 1.0, [16, 5, 16, 11, 0, 0])),
            (0.5, record(0.5, -0.25, 1.0, [16, 5, 16, 24, 0, 0])), // hour 24
            (1.0, [b"GSEX".to_vec(), record(0.5, -0.25, 1.0, [16, 5, 16, 11, 0, 1])[4..].to_vec()].concat()),
            (1.5, record(-0.5, 0.75, 1.25, [16, 5, 16, 11, 0, 2])),
        ];
        let (id, t) = load_mov(records).expect("loaded");
        assert_eq!(id, "movLB");
        // `ShiftTimeToGPS` re-times every single-point record from its frame time (the
        // median offset of record time - frame time anchors frame 0)
        let g: Vec<(f64, f64)> = t.gsensor.iter().map(|s| (s.time, s.x)).collect();
        assert_eq!(g, [(0.0, 0.5), (1.5, -0.5)]);
        assert_eq!(t.start.unwrap().format("%H:%M:%S%.3f").to_string(), "11:00:00.500");
    }

    #[test]
    fn fuzz_lb() {
        fuzz("a.dat", &sample());
        for seed in 0..60u64 {
            let records = (0..6u64)
                .map(|k| {
                    let mut r = noise(seed * 17 + k, if k % 2 == 0 { 24 } else { (seed as usize + k as usize) % 30 });
                    if r.len() >= 4 {
                        r[..4].copy_from_slice(b"GSEN");
                    }
                    (k as f64 * 0.5, r)
                })
                .collect();
            let _ = load_mov(records);
        }
    }
}
