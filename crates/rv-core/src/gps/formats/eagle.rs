//! `UnitGps_Eagle`: 68-byte binary G-sensor records of "Eagle" cameras, taken from a video's
//! data track. A record starts with a zero dword followed by a non-zero dword; the
//! acceleration is three little-endian `f32` in g: x at +0x2c, z at +0x30 and y (negated) at
//! +0x34. The records carry no time.
//!
//! `TestLoadData` always fails, so the format never reads log files, only records from videos.

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GSENSOR, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{le_f32, le_i32};

static SPEC: Spec = Spec { title: "Eag", ..EMPTY_SPEC };

/// Size of a record (`TestLoadItem`).
const RECORD_LEN: usize = 0x44;

pub struct Eagle;

pub fn create() -> Box<dyn Format> {
    Box::new(Eagle)
}

impl Format for Eagle {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, _data: &[u8]) -> DataTest {
        DataTest::No
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        line.len() == RECORD_LEN && le_i32(line, 0) == Some(0) && le_i32(line, 4).is_some_and(|v| v != 0)
    }

    fn load_item1(&mut self, _ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        it.pt = PT_GSENSOR;
        // The original range-checks only the first byte read (index 0x2c).
        let (Some(x), Some(z), Some(y)) = (le_f32(line, 0x2c), le_f32(line, 0x30), le_f32(line, 0x34)) else {
            return false;
        };
        it.x = f64::from(x);
        it.y = f64::from(-y);
        it.z = f64::from(z);
        it.time = 0.0;
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::build::to_track;
    use crate::gps::format::{Ctx, DataTest, RawItem};
    use crate::gps::manager::{Env, load_items};
    use crate::gps::test_support::*;
    use crate::gps::util::SEC;
    use crate::track::Track;

    /// Records of a video data track (frame time in seconds) through the embedded-data path.
    fn load_mov(records: Vec<(f64, Vec<u8>)>, file_times: (f64, f64)) -> Option<(String, Track)> {
        let mut formats = crate::gps::formats::all();
        let env = Env { name: "x.mov".to_string(), file_times, tz: 0.0, companion: &|_| None };
        let duration = records.iter().map(|r| r.0).fold(0.0, f64::max) * SEC;
        let items = records.into_iter().map(|(t, b)| RawItem::timed(t * SEC, b)).collect();
        let loaded = load_items(&mut formats, None, items, 0.0, "mov", duration, &env)?;
        let track = to_track(&loaded.points, None, file_times, !loaded.info.do_not_post_process_fix);
        Some((loaded.info.format_id, track))
    }

    fn record(tag: u32, x: f32, y: f32, z: f32) -> Vec<u8> {
        let mut r = vec![0u8; 0x44];
        r[4..8].copy_from_slice(&tag.to_le_bytes());
        r[0x2c..0x30].copy_from_slice(&x.to_le_bytes());
        r[0x30..0x34].copy_from_slice(&z.to_le_bytes());
        r[0x34..0x38].copy_from_slice(&(-y).to_le_bytes());
        r
    }

    const T0: f64 = 42_506.5;

    #[test]
    fn gsensor_records_from_video() {
        let mut bad_head = record(5, 0.5, 0.5, 0.5);
        bad_head[0] = 1;
        let records = vec![
            (0.0, record(1, 0.5, -0.25, 1.0)),
            (0.5, bad_head),
            (1.0, record(2, 0.12, 0.34, 0.98)),
            (1.5, record(3, 0.0, 0.0, 0.0)), // all zero: not a valid sample
            (2.0, record(4, -1.5, 0.75, 1.25)),
            (2.5, record(0, 0.5, 0.5, 0.5)), // second dword zero: not a record
            (3.0, record(5, 0.25, 0.25, 0.25)),
        ];
        let (id, t) = load_mov(records, (T0, T0 + 3.0 * SEC)).expect("loaded");
        assert_eq!(id, "movEag");
        assert!(t.gps.is_empty());
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(
            g,
            [
                (0.5, -0.25, 1.0),
                (f64::from(0.12f32), f64::from(0.34f32), f64::from(0.98f32)),
                (-1.5, 0.75, 1.25),
                (0.25, 0.25, 0.25)
            ]
        );
        // untimed samples are spread over the file's time window
        assert_eq!(t.gsensor[0].time, 0.0);
        assert_eq!(t.gsensor[3].time, 3.0);
    }

    #[test]
    fn never_reads_log_files() {
        let data: Vec<u8> = [record(1, 0.5, -0.25, 1.0), record(2, 0.25, 0.5, 1.0)].concat();
        let mut ctx = Ctx::default();
        assert_eq!(super::create().test_load_data(&mut ctx, &data), DataTest::No);
        assert_eq!(load_log("x.dat", &data), Err(GpsError::NoData));
    }

    #[test]
    fn fuzz_eagle() {
        let data: Vec<u8> = [record(1, 0.5, -0.25, 1.0), b"\r\n".to_vec(), record(2, 0.25, 0.5, 1.0)].concat();
        fuzz("x.dat", &data);
        for seed in 0..60u64 {
            let mut records = Vec::new();
            for k in 0..8u64 {
                let len = if k % 3 == 0 { 0x44 } else { (seed as usize * 7 + k as usize * 13) % 0x50 };
                let mut r = noise(seed * 31 + k, len);
                if r.len() >= 4 {
                    r[..4].fill(0);
                }
                records.push((k as f64 * 0.5, r));
            }
            let _ = load_mov(records, (T0, T0 + 4.0 * SEC));
        }
    }
}
