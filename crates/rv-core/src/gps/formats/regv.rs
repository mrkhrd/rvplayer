//! `UnitGps_RegV`: Registrator Viewer's own records, the samples of the `RVMI` data track it
//! writes into saved fragments (see [`crate::rvmi`] for the layout). Records end with 0x0C:
//! `tReV` (13 bytes, start TDateTime), `gReV` (21 bytes, GPS) and `sReV` (11 bytes,
//! G-sensor in 1/1000 g). A record's time is the start time plus its frame time. Never
//! detected in log files; the clean-up passes are skipped for this data.

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::util::{le_f32, le_f64, le_i16, le_i32, le_u16};

static SPEC: Spec = Spec { title: "RegViewer", format_id: "RegV", ..super::EMPTY_SPEC };

#[derive(Default)]
pub struct RegV {
    zero_time: f64,
}

pub fn create() -> Box<dyn Format> {
    Box::new(RegV::default())
}

impl Format for RegV {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, _data: &[u8]) -> DataTest {
        DataTest::No
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        line.last() == Some(&0x0c)
            && matches!((line.len(), line.get(..4)), (13, Some(b"tReV")) | (21, Some(b"gReV")) | (11, Some(b"sReV")))
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {
        self.zero_time = 0.0;
    }

    fn load_item1(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let time = self.zero_time + ctx.frame_time();
        match line.get(..4) {
            Some(b"sReV") => {
                let (Some(x), Some(y), Some(z)) = (le_i16(line, 4), le_i16(line, 6), le_i16(line, 8)) else {
                    return false;
                };
                let g = |v: i16| f64::from(v) / 1000.0;
                *it = Item { pt: PT_GSENSOR, time, x: g(x), y: g(y), z: g(z), ..Default::default() };
                true
            }
            Some(b"gReV") => {
                let (Some(x), Some(y), Some(h), Some(speed), Some(&course), Some(&warn)) =
                    (le_i32(line, 4), le_i32(line, 8), le_f32(line, 12), le_u16(line, 16), line.get(18), line.get(19))
                else {
                    return false;
                };
                *it = Item {
                    pt: PT_GPS,
                    time,
                    x: f64::from(x) / 1e6,
                    y: f64::from(y) / 1e6,
                    z: f64::from(h),
                    speed: f64::from(speed) / 10.0,
                    angle: f64::from(course) * 2.0,
                    warn,
                };
                true
            }
            Some(b"tReV") => {
                if let Some(t) = le_f64(line, 4) {
                    self.zero_time = t;
                }
                false
            }
            _ => false,
        }
    }

    fn do_post_process_fix(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::format::RawItem;
    use crate::gps::manager::{Env, load_items};
    use crate::gps::util::SEC;

    fn g_rev(lon: f64, lat: f64, alt: f32, speed10: u16, course_half: u8) -> Vec<u8> {
        let mut v = b"gReV".to_vec();
        v.extend_from_slice(&((lon * 1e6).round() as i32).to_le_bytes());
        v.extend_from_slice(&((lat * 1e6).round() as i32).to_le_bytes());
        v.extend_from_slice(&alt.to_le_bytes());
        v.extend_from_slice(&speed10.to_le_bytes());
        v.extend_from_slice(&[course_half, 0, 0x0c]);
        v
    }

    #[test]
    fn records_from_a_video_track() {
        let t0: f64 = 42_506.546_134_259; // 2016-05-16 13:06:26
        let mut start = b"tReV".to_vec();
        start.extend_from_slice(&t0.to_le_bytes());
        start.push(0x0c);
        let mut s_rev = b"sReV".to_vec();
        for v in [-120i16, 15, 980] {
            s_rev.extend_from_slice(&v.to_le_bytes());
        }
        s_rev.push(0x0c);
        let items = vec![
            RawItem::timed(0.0, start),
            RawItem::timed(1.0 * SEC, g_rev(0.0, 0.0, 0.0, 0, 0)),
            RawItem::timed(2.0 * SEC, g_rev(30.327022, 60.063627, 25.5, 701, 56)),
            RawItem::timed(2.5 * SEC, s_rev),
            RawItem::timed(3.0 * SEC, g_rev(30.327522, 60.063527, 26.0, 705, 56)),
        ];
        let env = Env { name: "a.mov".into(), file_times: (0.0, 0.0), tz: 0.0, companion: &|_| None };
        let mut formats = crate::gps::formats::all();
        let loaded = load_items(&mut formats, None, items, 0.0, "mov", 4.0 * SEC, &env).unwrap();
        assert_eq!(loaded.info.format_id, "movRegV");
        assert!(loaded.info.do_not_post_process_fix);
        let log = crate::gps::finish(loaded, (0.0, 0.0)).unwrap();
        let t = log.track;
        assert_eq!(t.gps.len(), 2, "zero placeholder dropped");
        let p = t.gps[0];
        assert_eq!(
            (p.lat, p.lon, p.altitude, p.speed_kmh, p.course, p.time),
            (60.063627, 30.327022, 25.5, 70.1, 112.0, 2.0)
        );
        assert_eq!((t.gsensor[0].time, t.gsensor[0].z), (2.5, 0.98));
    }
}
