//! `UnitGps_MAP`: binary "map800" logs (`.map`, every byte XOR 0xAA) and their plain variant
//! "gmap" (`.gmap`). The file is a sequence of 251-byte records, each starting with two
//! 0xFE bytes ("TT" when encoded). After decoding, a record holds (1-based positions):
//!
//! | pos   | content                                                         |
//! |-------|-----------------------------------------------------------------|
//! | 1-2   | 0xFE 0xFE                                                       |
//! | 3-16  | camera clock `YYYYMMDDhhmmss` (t2)                              |
//! | 17    | separator (not a digit)                                         |
//! | 18-31 | GPS time `YYYYMMDDhhmmss` (t1), zeros without a fix             |
//! | 32    | 0x03 = good fix, anything else marks the point suspicious       |
//! | 33    | N/S (0xE4, an encoded 'N', also counts as N)                    |
//! | 34-41 | latitude DDMMmmmm (DDMM.mmmm)                                   |
//! | 42    | E/W                                                             |
//! | 43-51 | longitude DDDMMmmmm                                             |
//! | 60-   | G-sensor groups of 12 chars "+xxx+zzz+yyy" (1/100 g), y negated |
//!
//! Both times are treated as UTC and get the local time-zone offset. A record without GPS
//! time takes the camera time minus the camera-GPS difference seen on the first record with
//! both (points loaded before that difference is known are shifted by it then). The format
//! replaces `LoadItem`: no validation or duplicate check; trailing all-zero G-sensor groups
//! are dropped.

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, PT_GSENSOR, RawItem, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{
    CharSet, DIGITS, copy, gp_int_to_coordinate, half_sphere_x, half_sphere_y, str_date_time_to_date_time,
    str_to_float, str_to_int,
};

static SPEC: Spec = Spec { title: "MAP", def_ext: "map", format_id: "map800", ..EMPTY_SPEC };
/// The format id after reading a record that was not encoded (`DecodeLine`).
static SPEC_GMAP: Spec = Spec { title: "MAP", def_ext: "map", format_id: "gmap", ..EMPTY_SPEC };

/// `startChar`.
const START_CHAR: u8 = 0xfe;
const XOR_KEY: u8 = 0xaa;

/// `AddXor(set, $AA)`: the characters plus each of them XOR 0xAA (at most 16 characters).
const fn add_xor(chars: &[u8]) -> CharSet {
    let mut buf = [0u8; 32];
    let mut i = 0;
    while i < 16 {
        let c = chars[if i < chars.len() { i } else { 0 }];
        buf[2 * i] = c;
        buf[2 * i + 1] = c ^ XOR_KEY;
        i += 1;
    }
    CharSet::of(&buf)
}

/// `startCh`: [startChar] + XOR.
const START_CH: CharSet = add_xor(&[START_CHAR]);
/// `digitCh`: digits + XOR.
const DIGIT_CH: CharSet = add_xor(b"0123456789");
/// `digit12`: the first digit of a year ('1', '2') + XOR.
const DIGIT12: CharSet = add_xor(b"12");

/// Record length taken by `SplitRawData`.
const RECORD: usize = 0xfb;

pub struct Map {
    /// `deltaTime`: camera clock minus GPS time, -1 until known.
    delta_time: f64,
    /// The last decoded record was plain ("gmap") rather than XOR-encoded ("map800").
    gmap: bool,
}

pub fn create() -> Box<dyn Format> {
    Box::new(Map { delta_time: -1.0, gmap: false })
}

/// `GetTime`: `YYYYMMDDhhmmss` at 1-based `offs` of `data`. A year from 2 to 2011 gets its
/// first three digits replaced by "200" (in `data`, like the original). 0 when the year is
/// outside 2011..2049 or the text is not a date; `None` where the original raises (the
/// text is cut short).
fn get_time(data: &mut [u8], offs: usize) -> Option<f64> {
    let mut y = str_to_int(copy(data, offs, 4), 0);
    if y > 1 && y < 2012 {
        let fix = data.get_mut(offs.checked_sub(1)?..offs + 2)?;
        fix.copy_from_slice(b"200");
        y = 2011;
    }
    if y > 2010 && y < 2050 {
        return Some(str_date_time_to_date_time(&time_text(data, offs)?));
    }
    Some(0.0)
}

/// The `A(...)` calls of `GetTime` (MAP and MAPStr): "YYYY-MM-DD-hh-mm-ss-" from the 14
/// characters at 1-based `offs`; `None` (a range error in the original) when they are not
/// all there.
pub(crate) fn time_text(data: &[u8], offs: usize) -> Option<Vec<u8>> {
    let src = data.get(offs.checked_sub(1)?..offs + 13)?;
    let mut s = Vec::with_capacity(20);
    for (at, n) in [(0, 4), (4, 2), (6, 2), (8, 2), (10, 2), (12, 2)] {
        s.extend_from_slice(&src[at..at + n]);
        s.push(b'-');
    }
    Some(s)
}

/// `GetCoord`: `size` digits at 1-based `offs`, DDDMM.mmmm * 10000.
fn get_coord(data: &[u8], offs: usize, size: usize) -> f64 {
    let v = str_to_int(copy(data, offs, size), 0);
    gp_int_to_coordinate(f64::from(v) / 10000.0)
}

/// `ReadX`: "+ddd" / "-ddd" at 1-based `p`, in 1/100 g.
fn read_x(data: &[u8], p: usize) -> Option<f64> {
    let s = copy(data, p, 4);
    let ok = s.len() == 4 && matches!(s[0], b'+' | b'-') && s[1..].iter().all(|&c| DIGITS.contains(c));
    ok.then(|| str_to_float(s, 0.0) / 100.0)
}

/// Two equal start characters at 1-based `p`, `p + 1`.
fn is_start(data: &[u8], p: usize) -> bool {
    match (data.get(p.wrapping_sub(1)), data.get(p)) {
        (Some(&a), Some(&b)) => a == b && START_CH.contains(a),
        _ => false,
    }
}

/// The byte at 1-based `p` is in `set`.
fn byte_in(data: &[u8], p: usize, set: &CharSet) -> bool {
    data.get(p.wrapping_sub(1)).is_some_and(|&c| set.contains(c))
}

impl Format for Map {
    fn spec(&self) -> &Spec {
        if self.gmap { &SPEC_GMAP } else { &SPEC }
    }

    fn all_ext(&self) -> Vec<&'static str> {
        vec!["map", "gmap"]
    }

    /// Finds the first record start among the first 47 bytes (start pair, 14 digits, a
    /// non-digit), then cuts 251-byte records, expecting the next one right after or
    /// searching for it from 235 bytes on (up to 267).
    fn split_raw_data(&mut self, data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
        let n = data.len();
        if n <= 100 {
            return None;
        }
        let first = (1..0x30).find(|&p| {
            is_start(data, p) && (2..0x10).all(|i| byte_in(data, p + i, &DIGIT_CH)) && !byte_in(data, p + 16, &DIGIT_CH)
        });
        let mut p = first?;
        let mut items = Vec::new();
        let mut p_end = p + 16;
        while p + 4 < n && p <= p_end {
            if is_start(data, p) && byte_in(data, p + 2, &DIGIT12) {
                items.push(RawItem::line(copy(data, p, RECORD)));
                let next = p + RECORD;
                p_end = p + 0x10b;
                if next + 4 > n {
                    break;
                }
                p = if next + 0xeb < n && !is_start(data, next) { p + 0xeb } else { next };
            } else {
                p += 1;
            }
        }
        Some((items, 0.0))
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {
        self.delta_time = -1.0;
    }

    /// Two equal start characters among the first 513 bytes.
    fn test_load_data(&mut self, _ctx: &mut Ctx, data: &[u8]) -> DataTest {
        let m = data.len().saturating_sub(1).min(0x200);
        DataTest::from((1..=m).any(|i| is_start(data, i)))
    }

    /// More than 32 bytes with a record start (start pair and '1'/'2') at one of the first
    /// 16 positions; the record's line is cut to begin there.
    fn test_load_item(&mut self, ctx: &mut Ctx, line: &[u8]) -> bool {
        if line.len() <= 0x20 {
            return false;
        }
        let Some(i) = (1..=16).find(|&i| is_start(line, i) && byte_in(line, i + 2, &DIGIT12)) else {
            return false;
        };
        if i != 1
            && let Some(item) = ctx.items.get_mut(ctx.index)
        {
            item.line = copy(line, i, line.len()).to_vec();
        }
        true
    }

    fn load_item(&mut self, ctx: &mut Ctx, line: &[u8]) -> bool {
        // DecodeLine, on the line as cut by TestLoadItem
        let mut data = ctx.items.get(ctx.index).map_or_else(|| line.to_vec(), |i| i.line.clone());
        self.gmap = data.first() == Some(&START_CHAR);
        if !self.gmap {
            let key = data.first().map_or(0, |&b| b ^ START_CHAR);
            for b in &mut data {
                *b ^= key;
            }
        }
        let Some(t1) = get_time(&mut data, 0x12) else { return false };
        let Some(t2) = get_time(&mut data, 3) else { return false };
        if t1 > 2.0 && t2 > 2.0 {
            if self.delta_time == -1.0 {
                for p in ctx.points.iter_mut().filter(|p| p.time > 2.0) {
                    p.time -= t2 - t1;
                }
            }
            self.delta_time = t2 - t1;
        }
        let mut gps = Item { pt: PT_GPS, ..Default::default() };
        if t1 > 2.0 {
            gps.time = t1;
        } else if t2 > 2.0 {
            gps.time = if self.delta_time == -1.0 { t2 } else { t2 - self.delta_time };
            gps.warn = 2;
        }
        let mut result = false;
        if gps.time > 2.0 {
            // the original raises reading the flags of a record cut short
            let (Some(&fix), Some(&ns), Some(&ew)) = (data.get(0x1f), data.get(0x20), data.get(0x29)) else {
                return false;
            };
            gps.time += ctx.tz;
            gps.y = get_coord(&data, 0x22, 8);
            half_sphere_y(&mut gps.y, &[if ns == 0xe4 { b'N' } else { ns }]);
            gps.x = get_coord(&data, 0x2b, 9);
            half_sphere_x(&mut gps.x, &[ew]);
            gps.z = 0.0;
            gps.speed = -1.0;
            gps.angle = -1.0;
            if fix != 3 {
                gps.warn = 2;
            }
            ctx.points.push(gps);
            result = true;
        }
        let mut gs: Vec<Item> = Vec::new();
        let mut keep = 0;
        let mut i = 0x3c;
        while data.len() >= i + 12 {
            let (Some(x), Some(z), Some(y)) = (read_x(&data, i), read_x(&data, i + 4), read_x(&data, i + 8)) else {
                break;
            };
            let y = -y;
            gs.push(Item { pt: PT_GSENSOR, x, y, z, ..Default::default() });
            if x != 0.0 || y != 0.0 || z != 0.0 {
                result = true;
                keep = gs.len();
            }
            i += 12;
        }
        gs.truncate(keep);
        ctx.points.extend(gs);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::test_support::*;
    use crate::gps::util::SEC;

    /// A decoded record: camera time, GPS time, fix byte, hemispheres, coordinates and
    /// G-sensor groups, padded with blanks to 251 bytes.
    #[allow(clippy::too_many_arguments)]
    fn record(t2: &str, t1: &str, fix: u8, ns: u8, lat: &str, ew: u8, lon: &str, gs: &[&str]) -> Vec<u8> {
        let mut r = vec![START_CHAR, START_CHAR];
        r.extend_from_slice(t2.as_bytes());
        r.push(b'A');
        r.extend_from_slice(t1.as_bytes());
        r.push(fix);
        r.push(ns);
        r.extend_from_slice(lat.as_bytes());
        r.push(ew);
        r.extend_from_slice(lon.as_bytes());
        assert_eq!(r.len(), 51);
        r.resize(59, b'0');
        for g in gs {
            r.extend_from_slice(g.as_bytes());
        }
        r.resize(RECORD, b' ');
        r
    }

    fn encode(r: &[u8]) -> Vec<u8> {
        r.iter().map(|&b| b ^ XOR_KEY).collect()
    }

    const NO_TIME: &str = "00000000000000";

    fn records() -> Vec<Vec<u8>> {
        vec![
            // no GPS time yet: camera time, suspicious
            record(
                "20160516130626",
                NO_TIME,
                0,
                b'N',
                "55451234",
                b'E',
                "037351234",
                &["+012-034+098", "+000+000+000", "+011-033+097", "+000+000+000"],
            ),
            // GPS time 3 h behind the camera clock
            record("20160516130627", "20160516100627", 3, b'N', "55451334", b'E', "037351334", &["-005+001-100"]),
            // GPS time lost again
            record("20160516130628", NO_TIME, 3, b'S', "55451434", b'W', "037351434", &[]),
        ]
    }

    fn file(records: &[Vec<u8>], enc: bool) -> Vec<u8> {
        let mut data = b"HDR01".to_vec();
        for r in records {
            data.extend(if enc { encode(r) } else { r.clone() });
        }
        data
    }

    fn split_count(data: &[u8]) -> Option<usize> {
        let mut f = Map { delta_time: -1.0, gmap: false };
        f.split_raw_data(data).map(|(items, _)| items.len())
    }

    #[test]
    fn map800_records() {
        let r = load_log("trip.map", &file(&records(), true)).unwrap();
        assert_eq!(r.format, "map800");
        let t = &r.track;
        assert_eq!(t.gps.len(), 3, "{:?}", t.gps);
        // the first fix is moved to GPS time once the camera-GPS difference is known
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 10:06:26");
        let times: Vec<f64> = t.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [0.0, 1.0, 2.0]);
        let warn: Vec<u8> = t.gps.iter().map(|p| p.warning).collect();
        assert_eq!(warn, [2, 0, 2]);
        let p = t.gps[0];
        assert!((p.lat - (55.0 + 45.1234 / 60.0)).abs() < 1e-9, "{}", p.lat);
        assert!((p.lon - (37.0 + 35.1234 / 60.0)).abs() < 1e-9, "{}", p.lon);
        assert_eq!(p.altitude, 0.0);
        assert!(t.gps[2].lat < 0.0 && t.gps[2].lon < 0.0, "S/W");
        // trailing zero group dropped, the zero group between samples kept; y negated
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.12, -0.98, -0.34), (0.0, 0.0, 0.0), (0.11, -0.97, -0.33), (-0.05, 1.0, 0.01)]);
    }

    #[test]
    fn gmap_plain_records() {
        let mut recs = records();
        // an encoded 'N' in plain data counts as N
        recs[0][32] = 0xe4;
        let r = load_log("trip.gmap", &file(&recs, false)).unwrap();
        assert_eq!(r.format, "gmap");
        assert_eq!(r.track.gps.len(), 3);
        assert!(r.track.gps[0].lat > 55.0);
    }

    #[test]
    fn records_inside_text_lines() {
        // records not at the start of the file: SplitRawData does not find them, the text
        // lines are cut at the record start by TestLoadItem
        let mut data = vec![b'#'; 60];
        data.push(b'\n');
        for r in records() {
            data.extend_from_slice(b"ab");
            data.extend(r);
            data.extend_from_slice(b"\r\n");
        }
        assert_eq!(split_count(&data), None);
        let r = load_log("trip.gmap", &data).unwrap();
        assert_eq!(r.format, "gmap");
        assert_eq!(r.track.gps.len(), 3);
        assert_eq!(r.track.gsensor.len(), 4);
    }

    #[test]
    fn time_zone_and_year_fix() {
        let data = file(&records(), true);
        let opts = crate::gps::LoadOptions { tz: 3.0 / 24.0, file_times: (0.0, 0.0) };
        let r = crate::gps::load_log_bytes("a.map", &data, &opts, &|_| None).unwrap();
        assert_eq!(r.track.start.unwrap().format("%H:%M:%S").to_string(), "13:06:26");
        // years 2..2011 become "200x"
        let mut d = b"xx20110102030405".to_vec();
        let t = get_time(&mut d, 3).unwrap();
        assert_eq!(&d[2..6], b"2001");
        let jan_2_2001 = 36_893.0;
        assert!((t - (jan_2_2001 + (3.0 * 3600.0 + 4.0 * 60.0 + 5.0) * SEC)).abs() < 1e-9);
        assert_eq!(get_time(&mut b"xx20500102030405".to_vec(), 3), Some(0.0));
        assert_eq!(get_time(&mut b"xx2016010203".to_vec(), 3), None, "cut short");
    }

    #[test]
    fn split_resyncs_after_junk() {
        let recs = records();
        let mut data = file(&recs[..1], true);
        data.extend_from_slice(&[0x11; 7]);
        data.extend(encode(&recs[1]));
        data.extend(encode(&recs[2]));
        assert_eq!(split_count(&data), Some(3));
        assert_eq!(split_count(&[b'x'; 300]), None);
        assert!(load_log("a.map", b"plain text without records\n").is_err());
    }

    #[test]
    fn fuzz_map() {
        fuzz("a.map", &file(&records(), true));
        fuzz("a.gmap", &file(&records(), false));
    }
}
