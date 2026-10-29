//! `UnitGps_MAPStr`: text records after a 0xFE 0xFE marker, separated by blanks:
//! `RTC:YYYYMMDDhhmmss...` (camera clock), `GPS:...` (fix) and `G-SENSOR:...` (recognised
//! but not read). After the "GPS:" prefix a fix holds (1-based positions):
//!
//! | pos   | content                                   |
//! |-------|-------------------------------------------|
//! | 1-14  | GPS time `YYYYMMDDhhmmss`                 |
//! | 16    | N/S                                       |
//! | 17-24 | latitude DDMMSSss (degrees, minutes, 1/100 s) |
//! | 25    | E/W                                       |
//! | 26-34 | longitude DDDMMSSss                       |
//! | 40-42 | speed, integer km/h                       |
//!
//! No time zone is applied: once an RTC record has been seen, the difference between the
//! camera clock and the first fix after it is added to every fix (before that, fixes keep
//! the GPS time as written).

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, RawItem, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::formats::map::time_text;
use crate::gps::util::{copy, half_sphere_x, half_sphere_y, pos, str_date_time_to_date_time, str_to_int};

static SPEC: Spec = Spec { title: "MAPStr", ..EMPTY_SPEC };

/// `startPrefix`.
const START_PREFIX: &[u8] = b"\xfe\xfe";
const RTC_PREFIX: &[u8] = b"RTC:";
const GPS_PREFIX: &[u8] = b"GPS:";
const GSENSOR_PREFIX: &[u8] = b"G-SENSOR:";

fn is_prefix(s: &[u8]) -> bool {
    s == RTC_PREFIX || s == GPS_PREFIX || s == GSENSOR_PREFIX
}

pub struct MapStr {
    /// `rtcTime`: the last camera clock read, 0 = none.
    rtc_time: f64,
    /// `deltaTime`: camera clock minus GPS time, 0 until known.
    delta_time: f64,
}

pub fn create() -> Box<dyn Format> {
    Box::new(MapStr { rtc_time: 0.0, delta_time: 0.0 })
}

/// `GetTime`: `YYYYMMDDhhmmss` at 1-based `offs`; 0 unless the year is 1970..2049 and the
/// text a valid date; `None` where the original raises (text cut short).
fn get_time(data: &[u8], offs: usize) -> Option<f64> {
    let y = str_to_int(copy(data, offs, 4), 0);
    if y > 1969 && y < 2050 {
        return Some(str_date_time_to_date_time(&time_text(data, offs)?));
    }
    Some(0.0)
}

/// `GetCoord`: `size` digits at 1-based `offs` as DDDMMSSss.
fn get_coord(data: &[u8], offs: usize, size: usize) -> f64 {
    let v = str_to_int(copy(data, offs, size), 0);
    let sec = f64::from(v % 10000) / 6000.0;
    let v = v / 10000;
    let min = f64::from(v % 100);
    (sec + min) / 60.0 + f64::from(v / 100)
}

/// `MySubStrCmp`: `s` occurs at 1-based `i` of `data`. The original raises when the
/// comparison runs past the end of the data; that is a mismatch here.
fn sub_str_at(data: &[u8], i: usize, s: &[u8]) -> bool {
    data.get(i.wrapping_sub(1)..).is_some_and(|d| d.starts_with(s))
}

impl Format for MapStr {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    /// After the first 0xFE 0xFE: every record from a prefix up to the next blank.
    fn split_raw_data(&mut self, data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
        let n = data.len();
        if n <= 0x32 {
            return None;
        }
        let mut i = pos(START_PREFIX, data)? + 1 + START_PREFIX.len();
        let mut items = Vec::new();
        loop {
            while i < n
                && !sub_str_at(data, i, RTC_PREFIX)
                && !sub_str_at(data, i, GPS_PREFIX)
                && !sub_str_at(data, i, GSENSOR_PREFIX)
            {
                i += 1;
            }
            if n <= i {
                break;
            }
            let mut i2 = i;
            while i2 <= n && data[i2 - 1] != b' ' {
                i2 += 1;
            }
            items.push(RawItem::line(copy(data, i, i2 - i)));
            i = i2 + 1;
            if i >= n {
                break;
            }
        }
        (!items.is_empty()).then_some((items, 0.0))
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {
        self.rtc_time = 0.0;
        self.delta_time = 0.0;
    }

    /// 0xFE 0xFE among the first 99 bytes, directly followed by one of the prefixes.
    fn test_load_data(&mut self, _ctx: &mut Ctx, data: &[u8]) -> DataTest {
        let Some(p) = pos(START_PREFIX, data).map(|p| p + 1) else { return DataTest::No };
        if p >= 100 {
            return DataTest::No;
        }
        let i = p + START_PREFIX.len();
        let mut i2 = i;
        while i2 < 100 {
            match data.get(i2 - 1) {
                // a range error in the original
                None => return DataTest::No,
                Some(b':') => break,
                Some(_) => i2 += 1,
            }
        }
        DataTest::from(is_prefix(copy(data, i, i2 + 1 - i)))
    }

    /// One of the prefixes, the ':' among the first 9 characters.
    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        let mut i = 1;
        loop {
            match line.get(i - 1) {
                // a range error in the original
                None => return false,
                Some(b':') => break,
                Some(_) => {}
            }
            i += 1;
            if i > 9 {
                return false;
            }
        }
        is_prefix(&line[..i])
    }

    fn load_item1(&mut self, _ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let id = copy(line, 1, pos(b":", line).map_or(0, |p| p + 1));
        let data = copy(line, id.len() + 1, line.len());
        if id == RTC_PREFIX {
            if let Some(t) = get_time(data, 1) {
                self.rtc_time = t;
            }
            return false;
        }
        if id != GPS_PREFIX {
            return false;
        }
        it.pt = PT_GPS;
        let Some(t) = get_time(data, 1) else { return false };
        it.time = t;
        if it.time <= 2.0 {
            return false;
        }
        if self.delta_time == 0.0 && self.rtc_time > 2.0 {
            self.delta_time = self.rtc_time - it.time;
        }
        it.time += self.delta_time;
        // the original raises reading the hemispheres of a record cut short
        let (Some(&ns), Some(&ew)) = (data.get(0xf), data.get(0x18)) else { return false };
        it.y = get_coord(data, 0x11, 8);
        half_sphere_y(&mut it.y, &[ns]);
        it.x = get_coord(data, 0x1a, 9);
        half_sphere_x(&mut it.x, &[ew]);
        it.z = 0.0;
        it.speed = f64::from(str_to_int(copy(data, 0x28, 3), 0));
        it.angle = -1.0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::test_support::*;

    fn fix(time: &str, lat: &str, lon: &str, speed: &str) -> String {
        format!("GPS:{time}AN{lat}E{lon}xxxxx{speed}")
    }

    fn log(rtc: bool) -> Vec<u8> {
        let mut recs = Vec::new();
        if rtc {
            recs.push("RTC:20160516160626".to_string());
        }
        recs.push(fix("20160516130626", "55451234", "037351234", "060"));
        recs.push("G-SENSOR:+012-034+098".to_string());
        recs.push(fix("20160516130627", "55451334", "037351334", "061"));
        recs.push("RTC:20160516170000".to_string());
        recs.push(fix("20160516130628", "55451434", "037351434", "062"));
        let mut data = b"\xfe\xfe".to_vec();
        data.extend_from_slice(recs.join(" ").as_bytes());
        data.extend_from_slice(b"\r\n");
        data
    }

    #[test]
    fn fixes_on_camera_clock() {
        let r = load_log("a.map", &log(true)).unwrap();
        assert_eq!(r.format, "MAPStr");
        let t = &r.track;
        assert_eq!(t.gps.len(), 3, "{:?}", t.gps);
        // the first fix after the RTC record defines the shift; later RTC records do not
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 16:06:26");
        let times: Vec<f64> = t.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [0.0, 1.0, 2.0]);
        let p = t.gps[0];
        assert!((p.lat - (55.0 + (45.0 + 12.34 / 60.0) / 60.0)).abs() < 1e-9, "{}", p.lat);
        assert!((p.lon - (37.0 + (35.0 + 12.34 / 60.0) / 60.0)).abs() < 1e-9, "{}", p.lon);
        let speeds: Vec<f64> = t.gps.iter().map(|p| p.speed_kmh).collect();
        assert_eq!(speeds, [60.0, 61.0, 62.0]);
        assert!(t.gsensor.is_empty());
    }

    #[test]
    fn fixes_without_rtc_keep_gps_time() {
        let r = load_log("a.map", &log(false)).unwrap();
        assert_eq!(r.track.start.unwrap().format("%H:%M:%S").to_string(), "13:06:26");
        // the RTC record after the first fix shifts the following ones
        let times: Vec<f64> = r.track.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [0.0, 1.0, 3.0 * 3600.0 + 53.0 * 60.0 + 34.0]);
    }

    #[test]
    fn detection() {
        let mut f = MapStr { rtc_time: 0.0, delta_time: 0.0 };
        let mut ctx = Ctx::default();
        assert_eq!(f.test_load_data(&mut ctx, &log(true)), DataTest::Maybe);
        assert_eq!(f.test_load_data(&mut ctx, b"\xfe\xfeXYZ:abc"), DataTest::No);
        assert_eq!(f.test_load_data(&mut ctx, b"\xfe\xfeRTC"), DataTest::No, "range error");
        let mut late = vec![b' '; 100];
        late.extend_from_slice(&log(true));
        assert_eq!(f.test_load_data(&mut ctx, &late), DataTest::No);
        assert!(f.test_load_item(&mut ctx, b"G-SENSOR:+1"));
        assert!(!f.test_load_item(&mut ctx, b"GPS"));
        assert!(!f.test_load_item(&mut ctx, b"0123456789:"));
        // a partial prefix at the end of the data is not a record
        let mut d = log(true);
        d.truncate(d.len() - 2);
        let n = f.split_raw_data(&d).unwrap().0.len();
        d.extend_from_slice(b" GP");
        assert_eq!(f.split_raw_data(&d).unwrap().0.len(), n);
    }

    #[test]
    fn fuzz_mapstr() {
        fuzz("a.map", &log(true));
    }
}
