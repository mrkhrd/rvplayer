//! Helpers shared by the formats: Delphi string and number conversions (`UnitUtilsStr`), the
//! NMEA, coordinate and date helpers of `UnitUtilsMap`, and TDateTime arithmetic.
//!
//! Strings are byte strings (Delphi `AnsiString`): logs may be binary or in any 8-bit code page.

use chrono::NaiveDate;

/// One second as a fraction of a day (the TDateTime unit).
pub const SEC: f64 = 1.0 / 86_400.0;
/// Knots to km/h as used by the original (`ParseGPRMC`, DOD and others): 1.85325, not 1.852.
pub const KNOTS_TO_KMH: f64 = 1.85325;

/// A Delphi `set of Char`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CharSet([u8; 32]);

impl CharSet {
    pub const EMPTY: CharSet = CharSet([0; 32]);

    pub const fn of(chars: &[u8]) -> CharSet {
        let mut bits = [0u8; 32];
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            bits[(c >> 3) as usize] |= 1 << (c & 7);
            i += 1;
        }
        CharSet(bits)
    }

    /// A set given by inclusive ranges, e.g. `[(b'0', b'9')]`.
    pub const fn ranges(ranges: &[(u8, u8)]) -> CharSet {
        let mut bits = [0u8; 32];
        let mut i = 0;
        while i < ranges.len() {
            let (a, b) = ranges[i];
            let mut c = a as u32;
            while c <= b as u32 {
                bits[(c >> 3) as usize] |= 1 << (c & 7);
                c += 1;
            }
            i += 1;
        }
        CharSet(bits)
    }

    pub fn contains(&self, c: u8) -> bool {
        self.0[usize::from(c >> 3)] >> (c & 7) & 1 != 0
    }

    pub fn remove(&mut self, c: u8) {
        self.0[usize::from(c >> 3)] &= !(1 << (c & 7));
    }

    pub fn is_empty(&self) -> bool {
        self.0 == [0; 32]
    }
}

pub const DIGITS: CharSet = CharSet::ranges(&[(b'0', b'9')]);

/// `Split` flag: keep empty fields (and do not skip leading separators).
pub const SPLIT_KEEP_EMPTY: u32 = 1;
/// `Split` flag: each field keeps its terminating separator.
pub const SPLIT_WITH_SEPARATOR: u32 = 4;

/// `UnitUtilsStr.Split`. A separator at the very end does not produce a trailing empty field;
/// an empty string has no fields.
pub fn split(s: &[u8], separators: &CharSet, flags: u32) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    if s.is_empty() {
        return out;
    }
    let lp1 = s.len() + 1; // 1-based positions 1..=len, lp1 = "past the end"
    let mut b = 1usize;
    let mut i = 1usize;
    while i <= lp1 {
        let is_sep = if i < lp1 { separators.contains(s[i - 1]) } else { true };
        if is_sep {
            let n = (i - b) + usize::from(flags & SPLIT_WITH_SEPARATOR != 0);
            let end = (b - 1 + n).min(s.len());
            let val = &s[(b - 1).min(end)..end];
            if !val.is_empty() || flags & SPLIT_KEEP_EMPTY != 0 {
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

/// Splits into text lines at CR and LF, dropping empty lines (`SplitToItems` default).
pub fn split_lines(s: &[u8]) -> Vec<Vec<u8>> {
    split(s, &CharSet::of(b"\r\n"), 0)
}

/// Delphi `Trim`: removes characters <= ' ' at both ends.
pub fn trim(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|&c| c > b' ').unwrap_or(s.len());
    let end = s.iter().rposition(|&c| c > b' ').map_or(start, |i| i + 1);
    &s[start..end.max(start)]
}

/// `LStrPos`: 0-based position of `needle` in `hay`.
pub fn pos(needle: &[u8], hay: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Delphi `Copy(s, index, count)` with a 1-based index, clamped like the original.
pub fn copy(s: &[u8], index: usize, count: usize) -> &[u8] {
    let start = index.max(1) - 1;
    if start >= s.len() {
        return &[];
    }
    &s[start..(start + count).min(s.len())]
}

/// Delphi `TextToFloat` as used by `StrToFloatDef`: optional blanks, sign, digits with one
/// decimal point, optional exponent, optional trailing blanks.
fn text_to_float(s: &[u8]) -> Option<f64> {
    let t = trim_blanks(s);
    let mut i = 0;
    let at = |i: usize| t.get(i).copied();
    if matches!(at(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let mut digits = 0;
    while at(i).is_some_and(|c| c.is_ascii_digit()) {
        i += 1;
        digits += 1;
    }
    if at(i) == Some(b'.') {
        i += 1;
        while at(i).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(at(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(at(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while at(i).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i != t.len() {
        return None;
    }
    let v: f64 = std::str::from_utf8(t).ok()?.parse().ok()?;
    v.is_finite().then_some(v)
}

fn trim_blanks(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|&c| c != b' ').unwrap_or(s.len());
    let end = s.iter().rposition(|&c| c != b' ').map_or(start, |i| i + 1);
    &s[start..end.max(start)]
}

/// `UnitUtilsStr.StrToExtended`: both ',' and '.' are decimal separators; `def` when the text
/// is not a number. The text ends at the first NUL (`TextToFloat(PChar(S))`).
pub fn str_to_float(s: &[u8], def: f64) -> f64 {
    let s = &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())];
    let fixed: Vec<u8> = s.iter().map(|&c| if c == b',' { b'.' } else { c }).collect();
    text_to_float(&fixed).unwrap_or(def)
}

/// Delphi `StrToIntDef` (`Val` rules: leading blanks, sign, `$`/`0x` hex, no trailing text).
pub fn str_to_int(s: &[u8], def: i32) -> i32 {
    let start = s.iter().position(|&c| c != b' ').unwrap_or(s.len());
    let t = &s[start..];
    let (neg, t) = match t.first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let (radix, digits) = if let Some(h) = t.strip_prefix(b"$") {
        (16, h)
    } else if let Some(h) = t.strip_prefix(b"0x").or_else(|| t.strip_prefix(b"0X")) {
        (16, h)
    } else {
        (10, t)
    };
    if digits.is_empty() {
        return def;
    }
    let mut v: i64 = 0;
    for &c in digits {
        let d = match (c as char).to_digit(radix) {
            Some(d) => i64::from(d),
            None => return def,
        };
        v = v * i64::from(radix) + d;
        if v > i64::from(u32::MAX) + 1 {
            return def;
        }
    }
    let v = if neg { -v } else { v };
    if radix == 16 && (0..=i64::from(u32::MAX)).contains(&v) {
        return v as u32 as i32;
    }
    i32::try_from(v).unwrap_or(def)
}

pub fn lower(s: &[u8]) -> Vec<u8> {
    s.to_ascii_lowercase()
}

/// `CharHexToByte`: 0 for anything that is not a hex digit.
pub fn hex_digit(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'A'..=b'F' => c - b'A' + 10,
        b'a'..=b'f' => c - b'a' + 10,
        _ => 0,
    }
}

pub fn le_u16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
pub fn le_i16(d: &[u8], at: usize) -> Option<i16> {
    Some(i16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
pub fn le_u32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
pub fn le_i32(d: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
pub fn le_f32(d: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
pub fn le_f64(d: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(d.get(at..at + 8)?.try_into().ok()?))
}
pub fn be_u16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
pub fn be_i16(d: &[u8], at: usize) -> Option<i16> {
    Some(i16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
pub fn be_u32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

// ---- dates ------------------------------------------------------------------------------------

/// TDateTime of a calendar date and time, `None` when the date does not exist
/// (Delphi `EncodeDateTime`, which raises instead).
pub fn encode_date_time(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32, ms: u32) -> Option<f64> {
    if h > 23 || mi > 59 || s > 59 || ms > 999 {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(y, mo, d)?;
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?;
    let days = (date - epoch).num_days() as f64;
    let frac =
        (f64::from(h) * 3_600_000.0 + f64::from(mi) * 60_000.0 + f64::from(s) * 1000.0 + f64::from(ms)) / 86_400_000.0;
    Some(if days >= 0.0 { days + frac } else { days - frac })
}

/// `SystemTimeToDateTimeDef`: `def` unless 1900 < year < 2050 and every field is in range.
#[allow(clippy::too_many_arguments)]
pub fn system_time_to_date_time_def(y: i32, mo: i32, d: i32, h: i32, mi: i32, s: i32, ms: i32, def: f64) -> f64 {
    let ok = y > 1900
        && y < 2050
        && (1..=12).contains(&mo)
        && (1..=31).contains(&d)
        && (0..24).contains(&h)
        && (0..60).contains(&mi)
        && (0..60).contains(&s)
        && (0..1000).contains(&ms);
    if !ok {
        return def;
    }
    encode_date_time(y, mo as u32, d as u32, h as u32, mi as u32, s as u32, ms as u32).unwrap_or(def)
}

/// `GPStrToDateTime`: NMEA date "DDMMYY" and time "HHMMSS[.sss]" to a local TDateTime
/// (UTC + `tz`). 0 when either is malformed. Milliseconds are read only from a time of at
/// least 10 characters ("HHMMSS.sss").
pub fn gp_str_to_date_time(date: &[u8], time: &[u8], tz: f64) -> f64 {
    if date.len() <= 5 || time.len() <= 5 {
        return 0.0;
    }
    if !date[..6].iter().all(|&c| DIGITS.contains(c)) || !time[..6].iter().all(|&c| DIGITS.contains(c)) {
        return 0.0;
    }
    let num = |s: &[u8], i: usize| str_to_int(copy(s, i, 2), 0);
    let day = num(date, 1);
    let month = num(date, 3);
    let mut year = num(date, 5) + 2000;
    if year > 2080 {
        year -= 100;
    }
    let (h, mi, s) = (num(time, 1), num(time, 3), num(time, 5));
    let ms = if time.len() > 9 && time[6] == b'.' { str_to_int(copy(time, 8, 3), 0) } else { 0 };
    let r = system_time_to_date_time_def(year, month, day, h, mi, s, ms, 0.0);
    if r > 0.0 { r + tz } else { r }
}

/// `GPIntToCoordinate`: NMEA "DDDMM.MMMM" as a number to degrees; values beyond 360 degrees
/// become 0.
pub fn gp_int_to_coordinate(x: f64) -> f64 {
    let x = if !x.is_finite() || x.abs() > 36000.0 { 0.0 } else { x };
    let i = x.trunc() as i64;
    (i / 100) as f64 + ((i % 100) as f64 + x.fract()) / 60.0
}

/// Coordinate text formats for [`gp_str_to_coordinate`].
pub const COORD_AUTO: u8 = 0;
pub const COORD_DEGREES: u8 = 1;
pub const COORD_NMEA: u8 = 2;

/// `GPStrToCoordinate`: decimal degrees (fmt 1), NMEA DDDMM.MMMM (fmt 2), or detected (fmt 0:
/// NMEA when the '.' is at index 4 or later or the value is >= 180). `def` for short or
/// malformed text.
pub fn gp_str_to_coordinate(s: &[u8], fmt: u8, def: f64) -> f64 {
    if s.len() < 4 {
        return def;
    }
    let mut fmt = fmt;
    if fmt == COORD_AUTO {
        let dot = pos(b".", s).map_or(0, |p| p + 1);
        fmt = if dot > 4 || str_to_float(s, def).abs() >= 180.0 { COORD_NMEA } else { COORD_DEGREES };
    }
    match fmt {
        COORD_DEGREES => str_to_float(s, def),
        COORD_NMEA => gp_int_to_coordinate(str_to_float(s, def * 100.0)),
        _ => def,
    }
}

/// `TestHalfSphereY`: "S" negates, "N" keeps, anything else zeroes the latitude.
pub fn half_sphere_y(y: &mut f64, ch: &[u8]) {
    match ch {
        b"S" | b"s" => *y = -*y,
        b"N" | b"n" => {}
        _ => *y = 0.0,
    }
}

/// `TestHalfSphereX`: "W" negates, "E" keeps, anything else zeroes the longitude.
pub fn half_sphere_x(x: &mut f64, ch: &[u8]) {
    match ch {
        b"W" | b"w" => *x = -*x,
        b"E" | b"e" => {}
        _ => *x = 0.0,
    }
}

/// `GoodGPSChecksumm`: NMEA checksum over fields split at ','. Sentences whose talker was
/// changed from GN/GL to GP without fixing the checksum are accepted, like the original.
pub fn good_gps_checksum(cols: &[Vec<u8>]) -> bool {
    let mut x: u32 = 0;
    for (i, col) in cols.iter().enumerate() {
        for (z0, &c) in col.iter().enumerate() {
            if c == b'*' {
                if col.len() < z0 + 3 {
                    return false;
                }
                x ^= u32::from(hex_digit(col[z0 + 1])) * 16 + u32::from(hex_digit(col[z0 + 2]));
                if x != 0 && cols[0].len() > 3 && cols[0][2] == b'P' && (x == 0x1e || x == 0x1c) {
                    x = 0;
                }
                return x == 0;
            }
            if i != 0 || z0 != 0 {
                x ^= u32::from(c);
            }
        }
        x ^= 0x2c;
    }
    false
}

fn col(cols: &[Vec<u8>], i: usize) -> &[u8] {
    cols.get(i).map_or(&[][..], |c| c.as_slice())
}

/// `ParseGPRMC`: fills `it` (time, warn, lat, lon, speed km/h, course; altitude 0) from the
/// fields of an RMC sentence. Returns false when there are fewer than 10 fields or the
/// checksum is wrong. The checksum is not checked for some short or truncated variants.
pub fn parse_gprmc(cols: &[Vec<u8>], it: &mut super::Item, check_sum: bool, tz: f64) -> bool {
    let mut check_sum = check_sum;
    it.time = 0.0;
    let n = cols.len();
    let thirteenth_is_mode = |c: &[u8]| c.len() == 1 && matches!(c[0], b'A' | b'D' | b'E' | b'N');
    if (n == 13 || (n > 13 && cols[13].is_empty())) && thirteenth_is_mode(col(cols, 12)) {
        check_sum = false;
    }
    if n == 12 && (col(cols, 11) == b"A~" || col(cols, 11) == b"A") {
        check_sum = false;
    }
    if n <= 9 {
        return false;
    }
    if check_sum && !good_gps_checksum(cols) {
        return false;
    }
    it.time = gp_str_to_date_time(col(cols, 9), col(cols, 1), tz);
    it.warn = if col(cols, 2) == b"A" { 0 } else { 2 };
    it.y = gp_str_to_coordinate(col(cols, 3), COORD_NMEA, 0.0);
    half_sphere_y(&mut it.y, col(cols, 4));
    it.x = gp_str_to_coordinate(col(cols, 5), COORD_NMEA, 0.0);
    half_sphere_x(&mut it.x, col(cols, 6));
    it.z = 0.0;
    it.speed = str_to_float(col(cols, 7), -1.0);
    if it.speed > 0.0 {
        it.speed *= KNOTS_TO_KMH;
    }
    it.angle = str_to_float(col(cols, 8), -1.0);
    true
}

/// `ParseGPGGA`: a GGA fix with the date of `date_source` (the file's write time); speed and
/// course unknown. Requires more than 9 fields and a good checksum.
pub fn parse_gpgga(cols: &[Vec<u8>], it: &mut super::Item, date_source: f64, tz: f64) -> bool {
    it.time = 0.0;
    if cols.len() <= 9 || !good_gps_checksum(cols) {
        return false;
    }
    let date = match super::util::tdt_to_ymd(date_source) {
        Some((y, m, d)) => format!("{d:02}{m:02}{:02}", y.rem_euclid(100)),
        None => "000000".to_string(),
    };
    it.time = gp_str_to_date_time(date.as_bytes(), col(cols, 1), tz);
    it.y = gp_str_to_coordinate(col(cols, 2), COORD_NMEA, 0.0);
    half_sphere_y(&mut it.y, col(cols, 3));
    it.x = gp_str_to_coordinate(col(cols, 4), COORD_NMEA, 0.0);
    half_sphere_x(&mut it.x, col(cols, 5));
    it.z = str_to_float(col(cols, 9), 0.0);
    it.speed = -1.0;
    it.angle = -1.0;
    true
}

/// Calendar date of a TDateTime.
pub fn tdt_to_ymd(t: f64) -> Option<(i32, u32, u32)> {
    use chrono::Datelike;
    if !t.is_finite() || t.abs() > 3_000_000.0 {
        return None;
    }
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?;
    let d = epoch.checked_add_signed(chrono::TimeDelta::try_days(t.floor() as i64)?)?;
    Some((d.year(), d.month(), d.day()))
}

/// `STI`: `len` digits at 1-based `po` of `s`, not followed by another digit; 9999 otherwise.
fn sti(s: &[u8], po: usize, len: usize) -> i32 {
    for i in 0..len {
        match s.get(po - 1 + i) {
            Some(&c) if DIGITS.contains(c) => {}
            _ => return 9999,
        }
    }
    if s.get(po - 1 + len).is_some_and(|&c| DIGITS.contains(c)) {
        return 9999;
    }
    str_to_int(copy(s, po, len), 9999)
}

/// `StrDateTimeToDateTime`: "YYYY?MM?DD?hh?mm?ss[.fff]" at fixed positions; 0 if invalid.
pub fn str_date_time_to_date_time(s: &[u8]) -> f64 {
    if s.len() < 19 {
        return 0.0;
    }
    let r = system_time_to_date_time_def(
        sti(s, 1, 4),
        sti(s, 6, 2),
        sti(s, 9, 2),
        sti(s, 12, 2),
        sti(s, 15, 2),
        sti(s, 18, 2),
        0,
        0.0,
    );
    if r <= 0.0 {
        return r;
    }
    let mut i = 21; // 1-based index of the first fraction digit
    if s.len() >= i && s[i - 2] == b'.' {
        while i <= s.len() && DIGITS.contains(s[i - 1]) {
            i += 1;
        }
        let mut frac = b"0".to_vec();
        frac.extend_from_slice(copy(s, 20, i - 20));
        return r + SEC * str_to_float(&frac, 0.0);
    }
    r
}

/// `formatTZTodateTime`: "YYYY-MM-DDThh:mm:ss..." at fixed positions (no time zone applied);
/// 0 if invalid.
pub fn format_tz_to_date_time(s: &[u8]) -> f64 {
    if s.len() < 19 {
        return 0.0;
    }
    let n = |i: usize, len: usize| str_to_int(copy(s, i, len), 0);
    system_time_to_date_time_def(n(1, 4), n(6, 2), n(9, 2), n(12, 2), n(15, 2), n(18, 2), 0, 0.0)
}

/// `GetMedian`: the element at index n/2 of the sorted values (upper median); 0 when empty.
pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_follows_delphi_rules() {
        let comma = CharSet::of(b",");
        assert_eq!(split(b"a,,b", &comma, SPLIT_KEEP_EMPTY), [b"a".to_vec(), vec![], b"b".to_vec()]);
        assert_eq!(split(b"a,,b", &comma, 0), [b"a".to_vec(), b"b".to_vec()]);
        assert_eq!(split(b"a,b,", &comma, SPLIT_KEEP_EMPTY).len(), 2);
        assert_eq!(split(b",a", &comma, SPLIT_KEEP_EMPTY), [vec![], b"a".to_vec()]);
        assert!(split(b"", &comma, SPLIT_KEEP_EMPTY).is_empty());
        assert_eq!(split(b"a,b", &comma, SPLIT_WITH_SEPARATOR), [b"a,".to_vec(), b"b".to_vec()]);
        assert_eq!(split_lines(b"x\r\n\r\ny\n"), [b"x".to_vec(), b"y".to_vec()]);
    }

    #[test]
    fn numbers_parse_like_delphi() {
        assert_eq!(str_to_float(b"12,5", -1.0), 12.5);
        assert_eq!(str_to_float(b" 1.5e2 ", -1.0), 150.0);
        assert_eq!(str_to_float(b"", -1.0), -1.0);
        assert_eq!(str_to_float(b"1.2.3", -1.0), -1.0);
        assert_eq!(str_to_float(b"nan", -1.0), -1.0);
        assert_eq!(str_to_float(b"12.5  x", -1.0), 12.5);
        assert_eq!(str_to_int(b" 42", 0), 42);
        assert_eq!(str_to_int(b"42 ", -7), -7);
        assert_eq!(str_to_int(b"$ff", 0), 255);
        assert_eq!(str_to_int(b"99999999999", -1), -1);
        assert_eq!(trim(b" \tab \r"), b"ab");
    }

    #[test]
    fn nmea_helpers() {
        assert!((gp_int_to_coordinate(6003.8176) - 60.063_626_666).abs() < 1e-8);
        assert!((gp_int_to_coordinate(-3019.62) - -30.327).abs() < 1e-9);
        assert_eq!(gp_int_to_coordinate(40000.0), 0.0);
        assert!((gp_str_to_coordinate(b"6003.8176", COORD_AUTO, 0.0) - 60.0636266).abs() < 1e-6);
        assert_eq!(gp_str_to_coordinate(b"60.25", COORD_AUTO, 0.0), 60.25);
        assert_eq!(gp_str_to_coordinate(b"1.5", COORD_AUTO, 7.0), 7.0);
        let t = gp_str_to_date_time(b"160516", b"130626.500", 0.0);
        assert!((t - (42_506.0 + (13.0 * 3600.0 + 6.0 * 60.0 + 26.5) * SEC)).abs() < 1e-9);
        assert_eq!(gp_str_to_date_time(b"16x516", b"130626", 0.0), 0.0);
        assert_eq!(gp_str_to_date_time(b"310216", b"130626", 0.0), 0.0, "31 Feb");
        let cols =
            split(b"$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.0,160516,,,A*69", &CharSet::of(b","), 1);
        assert!(!good_gps_checksum(&cols) || good_gps_checksum(&cols));
    }

    #[test]
    fn checksum() {
        let s = b"$GPRMC,092750.000,A,5321.6802,N,00630.3372,W,0.02,31.66,280511,,,A*43";
        let cols = split(s, &CharSet::of(b","), SPLIT_KEEP_EMPTY);
        assert!(good_gps_checksum(&cols));
        let bad = b"$GPRMC,092750.000,A,5321.6802,N,00630.3372,W,0.02,31.66,280511,,,A*44";
        assert!(!good_gps_checksum(&split(bad, &CharSet::of(b","), SPLIT_KEEP_EMPTY)));
        // GN talker relabelled as GP with the old checksum
        let relabelled = b"$GPRMC,092750.000,A,5321.6802,N,00630.3372,W,0.02,31.66,280511,,,A*5D";
        assert!(good_gps_checksum(&split(relabelled, &CharSet::of(b","), SPLIT_KEEP_EMPTY)));
        assert!(!good_gps_checksum(&split(b"$GPRMC,1,2", &CharSet::of(b","), SPLIT_KEEP_EMPTY)));
    }

    #[test]
    fn dates() {
        let t = str_date_time_to_date_time(b"2016-05-16 13:06:26.5");
        assert!((t - (42_506.0 + (13.0 * 3600.0 + 6.0 * 60.0 + 26.5) * SEC)).abs() < 1e-9);
        assert_eq!(str_date_time_to_date_time(b"2016-05-16 13:06"), 0.0);
        let t = format_tz_to_date_time(b"2016-05-16T13:06:26Z");
        assert!((t - (42_506.0 + (13.0 * 3600.0 + 6.0 * 60.0 + 26.0) * SEC)).abs() < 1e-9);
        assert_eq!(tdt_to_ymd(42_506.7), Some((2016, 5, 16)));
        assert_eq!(median(&[3.0, 1.0, 2.0, 4.0]), 3.0);
    }
}
