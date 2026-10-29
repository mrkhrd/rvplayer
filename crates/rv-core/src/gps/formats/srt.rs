//! `UnitGps_SRT`: SubRip subtitles (`.srt`) whose text carries the position, as many dash cams
//! write them, and MP4 text-track samples with the same kind of text.
//!
//! A file is split into cues at the "n / hh:mm:ss,mmm --> hh:mm:ss,mmm" headers; the cue start
//! is the record's video time. The cue text is upper-cased and cut into tokens of the
//! characters `ALNOTXYZENSW`, digits, `-./\:,'"` and the degree signs (anything else separates
//! tokens; "<letter>:" followed by a sign or digit also ends a token). Tokens are read by a
//! small state machine (`SubtitleParseItem`):
//! - "N55.123", "55.123N", "N55°07'22.8\"" (also "S", "E", "W"; degrees, minutes and seconds
//!   separated by `°` in CP1252 or UTF-8, `ø`, `'` or `"`);
//! - "LAT:" / "LAT.:" / "N:" / "S:" and "LON:" / "LON.:" / "E:" / "W:" followed by the value;
//! - a time "hh:mm:ss" (or with '-') and a date "dd.mm.yyyy" / "yyyy-mm-dd" (also '/', '\');
//! - G-sensor "X:" / "Y:" / "Z:" followed by the value in g.
//!
//! Date and time are the camera's clock taken as local time (no time zone is applied). A cue
//! without both a date and a time gets time -1 (unknown) and is placed by its video time.
//! Speed and course are not read. A fix is taken only from a cue more than 0.92 s (video time)
//! after the previous fix and when both coordinates differ from the previous fix, so the cue
//! at 00:00:00 and a fix with an unchanged latitude or longitude are dropped, like the original.

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, PT_GSENSOR, RawItem, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{CharSet, DIGITS, encode_date_time, pos, split, str_to_float, str_to_int};

static SPEC: Spec = Spec { title: "SubRip Subtitles", def_ext: "srt", ..EMPTY_SPEC };

/// Marks a coordinate or G-sensor axis whose value is the next token ("LAT:", "X:").
const NEXT: f64 = -200.0;
/// Marks a coordinate whose value is the next token, negated ("S:", "W:").
const NEXT_NEG: f64 = -202.0;
/// Cues closer than this in video time (0.92 s, in days) give one fix.
const FRAME_GAP: f64 = 1.064_814_814_814_814_8e-5;
/// `FindHead` looks for a header starting within this many bytes.
const HEAD_WINDOW: usize = 0x200;
/// `TestLoadData`: a whole header must lie within the first 50 bytes.
const TEST_WINDOW: usize = 0x32;

/// Characters of a token (after upper-casing); every other character separates tokens.
const TOKEN_CHARS: CharSet = CharSet::of(b"ALNOTXYZ-./\\:\"'\xb0\xc2\xf8ENSW,0123456789");
/// A token also ends after "<letter>:" when a sign or digit follows ("LAT:55.1").
const DDOT_PREV: CharSet = CharSet::ranges(&[(b'A', b'Z'), (b'a', b'z')]);
const DDOT_NEXT: CharSet = CharSet::of(b"+-0123456789");
/// Separators of degrees, minutes and seconds.
const DEG_SEP: CharSet = CharSet::of(b"\"'\xb0\xc2\xf8");
const TIME_SEP: CharSet = CharSet::of(b"-:");
const DATE_SEP: CharSet = CharSet::of(b"-./\\");

/// `TGpsFormat_SRT`: the last fix taken and the video time of the last cue that had one.
#[derive(Default)]
pub struct Srt {
    last_frame_time: f64,
    last_x: f64,
    last_y: f64,
}

pub fn create() -> Box<dyn Format> {
    Box::new(Srt::default())
}

/// Delphi `EncodeTime` for values already checked to be in range.
fn encode_time(h: i32, m: i32, s: i32, ms: i32) -> f64 {
    (f64::from(h) * 3_600_000.0 + f64::from(m) * 60_000.0 + f64::from(s) * 1000.0 + f64::from(ms)) / 86_400_000.0
}

/// The cursor of `FindHead` and its nested readers (`RInt`, `SkipChar`, `RChar`, `RStr`).
struct Reader<'a> {
    data: &'a [u8],
    p: usize,
    end: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        if self.p < self.end { self.data.get(self.p).copied() } else { None }
    }

    /// `RInt`: a run of digits and its value (`StrToIntDef(.., 0)`).
    fn int(&mut self) -> Option<i32> {
        if !self.peek().is_some_and(|c| DIGITS.contains(c)) {
            return None;
        }
        let start = self.p;
        loop {
            self.p += 1;
            if !self.peek().is_some_and(|c| DIGITS.contains(c)) {
                break;
            }
        }
        Some(str_to_int(self.data.get(start..self.p).unwrap_or_default(), 0))
    }

    /// `SkipChar`: an optional character.
    fn skip(&mut self, c: u8) {
        if self.peek() == Some(c) {
            self.p += 1;
        }
    }

    /// `RChar`.
    fn byte(&mut self, c: u8) -> Option<()> {
        (self.peek() == Some(c)).then(|| self.p += 1)
    }

    /// `RStr`.
    fn text(&mut self, s: &[u8]) -> Option<()> {
        if s.len() > self.end.saturating_sub(self.p) {
            return None;
        }
        for &c in s {
            self.byte(c)?;
        }
        Some(())
    }

    /// "," or "." before the milliseconds.
    fn ms_sep(&mut self) -> Option<()> {
        self.byte(b',').or_else(|| self.byte(b'.'))
    }

    /// One cue header "n[\r]\nhh:mm:ss,mmm --> hh:mm:ss,mmm" (n > 0); its start time.
    fn head(&mut self) -> Option<f64> {
        if self.int()? <= 0 {
            return None;
        }
        self.skip(b'\r');
        self.byte(b'\n')?;
        let h = self.int()?;
        self.byte(b':')?;
        let m = self.int()?;
        self.byte(b':')?;
        let s = self.int()?;
        self.ms_sep()?;
        let ms = self.int()?;
        if !((0..24).contains(&h) && (0..60).contains(&m) && (0..60).contains(&s) && (0..1000).contains(&ms)) {
            return None;
        }
        self.text(b" --> ")?;
        self.int()?;
        self.byte(b':')?;
        self.int()?;
        self.byte(b':')?;
        self.int()?;
        self.ms_sep()?;
        self.int()?;
        Some(encode_time(h, m, s, ms))
    }
}

/// `FindHead`: the first cue header starting in `data[*p..]` within [`HEAD_WINDOW`] bytes and
/// ending there and before `end`. On success `*p` is just after the header; returns where the
/// header starts and the cue's start time (days).
fn find_head(data: &[u8], p: &mut usize, end: usize) -> Option<(usize, f64)> {
    let end = end.min(data.len()).min(p.saturating_add(HEAD_WINDOW));
    let mut p0 = *p;
    while *p < end {
        let mut r = Reader { data, p: *p, end };
        if let Some(t) = r.head() {
            *p = r.p;
            return Some((p0, t));
        }
        p0 += 1;
        *p = p0;
    }
    None
}

/// `DegFromStr`: decimal degrees, or degrees, minutes and seconds separated by [`DEG_SEP`]
/// (only when the text has a "'"); 0 when unreadable.
fn deg_from_str(s: &[u8]) -> f64 {
    let r = str_to_float(s, 0.0);
    if r != 0.0 || pos(b"'", s).is_none() {
        return r;
    }
    let cols = split(s, &DEG_SEP, 0);
    let [de, mi, se, ..] = cols.as_slice() else { return r };
    let (de, mi, se) = (str_to_float(de, -1000.0), str_to_float(mi, -1000.0), str_to_float(se, -1000.0));
    if (-180.0..=180.0).contains(&de) && (0.0..60.0).contains(&mi) && (0.0..60.0).contains(&se) {
        (se / 60.0 + mi) / 60.0 + de
    } else {
        r
    }
}

/// The `Split` nested in `SubtitleParseItem`: exactly three fields of digits separated by
/// `sep` (an empty or too long field reads as -1); `None` for anything else.
fn split3(s: &[u8], sep: &CharSet) -> Option<[i32; 3]> {
    let mut v = [0i32; 3];
    let mut n = 0;
    let mut z = 0;
    for i in 0..=s.len() {
        match s.get(i) {
            Some(&c) if !sep.contains(c) => {
                if !DIGITS.contains(c) {
                    return None;
                }
            }
            _ => {
                let slot = v.get_mut(n)?;
                *slot = str_to_int(s.get(z..i).unwrap_or_default(), -1);
                n += 1;
                z = i + 1;
            }
        }
    }
    (n == 3).then_some(v)
}

/// Delphi `EncodeDate`; `None` where the original raises (the token is then ignored).
fn encode_date(y: i32, m: i32, d: i32) -> Option<f64> {
    encode_date_time(y, u32::try_from(m).ok()?, u32::try_from(d).ok()?, 0, 0, 0, 0)
}

/// `SubtitleParseItem`: applies one token to the fix `it` and the G-sensor sample `g`.
fn subtitle_parse_item(s: &[u8], it: &mut Item, g: &mut Item) {
    if g.x == NEXT {
        g.x = str_to_float(s, 0.0);
        return;
    }
    if s == b"X:" {
        g.x = NEXT;
        return;
    }
    if g.y == NEXT {
        g.y = str_to_float(s, 0.0);
        return;
    }
    if s == b"Y:" {
        g.y = NEXT;
        return;
    }
    if g.z == NEXT {
        g.z = str_to_float(s, 0.0);
        return;
    }
    if s == b"Z:" {
        g.z = NEXT;
        return;
    }
    let [first, .., last] = *s else { return };
    // NEXT + 201 = 1, NEXT_NEG + 201 = -1
    if it.x == NEXT || it.x == NEXT_NEG {
        it.x = (it.x + 201.0) * deg_from_str(s);
        return;
    }
    if it.y == NEXT || it.y == NEXT_NEG {
        it.y = (it.y + 201.0) * deg_from_str(s);
        return;
    }
    match s {
        b"LAT:" | b"LAT.:" | b"N:" => it.y = NEXT,
        b"S:" => it.y = NEXT_NEG,
        b"LON:" | b"LON.:" | b"E:" => it.x = NEXT,
        b"W:" => it.x = NEXT_NEG,
        _ => coordinate_or_date(s, first, last, it),
    }
}

/// The rest of `SubtitleParseItem`: a coordinate with its hemisphere letter first or last,
/// else a time (replaces the time of day), else a date (replaces the date).
fn coordinate_or_date(s: &[u8], first: u8, last: u8, it: &mut Item) {
    let hemi = |c: u8| matches!(c, b'E' | b'N' | b'S' | b'W');
    if hemi(first) || hemi(last) {
        let (c, value) = if hemi(first) {
            (first, s.get(1..).unwrap_or_default())
        } else {
            (last, s.get(..s.len() - 1).unwrap_or_default())
        };
        let d = deg_from_str(value);
        if d != 0.0 && (-180.0..=180.0).contains(&d) {
            match c {
                b'N' => it.y = d,
                b'S' => it.y = -d,
                b'E' => it.x = d,
                _ => it.x = -d,
            }
        }
        return;
    }
    if let Some([h, m, sec]) = split3(s, &TIME_SEP)
        && (0..24).contains(&h)
        && (0..60).contains(&m)
        && (0..60).contains(&sec)
    {
        it.time = it.time.trunc() + encode_time(h, m, sec, 0);
        return;
    }
    let date_ok = |d: i32, m: i32, y: i32| (1..=31).contains(&d) && (1..=12).contains(&m) && (1970..=2080).contains(&y);
    let d = match split3(s, &DATE_SEP) {
        Some([d, m, y]) if date_ok(d, m, y) => encode_date(y, m, d),
        Some([y, m, d]) if date_ok(d, m, y) => encode_date(y, m, d),
        _ => Some(0.0),
    };
    if let Some(d) = d
        && d > 0.0
    {
        it.time = it.time.fract() + d;
    }
}

/// `TGpsFormat_SRT.Parse` (also used by Panorama): reads a fix into `it` (cleared first) and a
/// G-sensor sample into `it2` from a cue text. The fix is good when it has a date and a time
/// of day, else, when `default_time` is not 0 and both coordinates are set, with
/// `default_time`. Returns (fix read, G-sensor sample read).
pub(crate) fn parse(data: &[u8], it: &mut Item, default_time: f64, it2: &mut Item) -> (bool, bool) {
    let data = data.to_ascii_uppercase();
    *it = Item::default();
    let n = data.len();
    let mut b = 0usize;
    for i in 0..=n {
        match data.get(i) {
            Some(&c) if TOKEN_CHARS.contains(c) => {
                if i >= 2 && data[i - 1] == b':' && DDOT_NEXT.contains(c) && DDOT_PREV.contains(data[i - 2]) && b < i {
                    subtitle_parse_item(&data[b..i], it, it2);
                    b = i;
                }
            }
            _ => {
                if b < i {
                    subtitle_parse_item(&data[b..i], it, it2);
                }
                b = i + 1;
            }
        }
    }
    let mut ok = false;
    if it.time > 2.0 && it.time.fract() != 0.0 {
        ok = true;
    } else if default_time != 0.0 && it.x != 0.0 && it.y != 0.0 {
        it.time = default_time;
        ok = true;
    }
    if ok {
        it.pt = PT_GPS;
        it.angle = -1.0;
        it.speed = -1.0;
    }
    let res2 = it2.x != 0.0 || it2.y != 0.0 || it2.z != 0.0;
    if res2 {
        it2.pt = PT_GSENSOR;
    }
    (ok, res2)
}

impl Format for Srt {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    /// Cues: the text between one header and the next, timed by the cue start. After a cue's
    /// header the search for the next header resumes after the cue's blank line.
    fn split_raw_data(&mut self, data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
        let end = data.len();
        let mut p = 0usize;
        let mut t = 0.0;
        match find_head(data, &mut p, end) {
            Some((_, t0)) => t = t0,
            None => p = end,
        }
        let mut items = Vec::new();
        let mut body1 = p;
        let mut t_next = 0.0;
        while p < end {
            let body2 = match find_head(data, &mut p, end) {
                Some((begin, tn)) => {
                    t_next = tn;
                    begin
                }
                None => {
                    p = end;
                    end
                }
            };
            if body2 > body1 {
                items.push(RawItem::timed(t, data.get(body1..body2).unwrap_or_default()));
            }
            body1 = p;
            while p + 4 < end {
                if data.get(p..p + 4) == Some(b"\r\n\r\n") {
                    p += 4;
                    break;
                }
                if data.get(p..p + 2) == Some(b"\n\n") {
                    p += 2;
                    break;
                }
                p += 1;
            }
            t = t_next;
        }
        (!items.is_empty()).then_some((items, 0.0))
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, data: &[u8]) -> DataTest {
        let mut p = 0;
        DataTest::from(find_head(data, &mut p, data.len().min(TEST_WINDOW)).is_some())
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {
        self.last_x = 0.0;
        self.last_y = 0.0;
        self.last_frame_time = 0.0;
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let (ok, res2) = parse(line, it1, -1.0, it2);
        let mut res1 = false;
        let frame_time = ctx.frame_time();
        if ok && (self.last_frame_time - frame_time).abs() > FRAME_GAP {
            self.last_frame_time = frame_time;
            if it1.x != 0.0 && it1.x != self.last_x && it1.y != 0.0 && it1.y != self.last_y {
                self.last_x = it1.x;
                self.last_y = it1.y;
                res1 = true;
            }
        }
        (res1, res2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::manager::{Env, load_items};
    use crate::gps::test_support::*;
    use crate::gps::util::SEC;

    /// 2016-05-16 as TDateTime.
    const D: f64 = 42_506.0;

    fn hms(h: f64, m: f64, s: f64) -> f64 {
        (h * 3600.0 + m * 60.0 + s) * SEC
    }

    fn cue(i: usize, start_s: u32, text: &str) -> String {
        let t = |s: u32| format!("00:{:02}:{:02},000", s / 60, s % 60);
        format!("{i}\r\n{} --> {}\r\n{text}\r\n\r\n", t(start_s), t(start_s + 1))
    }

    fn parse_str(s: &str, default_time: f64) -> (bool, bool, Item, Item) {
        let (mut it, mut g) = (Item::default(), Item::default());
        let (a, b) = parse(s.as_bytes(), &mut it, default_time, &mut g);
        (a, b, it, g)
    }

    #[test]
    fn subtitles_with_position_date_and_time() {
        let mut srt = String::new();
        srt.push_str(&cue(1, 0, "N55.750000 E37.610000 2016/05/16 12:30:45"));
        srt.push_str(&cue(2, 1, "N55.750100 E37.610200 2016/05/16 12:30:46"));
        srt.push_str(&cue(3, 2, "<font>n55.750200 e37.610400 60km/h 2016/05/16 12:30:47</font>"));
        srt.push_str(&cue(4, 3, "55.750300N 37.610600E 2016/05/16 12:30:48"));
        let r = load_log("a.srt", srt.as_bytes()).unwrap();
        assert_eq!(r.format, "srt");
        let t = &r.track;
        // the cue at 00:00:00 is not used (no video-time step from 0)
        assert_eq!(t.gps.len(), 3, "{:?}", t.gps);
        // track times are the cue (video) times; the start is video time 0
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 12:30:45");
        let times: Vec<f64> = t.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [1.0, 2.0, 3.0]);
        assert_eq!((t.gps[0].lat, t.gps[0].lon), (55.7501, 37.6102));
        assert_eq!((t.gps[2].lat, t.gps[2].lon), (55.7503, 37.6106));
        assert_eq!(t.gps[0].altitude, 0.0);
    }

    #[test]
    fn cue_headers() {
        // LF line ends, '.' before the milliseconds, a BOM before the first cue
        let mut srt = b"\xef\xbb\xbf".to_vec();
        for (i, s) in [(1, 1), (2, 3), (3, 5)] {
            let text = format!("N55.75{i} E37.61{i} 16.05.2016 12:00:0{s}");
            srt.extend_from_slice(format!("{i}\n00:00:0{s}.500 --> 00:00:0{}.000\n{text}\n\n", s + 1).as_bytes());
        }
        let r = load_log("a.srt", &srt).unwrap();
        assert_eq!(r.format, "srt");
        let t = &r.track;
        assert_eq!(t.gps.len(), 3);
        assert_eq!(t.start.unwrap().format("%H:%M:%S").to_string(), "11:59:59");
        assert_eq!(t.gps[2].time, 5.5);
        assert_eq!(t.gps[1].lat, 55.752);
        let mut ctx = Ctx::default();
        assert_eq!(Srt::default().test_load_data(&mut ctx, &srt), DataTest::Maybe);
        // the first header must lie within the first 50 bytes
        let late = format!("{}\r\n{}", "x".repeat(40), String::from_utf8_lossy(&srt[3..]));
        assert_eq!(Srt::default().test_load_data(&mut ctx, late.as_bytes()), DataTest::No);
        // cue number 0 is not a header
        let zero = String::from_utf8_lossy(&srt[3..]).replacen("1\n", "0\n", 1);
        assert_eq!(Srt::default().test_load_data(&mut ctx, zero.as_bytes()), DataTest::No);
    }

    #[test]
    fn split_into_cues() {
        let srt = format!("{}{}", cue(1, 1, "A\r\nB"), cue(2, 2, "C"));
        let (items, frame0) = Srt::default().split_raw_data(srt.as_bytes()).unwrap();
        assert_eq!(frame0, 0.0);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].line, b"\r\nA\r\nB\r\n\r\n");
        assert_eq!(items[0].frame_time, SEC);
        assert_eq!(items[1].line, b"\r\nC\r\n\r\n");
        assert_eq!(items[1].frame_time, 2.0 * SEC);
        assert!(Srt::default().split_raw_data(b"no subtitles here").is_none());
    }

    #[test]
    fn token_dialects() {
        let (ok, g, it, _) = parse_str("LAT: 55.5 LON: 37.25 12:00:01 2016-05-16", -1.0);
        assert!(ok && !g);
        assert_eq!((it.pt, it.y, it.x, it.angle, it.speed), (PT_GPS, 55.5, 37.25, -1.0, -1.0));
        assert!((it.time - (D + hms(12.0, 0.0, 1.0))).abs() < 1e-9);
        // "TIME:" reads as "T" and "E:", which takes the next token as the longitude
        let (ok, _, it, _) = parse_str("LAT: 55.5 LON: 37.25 TIME: 12:00:01 2016-05-16", -1.0);
        assert!(!ok && it.x == 0.0);
        let (ok, _, it, _) = parse_str("S: 33.5 W: 151.25 16.05.2016 12-00-00", -1.0);
        assert!(ok);
        assert_eq!((it.y, it.x), (-33.5, -151.25));
        assert!((it.time - (D + 0.5)).abs() < 1e-9);
        // "LAT.:1" is one token (no letter before ':') and is ignored
        let (ok, _, it, _) = parse_str("N:55.1 E:37.2 lat.:1 2016\\05\\16 01:02:03", -1.0);
        assert!(ok);
        assert_eq!((it.y, it.x), (55.1, 37.2));
        assert!((it.time - (D + hms(1.0, 2.0, 3.0))).abs() < 1e-9);
        // degrees, minutes, seconds with a UTF-8 and a CP1252 degree sign, and 'ø'
        let (ok, _, it, _) = parse_str("55\u{b0}45'30\"N 37\u{b0}36'0.5\"E", -1.0);
        assert!(ok);
        assert!((it.y - (55.0 + 45.5 / 60.0)).abs() < 1e-12);
        assert!((it.x - (37.0 + 36.0 / 60.0 + 0.5 / 3600.0)).abs() < 1e-12);
        assert_eq!(it.time, -1.0, "no date: unknown time");
        let mut cp = b"N55\xb045'30\" E37\xf836'0\"".to_vec();
        let (mut it, mut g) = (Item::default(), Item::default());
        assert!(parse(&cp, &mut it, -1.0, &mut g).0);
        assert!((it.y - (55.0 + 45.5 / 60.0)).abs() < 1e-12 && (it.x - 37.6).abs() < 1e-12);
        cp[1] = b'x';
        assert!(!parse(&cp, &mut it, -1.0, &mut g).0, "one coordinate is not a fix");
    }

    #[test]
    fn times_need_date_and_time() {
        let (ok, _, it, _) = parse_str("N55.1 E37.2 12:00:00", -1.0);
        assert!(ok);
        assert_eq!(it.time, -1.0);
        let (ok, _, it, _) = parse_str("N55.1 E37.2 31.02.2016 12:00:00", -1.0);
        assert!(ok, "an impossible date is ignored");
        assert_eq!(it.time, -1.0);
        let (ok, _, _, _) = parse_str("N55.1 E37.2 16.05.2016", 0.0);
        assert!(!ok, "date without time and no default");
        let (ok, _, it, _) = parse_str("N55.1 E37.2 16.05.2016 00:00:00", 0.0);
        assert!(!ok && it.time == D, "midnight has no fraction");
        let (ok, _, _, _) = parse_str("N55.1 E37.2 32.05.2016 12:00:00 25:00:00 1969/05/16", 0.0);
        assert!(!ok);
        let (ok, _, _, _) = parse_str("N200 E37.2", -1.0);
        assert!(!ok, "out-of-range degrees are ignored");
    }

    #[test]
    fn gsensor_values() {
        let (ok, g, _, s) = parse_str("X:0.12 Y:-0.05 Z: 0.98", -1.0);
        assert!(!ok && g);
        assert_eq!((s.pt, s.x, s.y, s.z), (PT_GSENSOR, 0.12, -0.05, 0.98));
        let (_, g, _, s) = parse_str("X:", -1.0);
        assert!(g && s.x == NEXT, "a dangling marker stays (and fails validation later)");
        let mut srt = String::new();
        for i in 1..=3 {
            let text = format!("N55.7{i} E37.6{i} 2016/05/16 12:30:4{i} X:0.{i} Y:-0.{i} Z:1.0{i}");
            srt.push_str(&cue(i, i as u32, &text));
        }
        let r = load_log("a.srt", srt.as_bytes()).unwrap();
        assert_eq!(r.track.gps.len(), 3);
        let g: Vec<(f64, f64, f64, f64)> = r.track.gsensor.iter().map(|s| (s.time, s.x, s.y, s.z)).collect();
        assert_eq!(g, [(1.0, 0.1, -0.1, 1.01), (2.0, 0.2, -0.2, 1.02), (3.0, 0.3, -0.3, 1.03)]);
    }

    #[test]
    fn repeated_coordinates_and_close_cues_are_dropped() {
        let mut srt = String::new();
        srt.push_str(&cue(1, 1, "N55.71 E37.61 2016/05/16 12:30:41"));
        srt.push_str(&cue(2, 2, "N55.71 E37.62 2016/05/16 12:30:42")); // same latitude
        srt.push_str(&cue(3, 3, "N55.73 E37.63 2016/05/16 12:30:43"));
        let r = load_log("a.srt", srt.as_bytes()).unwrap();
        let lats: Vec<f64> = r.track.gps.iter().map(|p| p.lat).collect();
        assert_eq!(lats, [55.71, 55.73]);
        // cues 0.5 s apart: a cue within 0.92 s of the last fix's cue is skipped
        let mut srt = String::new();
        for (i, ms) in [500, 1000, 1500, 2000, 2500].iter().enumerate() {
            let t = format!("00:00:{:02},{:03}", ms / 1000, ms % 1000);
            let text = format!("N55.7{i}1 E37.6{i}1 2016/05/16 12:30:4{i}");
            srt.push_str(&format!("{}\r\n{t} --> {t}\r\n{text}\r\n\r\n", i + 1));
        }
        let r = load_log("a.srt", srt.as_bytes()).unwrap();
        let lats: Vec<f64> = r.track.gps.iter().map(|p| p.lat).collect();
        assert_eq!(lats, [55.711, 55.731]);
    }

    #[test]
    fn cues_without_time_take_the_video_time() {
        let mut srt = String::new();
        srt.push_str(&cue(1, 1, "N55.71 E37.61 2016/05/16 12:30:41"));
        srt.push_str(&cue(2, 2, "N55.72 E37.62"));
        srt.push_str(&cue(3, 4, "N55.73 E37.63 2016/05/16 12:30:43"));
        let r = load_log("a.srt", srt.as_bytes()).unwrap();
        let t: Vec<(f64, f64)> = r.track.gps.iter().map(|p| (p.time, p.lat)).collect();
        // the cue times (1, 2, 4 s) win over the clock in the text (41, 43 s): the median offset
        assert_eq!(t, [(1.0, 55.71), (2.0, 55.72), (4.0, 55.73)]);
        assert_eq!(r.track.start.unwrap().format("%H:%M:%S").to_string(), "12:30:40");
    }

    #[test]
    fn text_track_samples_from_a_video() {
        let env = Env { name: "a.mp4".into(), file_times: (0.0, 0.0), tz: 0.0, companion: &|_: &str| None };
        let items = (0..4)
            .map(|i| {
                let text = format!("N55.75{i} E37.61{i} 2016/05/16 12:30:4{}", 5 + i);
                RawItem::timed(f64::from(i) * SEC, text.as_bytes())
            })
            .collect();
        // SRT alone: with a video's records every format is a candidate
        let mut fmts = vec![create()];
        let loaded = load_items(&mut fmts, None, items, 0.0, "mov", 4.0 * SEC, &env).unwrap();
        assert_eq!(loaded.info.format_id, "movSrt");
        let fixes: Vec<&Item> = loaded.points.iter().filter(|p| p.x != 0.0).collect();
        assert_eq!(fixes.len(), 3, "the sample at video time 0 is not used");
        for (k, p) in fixes.iter().enumerate() {
            assert!((p.time - (D + hms(12.0, 30.0, 46.0 + k as f64))).abs() < 1e-9);
            assert!((p.y - (55.751 + 0.001 * k as f64)).abs() < 1e-9);
        }
    }

    #[test]
    fn fuzz_srt() {
        let mut srt = String::new();
        srt.push_str(&cue(1, 0, "N55.750000 E37.610000 2016/05/16 12:30:45 X:0.1 Y:0.2 Z:0.9"));
        srt.push_str(&cue(2, 1, "LAT: 55\u{b0}45'30\"N LON.: W37\u{b0}36'0\" 16.05.2016 12:30:46"));
        srt.push_str(&cue(3, 2, "S:55.7502 E:37.6104 2016-05-16 12-30-47"));
        fuzz("a.srt", srt.as_bytes());
        for s in ["", ":", "N", "N:", "X:", "1:2:3:4", "--::", "''''", "N'''", "\u{b0}\u{b0}'S", "W:-"] {
            let _ = parse_str(s, -1.0);
        }
    }
}
