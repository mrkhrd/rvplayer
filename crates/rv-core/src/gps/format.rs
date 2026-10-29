//! The format interface (`UnitGpsFormat.IGpsInfoFormat`) and the load context the formats work
//! on (`TGPSLoadContext`, `TTrackInfoItem`).

use super::util::{CharSet, lower, pos};

/// One loaded record (`TTrackInfoItem`). The original overlays the GPS and G-sensor records;
/// this keeps the same field names so the formats read like the original:
/// GPS: `x` = longitude, `y` = latitude, `z` = altitude (m); G-sensor: `x`, `y`, `z` in g.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Item {
    /// 0 none, [`PT_GPS`] or [`PT_GSENSOR`].
    pub pt: u8,
    /// Local wall-clock TDateTime (days since 1899-12-30); 0 = none, -1 = unknown.
    pub time: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Course in degrees, -1 = unknown (GPS only).
    pub angle: f64,
    /// Speed in km/h, -1 = unknown (GPS only).
    pub speed: f64,
    /// 0 good fix, 2 no fix / suspicious (GPS only).
    pub warn: u8,
}

pub const PT_GPS: u8 = 1;
pub const PT_GSENSOR: u8 = 2;

impl Item {
    pub fn gps(time: f64, lon: f64, lat: f64) -> Item {
        Item { pt: PT_GPS, time, x: lon, y: lat, angle: -1.0, speed: -1.0, ..Default::default() }
    }

    pub fn gsensor(time: f64, x: f64, y: f64, z: f64) -> Item {
        Item { pt: PT_GSENSOR, time, x, y, z, ..Default::default() }
    }

    /// `IsTrackPointDataValid` (UnitUtilsMap), including its fixes: a GPS date in 1970 (GPS week
    /// roll-over) moves forward by 14640 days; a coordinate without a fractional part marks
    /// the point as suspicious (warn 2).
    pub fn is_valid(&mut self) -> bool {
        match self.pt {
            PT_GPS => {
                if !self.time.is_finite() {
                    return false;
                }
                if self.time > 25569.0 && self.time < 25934.0 {
                    self.time += 14640.0;
                }
                if (self.time <= 35000.0 || self.time >= 70000.0) && self.time != -1.0 {
                    return false;
                }
                if !(self.x.abs() <= 180.0 && self.y.abs() <= 90.0) {
                    return false;
                }
                if self.x.fract() == 0.0 || self.y.fract() == 0.0 {
                    self.warn = 2;
                }
                true
            }
            PT_GSENSOR => {
                if self.x == 0.0 && self.y == 0.0 && self.z == 0.0 {
                    return false;
                }
                self.x.abs() < 10.0 && self.y.abs() < 10.0 && self.z.abs() < 10.0
            }
            _ => false,
        }
    }
}

/// One input record (`TGPSRawItem`): a text line or a binary record.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawItem {
    /// Video time of the record in days (embedded tracks); -1 / 0 when the source has none.
    pub frame_time: f64,
    pub line: Vec<u8>,
    pub gps_count: usize,
    /// Index into the loaded points of this record's GPS fix, if any.
    pub gps_index: Option<usize>,
}

impl RawItem {
    pub fn line(line: impl Into<Vec<u8>>) -> RawItem {
        RawItem { frame_time: 0.0, line: line.into(), gps_count: 0, gps_index: None }
    }

    pub fn timed(frame_time: f64, line: impl Into<Vec<u8>>) -> RawItem {
        RawItem { frame_time, line: line.into(), gps_count: 0, gps_index: None }
    }
}

/// `TTrackInfo`: what is known about the source while loading.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Info {
    /// Path of the log or video (used to find companion files such as `.3gf`).
    pub name: String,
    /// File-system creation and last-write times (local TDateTime), or start = finish - duration.
    pub date_time_start: f64,
    pub date_time_finish: f64,
    pub format_id: String,
    pub do_not_post_process_fix: bool,
    pub parking_mode: bool,
}

/// `TGPSLoadContext` plus the loaded points (`info.points[0..len]`).
#[derive(Debug, Default)]
pub struct Ctx {
    pub index: usize,
    pub items: Vec<RawItem>,
    /// Source duration in days (videos), 0 for logs.
    pub duration: f64,
    pub frame0_time: f64,
    /// Whole file contents for logs; `None` for records taken from a video.
    pub rawdata: Option<Vec<u8>>,
    pub info: Info,
    pub points: Vec<Item>,
    pub last_gps_index: Option<usize>,
    /// Local time zone offset in days, added to UTC times (`timeZoneLocalSystem`).
    pub tz: f64,
    /// User clock shift in days (`timeZoneUserShift`), 0 unless the user set a time zone.
    pub tz_user_shift: f64,
}

impl Ctx {
    pub fn item(&self) -> &RawItem {
        &self.items[self.index]
    }

    pub fn frame_time(&self) -> f64 {
        self.items.get(self.index).map_or(0.0, |i| i.frame_time)
    }
}

/// The static description of a format (fields of `IGpsInfoFormat` set in `Create`).
#[derive(Debug, Clone, Default)]
pub struct Spec {
    pub title: &'static str,
    pub def_ext: &'static str,
    /// Defaults to `def_ext`, then `title`.
    pub format_id: &'static str,
    pub must_be_chars_in_line: CharSet,
    pub must_be_chars_in_data: CharSet,
    pub bad_chars: CharSet,
    /// Lower-case strings that must all occur in the first 4096 bytes of the data.
    pub must_be_strings_in_data: &'static [&'static str],
    pub data_prefix: &'static [u8],
    pub line_prefix: &'static [u8],
}

impl Spec {
    pub fn id(&self) -> &'static str {
        if !self.format_id.is_empty() {
            self.format_id
        } else if !self.def_ext.is_empty() {
            self.def_ext
        } else {
            self.title
        }
    }
}

/// `IGpsInfoFormat.CheckChars`: trailing NULs are ignored; no byte may be in `bad`; every
/// character of `must` must occur.
pub fn check_chars(data: &[u8], must: &CharSet, bad: &CharSet) -> bool {
    let n = data.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
    let data = &data[..n];
    if !bad.is_empty() && data.iter().any(|&c| bad.contains(c)) {
        return false;
    }
    if !must.is_empty() {
        let mut left = *must;
        for &c in data {
            if left.contains(c) {
                left.remove(c);
                if left.is_empty() {
                    break;
                }
            }
        }
        if !left.is_empty() {
            return false;
        }
    }
    true
}

/// `IGpsInfoFormat.TestLoadData`.
pub fn default_test_load_data(spec: &Spec, data: &[u8]) -> bool {
    if !spec.data_prefix.is_empty() && !data.starts_with(spec.data_prefix) {
        return false;
    }
    if !check_chars(data, &spec.must_be_chars_in_data, &spec.bad_chars) {
        return false;
    }
    if !spec.must_be_strings_in_data.is_empty() {
        let head = lower(&data[..data.len().min(0x1000)]);
        return spec.must_be_strings_in_data.iter().all(|s| pos(s.as_bytes(), &head).is_some());
    }
    true
}

/// `IGpsInfoFormat.TestLoadItem`.
pub fn default_test_load_item(spec: &Spec, line: &[u8]) -> bool {
    if !spec.line_prefix.is_empty() && !line.starts_with(spec.line_prefix) {
        return false;
    }
    check_chars(line, &spec.must_be_chars_in_line, &spec.bad_chars)
}

/// A GPS log or embedded-data format (`IGpsInfoFormat`). Instances hold per-load state and
/// are created fresh for every load.
pub trait Format: Send {
    fn spec(&self) -> &Spec;

    /// `GetAllExt`: extensions this format claims.
    fn all_ext(&self) -> Vec<&'static str> {
        let e = self.spec().def_ext;
        if e.is_empty() { vec![] } else { vec![e] }
    }

    /// `SplitRawData`: the format's own record splitting of a whole file, with the time of
    /// video frame 0 if the data has one. `None` = not this format's data.
    fn split_raw_data(&mut self, _data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
        None
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {}

    fn after_load_data(&mut self, _ctx: &mut Ctx) {}

    /// `TestLoadData`: whether the whole data may be this format. A format that loads the
    /// data by itself (GPX, KML) fills `ctx.points` and returns [`DataTest::Loaded`].
    fn test_load_data(&mut self, _ctx: &mut Ctx, data: &[u8]) -> DataTest {
        DataTest::from(default_test_load_data(self.spec(), data))
    }

    /// `TestLoadItem`.
    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        default_test_load_item(self.spec(), line)
    }

    /// `LoadItem2`: up to two records from one input record; returns which were filled.
    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it1: &mut Item, _it2: &mut Item) -> (bool, bool) {
        (self.load_item1(ctx, line, it1), false)
    }

    /// `LoadItem1`: one record from one input record.
    fn load_item1(&mut self, _ctx: &mut Ctx, _line: &[u8], _it: &mut Item) -> bool {
        false
    }

    /// `LoadItem`: loads and validates the records of the current input record and appends
    /// them to `ctx.points`. Override only when the format replaces the whole step (MAP).
    fn load_item(&mut self, ctx: &mut Ctx, line: &[u8]) -> bool {
        default_load_item(self, ctx, line)
    }

    /// `doPostProcessFix`: false skips the clean-up passes for this format's data.
    fn do_post_process_fix(&self) -> bool {
        true
    }

    /// `isAllowFracSeconds`.
    fn is_allow_frac_seconds(&self) -> bool {
        true
    }
}

/// Result of [`Format::test_load_data`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataTest {
    No,
    /// The data may be this format: its lines are offered to it.
    Maybe,
    /// The format loaded the whole data itself (`prevLoader` set in `TestLoadData`).
    Loaded,
}

impl From<bool> for DataTest {
    fn from(b: bool) -> DataTest {
        if b { DataTest::Maybe } else { DataTest::No }
    }
}

/// Fixes closer than this in time (0.9 s, days) and position are duplicates.
const DUP_TIME: f64 = 1.0 / 96_000.0;
const DUP_DEG: f64 = 8.993_216_059_187_305e-8;

/// `IGpsInfoFormat.LoadItem`.
pub fn default_load_item<F: Format + ?Sized>(f: &mut F, ctx: &mut Ctx, line: &[u8]) -> bool {
    let mut it1 = Item::default();
    let mut it2 = Item::default();
    let (r1, r2) = f.load_item2(ctx, line, &mut it1, &mut it2);
    let mut res1 = r1 && it1.is_valid();
    let mut res2 = r2 && it2.is_valid();
    if res1
        && it1.pt == PT_GPS
        && let Some(last) = ctx.last_gps_index.and_then(|i| ctx.points.get(i))
        && (it1.time - last.time).abs() < DUP_TIME
        && (it1.x - last.x).abs() < DUP_DEG
        && (it1.y - last.y).abs() < DUP_DEG
    {
        res1 = false;
    }
    if !res1 && res2 {
        it1 = it2;
        res1 = true;
        res2 = false;
    }
    let len = ctx.points.len();
    if res2 && it2.pt == PT_GPS {
        ctx.last_gps_index = Some(len + 1);
    } else if res1 && it1.pt == PT_GPS {
        ctx.last_gps_index = Some(len);
    }
    if res1 {
        ctx.points.push(it1);
    }
    if res2 {
        ctx.points.push(it2);
    }
    res1 || res2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_chars_rules() {
        let must = CharSet::of(b"$,");
        assert!(check_chars(b"$GP,x\0\0", &must, &CharSet::EMPTY));
        assert!(!check_chars(b"GP,x", &must, &CharSet::EMPTY));
        assert!(!check_chars(b"$GP,\x01", &must, &CharSet::of(b"\x01")));
        assert!(check_chars(b"$GP,\0", &must, &CharSet::of(b"\0")), "trailing NULs ignored");
    }

    #[test]
    fn validity_fixes() {
        let mut p = Item::gps(25600.0, 30.5, 60.5);
        assert!(p.is_valid());
        assert_eq!(p.time, 25600.0 + 14640.0);
        let mut p = Item::gps(42000.0, 30.0, 60.5);
        assert!(p.is_valid());
        assert_eq!(p.warn, 2);
        assert!(!Item::gps(42000.0, 181.0, 60.5).is_valid());
        assert!(!Item::gps(1.0, 30.5, 60.5).is_valid());
        assert!(Item::gps(-1.0, 30.5, 60.5).is_valid());
        assert!(!Item::gps(f64::NAN, 30.5, 60.5).is_valid());
        assert!(!Item::gps(42000.0, f64::NAN, 60.5).is_valid());
        assert!(!Item::gsensor(0.0, 0.0, 0.0, 0.0).is_valid());
        assert!(!Item::gsensor(0.0, 10.0, 0.0, 0.0).is_valid());
        assert!(Item::gsensor(0.0, 0.1, 0.0, 0.0).is_valid());
    }
}
