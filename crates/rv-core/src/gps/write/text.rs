//! Number, date and time texts as the original's writers produce them (`UnitUtilsStr`:
//! `FloatToStrPo`, `IntToStr2`, `dateTimeToFormatTZ`, `DateTimeForFileToStr`,
//! `TimeDurationToStr`; SysUtils `FloatToStrF`, `DateToStr`, `TimeToStr`) and the two map helpers
//! the writers use (`gmGetDistance`, `IsGPSBreak`).

use super::Locale;
use crate::gps::format::Item;
use crate::gps::util::SEC;

/// One millisecond in days.
pub(crate) const MS: f64 = SEC / 1000.0;

/// Delphi `Round`: round half to even (the x87 default). Non-finite values give 0.
pub(crate) fn round(v: f64) -> i64 {
    if v.is_finite() { v.round_ties_even() as i64 } else { 0 }
}

/// `IntToStrN(x, n)`: at least `n` digits, zero-padded.
pub(crate) fn int_n(x: i64, n: usize) -> String {
    if x < 0 { format!("-{:0n$}", x.unsigned_abs()) } else { format!("{x:0n$}") }
}

/// `IntToStr2`.
pub(crate) fn int2(x: i64) -> String {
    int_n(x, 2)
}

/// `FloatToDecimal` with 18 significant digits: the digits of |v| without trailing zeros and the
/// exponent `e` with |v| = 0.d1d2d3... × 10^e. Zero (and non-finite values) give no digits.
fn decimal(v: f64) -> (Vec<u8>, i32) {
    if v == 0.0 || !v.is_finite() {
        return (Vec::new(), 0);
    }
    let s = format!("{:.17e}", v.abs());
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let mut d: Vec<u8> = m.bytes().filter(u8::is_ascii_digit).collect();
    while d.last() == Some(&b'0') {
        d.pop();
    }
    (d, e.parse::<i32>().unwrap_or(0) + 1)
}

/// Rounds a decimal from [`decimal`] half up to `decimals` digits after the point.
fn round_decimals(d: &mut Vec<u8>, e: &mut i32, decimals: i32) {
    let keep = *e + decimals;
    if keep < 0 {
        d.clear();
    } else if (keep as usize) < d.len() {
        let keep = keep as usize;
        let up = d[keep] >= b'5';
        d.truncate(keep);
        if up {
            let mut i = d.len();
            loop {
                if i == 0 {
                    d.insert(0, b'1');
                    *e += 1;
                    break;
                }
                i -= 1;
                if d[i] == b'9' {
                    d[i] = b'0';
                } else {
                    d[i] += 1;
                    break;
                }
            }
        }
    }
    while d.last() == Some(&b'0') {
        d.pop();
    }
    if d.is_empty() {
        *e = 0;
    }
}

/// `FloatToStrF(v, ffFixed, 18, decimals)` with '.' as the decimal separator.
pub(crate) fn fixed(v: f64, decimals: usize) -> String {
    let (mut d, mut e) = decimal(v);
    round_decimals(&mut d, &mut e, decimals as i32);
    let digit = |k: i32| if k >= 0 { d.get(k as usize).map_or('0', |&c| char::from(c)) } else { '0' };
    let mut s = String::new();
    if v < 0.0 && !d.is_empty() {
        s.push('-');
    }
    if e <= 0 {
        s.push('0');
    } else {
        (0..e).for_each(|k| s.push(digit(k)));
    }
    if decimals > 0 {
        s.push('.');
        (0..decimals as i32).for_each(|k| s.push(digit(e + k)));
    }
    s
}

/// `FloatToStrF(v, ffGeneral, precision, exp_digits)` with '.' as the decimal separator: the
/// shortest text of `precision` significant digits, in exponent form ("1.5E-6") when the
/// exponent is above `precision` or the value below 0.0001.
pub(crate) fn general(v: f64, precision: i32, exp_digits: usize) -> String {
    let (mut d, mut e) = decimal(v);
    if precision < 18 {
        let decimals = precision - e;
        round_decimals(&mut d, &mut e, decimals);
    }
    if d.is_empty() {
        return "0".to_string();
    }
    let digits = |r: &[u8]| r.iter().map(|&c| char::from(c)).collect::<String>();
    let mut s = String::new();
    if v < 0.0 {
        s.push('-');
    }
    if e > precision || e < -3 {
        s.push(char::from(d[0]));
        if d.len() > 1 {
            s.push('.');
            s.push_str(&digits(&d[1..]));
        }
        let x = e - 1;
        s.push('E');
        if x < 0 {
            s.push('-');
        }
        s.push_str(&format!("{:0w$}", x.unsigned_abs(), w = exp_digits.min(4)));
    } else if e <= 0 {
        s.push_str("0.");
        (0..-e).for_each(|_| s.push('0'));
        s.push_str(&digits(&d));
    } else {
        let e = e as usize;
        (0..e).for_each(|k| s.push(d.get(k).map_or('0', |&c| char::from(c))));
        if d.len() > e {
            s.push('.');
            s.push_str(&digits(&d[e..]));
        }
    }
    s
}

/// `FloatToStrPo(digit, dotMandatory, v)`: `fixed(v, digit)` when `dot_mandatory`, else the
/// general format; then everything after `digit` decimals is cut off (not rounded). Like the
/// original, a value in exponent form with a '.' loses its exponent ("1.5E-6" → "1.5").
pub(crate) fn po(v: f64, digit: usize, dot_mandatory: bool) -> String {
    let mut s = if dot_mandatory { fixed(v, digit) } else { general(v, 18, digit) };
    if let Some(p) = s.find('.') {
        let keep = p + 1 + digit;
        if keep < s.len() {
            s.truncate(keep);
        }
    }
    s
}

/// `gmPositionToWebStr`: a coordinate with 6 decimals.
pub(crate) fn coord(v: f64) -> String {
    po(v, 6, true)
}

/// Date and time parts of a local TDateTime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Parts {
    pub y: i32,
    pub mo: u32,
    pub d: u32,
    pub h: u32,
    pub mi: u32,
    pub s: u32,
    pub ms: u32,
}

/// `DecodeDateTime` (via `DateTimeToTimeStamp`): the date is the whole days, the time the
/// fraction rounded to the millisecond. (Within half a millisecond of midnight the original
/// keeps the date and shows hour 24; this carries to the next day instead.)
pub(crate) fn decode(t: f64) -> Parts {
    use chrono::Datelike;
    const MS_PER_DAY: i64 = 86_400_000;
    let t = if t.is_finite() && t.abs() < 3e6 { t } else { 0.0 };
    let ms = (t.fract().abs() * MS_PER_DAY as f64).round_ties_even() as i64;
    let days = t.trunc() as i64 + ms / MS_PER_DAY;
    let ms = (ms % MS_PER_DAY) as u32;
    let epoch = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).expect("valid epoch");
    let date = epoch.checked_add_signed(chrono::TimeDelta::days(days)).unwrap_or(epoch);
    Parts {
        y: date.year(),
        mo: date.month(),
        d: date.day(),
        h: ms / 3_600_000,
        mi: ms / 60_000 % 60,
        s: ms / 1000 % 60,
        ms: ms % 1000,
    }
}

/// `dateTimeToFormatTZ`: "2016-05-16T13:06:26Z" (milliseconds dropped).
pub(crate) fn iso_z(t: f64) -> String {
    let p = decode(t);
    let two = |v: u32| int2(v.into());
    format!("{}-{}-{}T{}:{}:{}Z", p.y, two(p.mo), two(p.d), two(p.h), two(p.mi), two(p.s))
}

/// `DateTimeForFileToStr`: "2016.05.16 13-06-26".
pub(crate) fn for_file(t: f64) -> String {
    let p = decode(t);
    format!(
        "{}.{}.{} {}-{}-{}",
        int_n(p.y.into(), 4),
        int2(p.mo.into()),
        int2(p.d.into()),
        int2(p.h.into()),
        int2(p.mi.into()),
        int2(p.s.into())
    )
}

/// `TimeDurationToStrD(t, False)`: "H:MM:SS" with the seconds rounded up.
pub(crate) fn duration_str(t: f64) -> String {
    let ms = round(t.abs() / MS).unsigned_abs();
    let s = ms.div_ceil(1000);
    format!("{}:{}:{}", s / 3600, int2(((s / 60) % 60) as i64), int2((s % 60) as i64))
}

/// `TimeSrtToStr`: "hh:mm:ss,mmm" of a time in days; negative times are 0.
pub(crate) fn srt_time(t: f64) -> String {
    let ms = round(t.max(0.0) / MS);
    format!(
        "{}:{}:{},{}",
        int2(ms / 3_600_000),
        int2((ms / 60_000) % 60),
        int2((ms / 1000) % 60),
        int_n(ms % 1000, 3)
    )
}

impl Locale {
    /// `DateToStr`.
    pub(crate) fn date(&self, t: f64) -> String {
        format_pattern(&self.date_format, &decode(t))
    }

    /// `TimeToStr`.
    pub(crate) fn time(&self, t: f64) -> String {
        format_pattern(&self.time_format, &decode(t))
    }

    /// `DateTimeToStr`: the date, then the time unless it is exactly midnight.
    pub(crate) fn date_time(&self, t: f64) -> String {
        let p = decode(t);
        let mut s = format_pattern(&self.date_format, &p);
        if (p.h, p.mi, p.s, p.ms) != (0, 0, 0, 0) {
            s.push(' ');
            s.push_str(&format_pattern(&self.time_format, &p));
        }
        s
    }

    /// Replaces the '.' of a number from this module by the decimal separator.
    pub(crate) fn num(&self, s: &str) -> String {
        if self.decimal_separator == '.' { s.to_string() } else { s.replace('.', &self.decimal_separator.to_string()) }
    }
}

/// Formats date-time parts with a Qt-style pattern (see [`Locale`]).
pub(crate) fn format_pattern(pattern: &str, p: &Parts) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let twelve = pattern.contains("AP") || pattern.contains("ap") || pattern.contains('A') || pattern.contains('a');
    let mut out = String::new();
    let mut i = 0;
    let num = |out: &mut String, v: u32, n: usize| out.push_str(&int_n(v.into(), if n >= 2 { 2 } else { 1 }));
    while i < chars.len() {
        let c = chars[i];
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        match c {
            '\'' => {
                // quoted text; '' is a quote
                if n >= 2 && n % 2 == 0 {
                    (0..n / 2).for_each(|_| out.push('\''));
                    i += n;
                    continue;
                }
                let end = chars[i + 1..].iter().position(|&x| x == '\'').map_or(chars.len(), |k| i + 1 + k);
                out.extend(&chars[i + 1..end]);
                i = end + 1;
                continue;
            }
            'd' => num(&mut out, p.d, n),
            'M' => num(&mut out, p.mo, n),
            'y' if n >= 4 => {
                out.push_str(&int_n(p.y.into(), 4));
                n = 4;
            }
            'y' if n >= 2 => {
                out.push_str(&int2(i64::from(p.y.rem_euclid(100))));
                n = 2;
            }
            'H' => num(&mut out, p.h, n),
            'h' => num(&mut out, if twelve { (p.h + 11) % 12 + 1 } else { p.h }, n),
            'm' => num(&mut out, p.mi, n),
            's' => num(&mut out, p.s, n),
            'z' => out.push_str(&int_n(p.ms.into(), if n >= 3 { 3 } else { 1 })),
            'A' | 'a' => {
                let pm = p.h >= 12;
                let text = if pm { "PM" } else { "AM" };
                out.push_str(&if c == 'A' { text.to_string() } else { text.to_lowercase() });
                n = 1;
                if chars.get(i + 1).is_some_and(|x| x.eq_ignore_ascii_case(&'p')) {
                    n = 2;
                }
            }
            _ => {
                out.push(c);
                n = 1;
            }
        }
        i += n;
    }
    out
}

/// `gmGetDistance`: metres between two points on a 6371 km sphere. The original uses the law
/// of cosines in 80-bit floats, its cosine clamped to 0..1 (so at most a quarter circle); this
/// uses the equivalent haversine form, which keeps that precision for metre-sized steps in
/// 64-bit floats.
pub(crate) fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    const SAME: f64 = 8.993_216_059_187_305e-8;
    if (x1 - x2).abs() <= SAME && (y1 - y2).abs() <= SAME {
        return 0.0;
    }
    let (y1, y2) = (y1.to_radians(), y2.to_radians());
    let h = ((y2 - y1) / 2.0).sin().powi(2) + y1.cos() * y2.cos() * ((x2 - x1).to_radians() / 2.0).sin().powi(2);
    let c = 2.0 * h.clamp(0.0, 1.0).sqrt().asin();
    c.min(std::f64::consts::FRAC_PI_2) * 6_371_000.0
}

/// `IsGPSBreak`: whether two fixes are too far apart in time or place to be joined by a line.
pub(crate) fn is_break(p1: &Item, p2: &Item) -> bool {
    let dt = (p1.time - p2.time).abs();
    let dx = (p1.x - p2.x).abs() + (p1.y - p2.y).abs();
    let joined = (dx < 0.005 && dt <= 11.0 * SEC)
        || (dx <= 0.001 && dt <= 15.0 * 60.0 * SEC)
        || (dx <= 0.0002 && dt <= 4.0 * 3600.0 * SEC)
        || (dt <= 11.0 * SEC && dx < 259.2 * dt);
    !joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delphi_numbers() {
        assert_eq!(round(2.5), 2);
        assert_eq!(round(3.5), 4);
        assert_eq!(round(-0.5), 0);
        assert_eq!(fixed(30.327022, 6), "30.327022");
        assert_eq!(fixed(30.5, 6), "30.500000");
        assert_eq!(fixed(-0.0000001, 6), "0.000000");
        assert_eq!(fixed(-1.25, 1), "-1.3", "half up on the decimal digits");
        assert_eq!(fixed(0.0078125, 6), "0.007813");
        assert_eq!(fixed(0.6, 0), "1");
        assert_eq!(fixed(9.9999999, 6), "10.000000");
        assert_eq!(fixed(42506.54590277778, 7), "42506.5459028");
        assert_eq!(general(12.5, 18, 1), "12.5");
        assert_eq!(general(100.0, 18, 1), "100");
        assert_eq!(general(0.0, 18, 1), "0");
        assert_eq!(general(0.001, 18, 1), "0.00100000000000000002", "18 digits of the double, as Delphi");
        assert_eq!(general(0.001, 15, 1), "0.001");
        assert_eq!(po(0.001, 1, false), "0.0");
        assert_eq!(general(0.0000015, 15, 1), "1.5E-6");
        assert_eq!(general(-1.5e20, 15, 2), "-1.5E20");
        assert_eq!(general(45.3, 15, 0), "45.3");
        assert_eq!(po(12.96, 1, false), "12.9", "cut, not rounded");
        assert_eq!(po(0.1, 1, false), "0.1");
        assert_eq!(po(-3.75, 1, false), "-3.7");
        assert_eq!(po(70.0, 2, false), "70");
        assert_eq!(po(0.0000015, 1, false), "1.5", "the original loses the exponent");
        assert_eq!(po(0.0123, 3, true), "0.012");
        assert_eq!(coord(-30.1234567), "-30.123457");
    }

    #[test]
    fn dates_and_durations() {
        let t = crate::gps::util::encode_date_time(2016, 5, 16, 13, 6, 26, 500).unwrap();
        assert_eq!(iso_z(t), "2016-05-16T13:06:26Z");
        assert_eq!(for_file(t), "2016.05.16 13-06-26");
        assert_eq!(decode(t).ms, 500);
        assert_eq!(decode(-1.25), Parts { y: 1899, mo: 12, d: 29, h: 6, ..Parts::default() });
        assert_eq!(decode(42506.0 - 0.1 / 86_400_000.0).d, 16, "carried to the next day");
        assert_eq!(duration_str(3725.2 * SEC), "1:02:06");
        assert_eq!(duration_str(0.0), "0:00:00");
        assert_eq!(srt_time(3725.2 * SEC), "01:02:05,200");
        assert_eq!(srt_time(-1.0), "00:00:00,000");
        let p = decode(t);
        assert_eq!(format_pattern("dd.MM.yyyy HH:mm:ss.zzz", &p), "16.05.2016 13:06:26.500");
        assert_eq!(format_pattern("M/d/yy h:mm:ss AP", &p), "5/16/16 1:06:26 PM");
        assert_eq!(format_pattern("yyyy-MM-dd'T'hh", &p), "2016-05-16T13");
        let l = Locale::default();
        assert_eq!(l.date_time(t.trunc()), "16.05.2016");
        assert_eq!(l.date_time(t), "16.05.2016 13:06:26");
    }

    #[test]
    fn geo_helpers() {
        // 0.01 degree of latitude is ~1112 m
        assert!((distance(30.0, 60.0, 30.0, 60.01) - 1112.0).abs() < 1.0);
        assert_eq!(distance(30.0, 60.0, 30.00000001, 60.0), 0.0);
        let a = Item::gps(42506.5, 30.0, 60.0);
        let mut b = Item::gps(42506.5 + SEC, 30.0001, 60.0);
        assert!(!is_break(&a, &b));
        b.time += 20.0 * SEC;
        assert!(!is_break(&a, &b), "a small step after a 21 s gap is joined");
        b.x += 0.01;
        assert!(is_break(&a, &b));
        assert!(is_break(&a, &Item::gsensor(42506.5, 0.1, 0.0, 1.0)));
    }
}
