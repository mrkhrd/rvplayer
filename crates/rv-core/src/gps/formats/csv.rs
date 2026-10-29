//! CSV as written by the original's "Convert GPS Log" (`UnitGps_CSV.SaveFileV`). The original
//! could only write it; this reader is an addition that reads those files back.
//!
//! Layout: a header line, then one record per line, separated by ';' (when the decimal
//! separator was ',') or ',':
//! - GPS only: `Date;Time;ms;X;Y;H;Speed;Bearing;Trip` (X = longitude, Y = latitude,
//!   H = altitude m, speed km/h; a '?' before X and Y marks a point without a good fix, whose
//!   speed, bearing and trip are left empty);
//! - G-sensor only: `Date;Time;ms;X;Y;Z` (g);
//! - both: a leading `Type` column with "[gps]" or "[gsensor]".
//!
//! Dates and times were written in the writer's locale (`DateToStr` / `TimeToStr`): day-first
//! ("16.05.2016", "16/05/2016"), ISO ("2016-05-16") and month-first ("05/16/2016", when the
//! day-first reading is impossible) dates and 24-hour or AM/PM times are accepted. Times are
//! local wall-clock times, used as they are.

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::util::{
    CharSet, SPLIT_KEEP_EMPTY, encode_date_time, split, split_lines, str_to_float, str_to_int, trim,
};

static SPEC: Spec = Spec { title: "Comma Separated Values", def_ext: "csv", ..super::EMPTY_SPEC };

#[derive(Default)]
pub struct Csv {
    sep: u8,
    /// The header starts with "Type".
    typed: bool,
    /// Without a Type column: the header has an H (GPS) or a Z (G-sensor) column.
    gps_only: bool,
}

pub fn create() -> Box<dyn Format> {
    Box::new(Csv::default())
}

/// (separator, typed, gps-only) of a header line.
fn header(line: &[u8]) -> Option<(u8, bool, bool)> {
    let line = trim(line);
    let typed = line.starts_with(b"Type");
    let rest = if typed { line.get(5..)? } else { line };
    if !rest.starts_with(b"Date") {
        return None;
    }
    let sep = *rest.get(4)?;
    if sep != b';' && sep != b',' {
        return None;
    }
    let cols = split(line, &CharSet::of(&[sep]), SPLIT_KEEP_EMPTY);
    let names: Vec<&[u8]> = cols.iter().map(|c| trim(c)).collect();
    let at = usize::from(typed);
    if names.get(at + 1) != Some(&&b"Time"[..]) || names.get(at + 2) != Some(&&b"ms"[..]) {
        return None;
    }
    Some((sep, typed, names.contains(&&b"H"[..])))
}

/// A locale date: (year, month, day).
fn parse_date(s: &[u8]) -> Option<(i32, u32, u32)> {
    let parts: Vec<i32> = split(trim(s), &CharSet::of(b"./- "), 0).iter().map(|p| str_to_int(p, -1)).collect();
    let [a, b, c] = parts[..] else { return None };
    if a < 0 || b < 0 || c < 0 {
        return None;
    }
    let year = |y: i32| if y < 100 { y + if y < 80 { 2000 } else { 1900 } } else { y };
    let (y, m, d) = if a > 31 {
        (a, b, c)
    } else if b <= 12 {
        (year(c), b, a)
    } else {
        (year(c), a, b)
    };
    Some((y, u32::try_from(m).ok()?, u32::try_from(d).ok()?))
}

/// A locale time: (hour, minute, second).
fn parse_time(s: &[u8]) -> Option<(u32, u32, u32)> {
    let t = trim(s).to_ascii_uppercase();
    let (t, pm) = match (t.strip_suffix(b"PM"), t.strip_suffix(b"AM")) {
        (Some(x), _) => (trim(x).to_vec(), Some(true)),
        (_, Some(x)) => (trim(x).to_vec(), Some(false)),
        _ => (t.clone(), None),
    };
    let parts: Vec<i32> = split(&t, &CharSet::of(b":."), 0).iter().map(|p| str_to_int(p, -1)).collect();
    let (h, m, sec) = match parts[..] {
        [h, m, s] => (h, m, s),
        [h, m] => (h, m, 0),
        _ => return None,
    };
    let mut h = u32::try_from(h).ok()?;
    match pm {
        Some(true) if h < 12 => h += 12,
        Some(false) if h == 12 => h = 0,
        _ => {}
    }
    Some((h, u32::try_from(m).ok()?, u32::try_from(sec).ok()?))
}

/// A coordinate with the "no good fix" mark.
fn coord(s: &[u8]) -> (f64, bool) {
    let s = trim(s);
    match s.strip_prefix(b"?") {
        Some(v) => (str_to_float(v, 0.0), true),
        None => (str_to_float(s, 0.0), false),
    }
}

impl Format for Csv {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, data: &[u8]) -> DataTest {
        let first = split_lines(&data[..data.len().min(0x1000)]).into_iter().next().unwrap_or_default();
        match header(&first) {
            Some((sep, typed, gps_only)) => {
                (self.sep, self.typed, self.gps_only) = (sep, typed, gps_only);
                DataTest::Maybe
            }
            None => DataTest::No,
        }
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        self.sep != 0 && line.contains(&self.sep) && header(line).is_none()
    }

    fn load_item1(&mut self, _ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let cols = split(line, &CharSet::of(&[self.sep]), SPLIT_KEEP_EMPTY);
        let (gps, at) = if self.typed {
            match cols.first().map(|c| trim(c)) {
                Some(b"[gps]") => (true, 1),
                Some(b"[gsensor]") => (false, 1),
                _ => return false,
            }
        } else {
            (self.gps_only, 0)
        };
        let col = |i: usize| cols.get(at + i).map_or(&[][..], |c| c.as_slice());
        let (Some((y, mo, d)), Some((h, mi, s))) = (parse_date(col(0)), parse_time(col(1))) else { return false };
        let ms = u32::try_from(str_to_int(trim(col(2)), 0)).unwrap_or(0).min(999);
        let Some(time) = encode_date_time(y, mo, d, h, mi, s, ms) else { return false };
        if gps {
            let ((x, warn_x), (yv, warn_y)) = (coord(col(3)), coord(col(4)));
            *it = Item {
                pt: PT_GPS,
                time,
                x,
                y: yv,
                z: str_to_float(trim(col(5)), 0.0),
                speed: str_to_float(trim(col(6)), -1.0),
                angle: str_to_float(trim(col(7)), -1.0),
                warn: if warn_x || warn_y { 2 } else { 0 },
            };
        } else {
            let v = |i: usize| str_to_float(trim(col(i)), 0.0);
            *it = Item { pt: PT_GSENSOR, time, x: v(3), y: v(4), z: v(5), ..Default::default() };
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    const BOTH: &str = "Type;Date;Time;ms;X;Y;H;Speed;Bearing;Trip\r\n\
[gps];16.05.2016;13:06:26;000;30,327022;60,063627;12,5;70;112;0;\r\n\
[gsensor];16.05.2016;13:06:26;500;0,01;-0,02;0,98\r\n\
[gps];16.05.2016;13:06:27;000;30,327522;60,063527;13,0;71;112;31;\r\n\
[gsensor];16.05.2016;13:06:27;500;0,02;-0,01;0,97\r\n\
[gps];16.05.2016;13:06:28;000;?30,328022;?60,063427;13,0;;;;\r\n";

    #[test]
    fn reads_the_original_export() {
        let r = load_log("trip.csv", BOTH.as_bytes()).unwrap();
        assert_eq!(r.format, "csv");
        let t = &r.track;
        assert_eq!(t.gps.len(), 3);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert_eq!((p.lon, p.lat, p.altitude, p.speed_kmh, p.course), (30.327022, 60.063627, 12.5, 70.0, 112.0));
        assert_eq!(t.gps[2].warning, 2);
        assert_eq!(t.gsensor.len(), 2);
        assert_eq!((t.gsensor[0].time, t.gsensor[0].z), (0.5, 0.98));
    }

    #[test]
    fn gps_only_with_comma_and_us_dates() {
        let csv = "Date,Time,ms,X,Y,H,Speed,Bearing,Trip\r\n\
                   5/16/2016,1:06:26 PM,0,30.327022,60.063627,12.5,70,112,0,\r\n\
                   5/16/2016,1:06:27 PM,0,30.327522,60.063527,13,71,112,31,\r\n";
        let r = load_log("trip.csv", csv.as_bytes()).unwrap();
        assert_eq!(r.track.gps.len(), 2);
        assert_eq!(r.track.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let iso = "Date;Time;ms;X;Y;Z\r\n2016-05-16;13:06:26;0;0,1;0,2;1\r\n2016-05-16;13:06:27;0;0,1;0,3;1\r\n";
        let r = load_log("g.csv", iso.as_bytes()).unwrap();
        assert_eq!((r.track.gps.len(), r.track.gsensor.len()), (0, 2));
        assert!(load_log("x.csv", b"a;b;c\r\n1;2;3\r\n").is_err());
        fuzz("a.csv", BOTH.as_bytes());
    }
}
