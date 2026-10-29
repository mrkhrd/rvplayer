//! `UnitGps_LogDS10`: "DS 10" logs. The whole file is one line of records that each start
//! with '$' (no line breaks, commas or tabs anywhere):
//! `$GPS<n>N<lat>E<lon>` and `$G_sensor<n>?<x>?<y>?<z>`, where `<n>` is a record counter,
//! `N`/`S` and `E`/`W` are the hemispheres of the following number, and `?` is any separator.
//!
//! Times are the file's last-write time plus `<n>` record steps of 1/83520 day (about 1.03 s).
//! Coordinates are NMEA DDMM.MMMM divided by 100 ("55.301234" = 55 deg 30.1234 min); speed and
//! course are unknown. G-sensor values are 1/100 g offsets from 1 g (`v / 100 + 1`).

use crate::gps::format::{
    Ctx, DataTest, Format, Item, PT_GPS, PT_GSENSOR, RawItem, Spec, check_chars, default_test_load_item,
};
use crate::gps::util::{CharSet, DIGITS, copy, gp_int_to_coordinate, half_sphere_x, half_sphere_y, pos, str_to_float};

static SPEC: Spec = Spec {
    title: "DS 10",
    def_ext: "log",
    format_id: "logDS10",
    must_be_chars_in_data: CharSet::of(b"$G"),
    bad_chars: CharSet::of(b"\t\n\r,;<>[]"),
    ..crate::gps::formats::EMPTY_SPEC
};

/// `oneItemTime`: the time of one record step in days.
const ONE_ITEM_TIME: f64 = 1.0 / 83_520.0;

/// `Split` flag 2 (not supported by `util::split`): every field but the first keeps the
/// separator in front of it.
const SPLIT_WITH_LEADING_SEPARATOR: u32 = 2;

pub struct LogDs10;

pub fn create() -> Box<dyn Format> {
    Box::new(LogDs10)
}

/// `UnitUtilsStr.Split` with the flags 1 (keep empty), 2 (leading separator) and 4 (trailing
/// separator); only flag 2 is used here.
fn split_ex(s: &[u8], separators: &CharSet, flags: u32) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    if s.is_empty() {
        return out;
    }
    let lp1 = s.len() + 1;
    let mut b = 1usize;
    let mut i = 1usize;
    while i <= lp1 {
        let is_sep = if i < lp1 { separators.contains(s[i - 1]) } else { true };
        if is_sep {
            if b > 1 && flags & SPLIT_WITH_LEADING_SEPARATOR != 0 {
                b -= 1;
            }
            let n = (i - b) + usize::from(flags & 4 != 0);
            let val = copy(s, b, n);
            if !val.is_empty() || flags & 1 != 0 {
                out.push(val.to_vec());
            }
            b = i + 1;
            if b >= lp1 {
                break;
            }
        }
        i += 1;
    }
    out
}

/// `TestRaw`: the data has no bad characters, contains '$' and 'G', and one of the record
/// heads with a counter starting with '0' occurs in its first 2000 bytes.
fn test_raw(data: &[u8]) -> bool {
    if !check_chars(data, &SPEC.must_be_chars_in_data, &SPEC.bad_chars) {
        return false;
    }
    let head = copy(data, 1, 2000);
    pos(b"$GPS0", head).is_some() || pos(b"$G_sensor0", head).is_some()
}

/// `ReadNumb`: the run of '+', '-', '.' and digits at the 1-based position `po`; `po` moves
/// past it.
fn read_numb<'a>(line: &'a [u8], po: &mut usize) -> &'a [u8] {
    let index = *po;
    while let Some(&c) = line.get(*po - 1) {
        if !(c == b'+' || c == b'-' || c == b'.' || c.is_ascii_digit()) {
            break;
        }
        *po += 1;
    }
    copy(line, index, *po - index)
}

impl Format for LogDs10 {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn split_raw_data(&mut self, data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
        if !test_raw(data) {
            return None;
        }
        let items =
            split_ex(data, &CharSet::of(b"$"), SPLIT_WITH_LEADING_SEPARATOR).into_iter().map(RawItem::line).collect();
        Some((items, 0.0))
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, data: &[u8]) -> DataTest {
        DataTest::from(test_raw(data))
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        // The original raises a range error when the digit position is past the end.
        let digit_at = |i: usize| line.get(i).is_some_and(|&c| DIGITS.contains(c));
        let id = copy(line, 1, 4);
        let ok = (id == b"$GPS" && digit_at(4)) || (id == b"$G_s" && digit_at(9));
        ok && default_test_load_item(&SPEC, line)
    }

    fn load_item1(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let finish = ctx.info.date_time_finish;
        if copy(line, 1, 4) == b"$GPS" {
            it.pt = PT_GPS;
            let mut po = 5;
            it.time = str_to_float(read_numb(line, &mut po), 0.0) * ONE_ITEM_TIME + finish;
            let tmp = po;
            po += 1;
            it.y = gp_int_to_coordinate(str_to_float(read_numb(line, &mut po), 0.0) * 100.0);
            // The hemisphere letters precede the numbers; past the end the original raises.
            let Some(&ns) = line.get(tmp - 1) else { return false };
            half_sphere_y(&mut it.y, &[ns]);
            let tmp = po;
            po += 1;
            it.x = gp_int_to_coordinate(str_to_float(read_numb(line, &mut po), 0.0) * 100.0);
            let Some(&ew) = line.get(tmp - 1) else { return false };
            half_sphere_x(&mut it.x, &[ew]);
            it.angle = -1.0;
            it.speed = -1.0;
            true
        } else if copy(line, 1, 9) == b"$G_sensor" {
            it.pt = PT_GSENSOR;
            let mut po = 10;
            it.time = str_to_float(read_numb(line, &mut po), 0.0) * ONE_ITEM_TIME + finish;
            let mut axis = || {
                po += 1;
                str_to_float(read_numb(line, &mut po), 0.0) / 100.0 + 1.0
            };
            it.x = axis();
            it.y = axis();
            it.z = axis();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CharSet, split_ex};
    use crate::gps::test_support::*;
    use crate::gps::util::encode_date_time;

    const LOG: &str = "$GPS0000N55.301234E037.301234$G_sensor0000X-5Y3Z-98\
                       $GPS0001N55.301334E037.301434$G_sensor0001X-4Y2Z-97\
                       $GPS0002S55.301434W037.301634$G_sensor0002 -3 1 -96";

    /// One record step in seconds.
    const STEP: f64 = 86_400.0 / 83_520.0;

    #[test]
    fn split_keeps_the_leading_separator() {
        let d = CharSet::of(b"$");
        assert_eq!(split_ex(b"$GPS1$GPS2", &d, 2), [b"$GPS1".to_vec(), b"$GPS2".to_vec()]);
        assert_eq!(split_ex(b"xx$G", &d, 2), [b"xx".to_vec(), b"$G".to_vec()]);
        assert_eq!(split_ex(b"$a$$b$", &d, 2), [b"$a".to_vec(), b"$".to_vec(), b"$b".to_vec()]);
        assert!(split_ex(b"", &d, 2).is_empty());
    }

    #[test]
    fn records_counted_from_the_write_time() {
        let write = encode_date_time(2016, 5, 16, 13, 6, 26, 0).unwrap();
        let r = load_log_at("trip.log", LOG.as_bytes(), (write - 0.01, write)).unwrap();
        assert_eq!(r.format, "logDS10");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        assert_eq!(t.gps.len(), 3);
        let p = t.gps[0];
        assert!((p.lat - (55.0 + 30.1234 / 60.0)).abs() < 1e-9, "{}", p.lat);
        assert!((p.lon - (37.0 + 30.1234 / 60.0)).abs() < 1e-9, "{}", p.lon);
        assert_eq!((p.altitude, p.warning), (0.0, 0));
        assert!((t.gps[1].time - STEP).abs() < 1e-5, "{}", t.gps[1].time);
        assert!((t.gps[2].time - 2.0 * STEP).abs() < 1e-5);
        // S / W negate
        assert!(t.gps[2].lat < -55.5 && t.gps[2].lon < -37.5);
        let g = &t.gsensor;
        assert_eq!(g.len(), 3);
        assert!((g[0].x - 0.95).abs() < 1e-12 && (g[0].y - 1.03).abs() < 1e-12 && (g[0].z - 0.02).abs() < 1e-12);
        assert!((g[2].x - 0.97).abs() < 1e-12 && (g[2].y - 1.01).abs() < 1e-12 && (g[2].z - 0.04).abs() < 1e-12);
        assert!((g[1].time - t.gps[1].time).abs() < 1e-5);
    }

    #[test]
    fn detection_rules() {
        let write = 42_506.5;
        assert!(load_log_at("a.log", LOG.as_bytes(), (write, write)).is_ok());
        // a line break is a bad character
        assert!(load_log_at("a.log", format!("{LOG}\r\n").as_bytes(), (write, write)).is_err());
        // a counter starting with '0' must occur in the first 2000 bytes
        let late = LOG.replace("$GPS0", "$GPS1").replace("$G_sensor0", "$G_sensor1");
        assert!(load_log_at("a.log", late.as_bytes(), (write, write)).is_err());
        // without a file time the records are around 1899
        assert!(load_log("a.log", LOG.as_bytes()).is_err());
        // a record without the hemisphere letter after the counter is dropped
        let cut = "$GPS0000N55.301234E037.301234$GPS0001$GPS0002N55.301434E037.301634";
        let r = load_log_at("a.log", cut.as_bytes(), (write, write)).unwrap();
        assert_eq!(r.track.gps.len(), 2);
        assert!((r.track.gps[1].time - 2.0 * STEP).abs() < 1e-5);
    }

    #[test]
    fn fuzz_logds10() {
        fuzz("a.log", LOG.as_bytes());
    }
}
