//! `UnitGps_DODtrak`: GPS blocks of DOD (Novatek-based) cameras, stored in `free` atoms with
//! a "GPS " tag inside the video's media data (see [`crate::gps::embedded`] for how they are
//! found). Each block becomes a record "MyDoDprfx" + payload (the atom from byte 16). Four
//! layouts, tried in this order; the format id gets the layout letter ("DOD_D"):
//! - B: date at payload 0x48, time 0x20 (UTC), f32 NMEA coordinates, speed f32 as is;
//! - C: date at 0x60, f64 coordinates, speed/course f64, G-sensor i32 /1024, and an encoded
//!   RMC sentence from 0x79 ("CC");
//! - D: date at 0x2c, local time 0x20 (hours -12..47 wrap), status/hemispheres at 0x38,
//!   f32 coordinates, speed knots;
//! - A: XOR-encoded 52-byte records from 0x20 marked "ATC"/"001"; the one with the highest
//!   counter gives the fix (1e-7 degrees), followed by an i8 G-sensor triple (1/64 g).

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::util::{
    CharSet, KNOTS_TO_KMH, SPLIT_KEEP_EMPTY, encode_date_time, gp_int_to_coordinate, half_sphere_x, half_sphere_y,
    le_f32, le_f64, le_i16, le_i32, parse_gprmc, split,
};

pub const LINE_PREFIX: &[u8] = b"MyDoDprfx";

static SPEC: Spec = Spec { title: "DOD", line_prefix: LINE_PREFIX, ..super::EMPTY_SPEC };

#[derive(Default)]
pub struct Dod {
    last_cnt: i32,
    see_format_id: &'static str,
}

pub fn create() -> Box<dyn Format> {
    Box::new(Dod::default())
}

fn i32_at(d: &[u8], at: usize) -> i32 {
    le_i32(d, at).unwrap_or(i32::MIN)
}

fn in_range(v: i32, lo: i32, hi: i32) -> bool {
    (lo..=hi).contains(&v)
}

/// Date and time fields: (year offset at, month at, day at); time at 0x20.. with `hours`.
fn date_ok(d: &[u8], min_len: usize, y: usize, hours: (i32, i32)) -> bool {
    d.len() >= min_len
        && in_range(i32_at(d, y), 12, 50)
        && in_range(i32_at(d, y + 4), 1, 12)
        && in_range(i32_at(d, y + 8), 1, 31)
        && in_range(i32_at(d, 0x20), hours.0, hours.1)
        && in_range(i32_at(d, 0x24), 0, 59)
        && in_range(i32_at(d, 0x28), 0, 59)
}

fn encode(d: &[u8], y: usize, hour: i32, ms_at: Option<usize>) -> Option<f64> {
    let ms = match ms_at {
        Some(at) => u32::try_from(i32_at(d, at)).ok()?,
        None => 0,
    };
    encode_date_time(
        i32_at(d, y) + 2000,
        u32::try_from(i32_at(d, y + 4)).ok()?,
        u32::try_from(i32_at(d, y + 8)).ok()?,
        u32::try_from(hour).ok()?,
        u32::try_from(i32_at(d, 0x24)).ok()?,
        u32::try_from(i32_at(d, 0x28)).ok()?,
        ms,
    )
}

fn hemisphere(c: Option<&u8>) -> [u8; 1] {
    [c.copied().unwrap_or(0)]
}

impl Dod {
    fn variant_b(&mut self, ctx: &Ctx, d: &[u8], it1: &mut Item) -> bool {
        let Some(t) = encode(d, 0x48, i32_at(d, 0x20), Some(0x2c)) else { return false };
        it1.time = t + ctx.tz;
        if it1.time < 2.0 {
            return false;
        }
        it1.pt = PT_GPS;
        it1.x = gp_int_to_coordinate(f64::from(le_f32(d, 0x38).unwrap_or(0.0)));
        half_sphere_x(&mut it1.x, &hemisphere(d.get(0x3c)));
        it1.y = gp_int_to_coordinate(f64::from(le_f32(d, 0x30).unwrap_or(0.0)));
        half_sphere_y(&mut it1.y, &hemisphere(d.get(0x34)));
        it1.speed = f64::from(le_f32(d, 0x40).unwrap_or(0.0));
        it1.angle = f64::from(le_f32(d, 0x44).unwrap_or(0.0));
        self.see_format_id = "B";
        true
    }

    fn variant_c(&mut self, ctx: &Ctx, d: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let Some(t) = encode(d, 0x60, i32_at(d, 0x20), Some(0x2c)) else { return (false, false) };
        it1.time = t + ctx.tz;
        if it1.time < 2.0 {
            return (false, false);
        }
        it1.pt = PT_GPS;
        it1.x = gp_int_to_coordinate(le_f64(d, 0x40).unwrap_or(0.0));
        half_sphere_x(&mut it1.x, &hemisphere(d.get(0x48)));
        it1.y = gp_int_to_coordinate(le_f64(d, 0x30).unwrap_or(0.0));
        half_sphere_y(&mut it1.y, &hemisphere(d.get(0x38)));
        it1.speed = le_f64(d, 0x50).unwrap_or(0.0);
        it1.angle = le_f64(d, 0x58).unwrap_or(0.0);
        self.see_format_id = "C";
        let g = |at: usize| f64::from(i32_at(d, at)) / 1024.0;
        *it2 = Item { pt: PT_GSENSOR, time: 0.0, x: g(0x70), y: g(0x74), z: g(0x6c), ..Default::default() };
        if let Some(enc) = d.get(0x79..).filter(|e| !e.is_empty()) {
            let k = enc[0];
            let s: Vec<u8> = enc.iter().map(|&c| c.wrapping_sub(k).wrapping_add(b'$')).collect();
            let cols = split(&s, &CharSet::of(b","), SPLIT_KEEP_EMPTY);
            if cols.len() > 5 {
                parse_gprmc(&cols, it1, true, ctx.tz);
                self.see_format_id = "CC";
            }
        }
        (true, true)
    }

    fn variant_d(&mut self, ctx: &Ctx, d: &[u8], it1: &mut Item) -> bool {
        let mut hour = i32_at(d, 0x20);
        if hour > 23 {
            hour -= 24;
        }
        if hour < 0 {
            hour += 24;
        }
        let Some(t) = encode(d, 0x2c, hour, None) else { return false };
        it1.time = t - ctx.tz_user_shift;
        let (status, ns, ew) = (d.get(0x38).copied(), d.get(0x39).copied(), d.get(0x3a).copied());
        if !(it1.time >= 2.0
            && matches!(ew, Some(b'E' | b'W'))
            && matches!(ns, Some(b'N' | b'S'))
            && matches!(status, Some(b'A' | b'V')))
        {
            return false;
        }
        it1.pt = PT_GPS;
        it1.x = gp_int_to_coordinate(f64::from(le_f32(d, 0x40).unwrap_or(0.0)));
        half_sphere_x(&mut it1.x, &hemisphere(d.get(0x3a)));
        it1.y = gp_int_to_coordinate(f64::from(le_f32(d, 0x3c).unwrap_or(0.0)));
        half_sphere_y(&mut it1.y, &hemisphere(d.get(0x39)));
        it1.speed = KNOTS_TO_KMH * f64::from(le_f32(d, 0x44).unwrap_or(0.0));
        it1.angle = f64::from(le_f32(d, 0x48).unwrap_or(0.0));
        if status != Some(b'A') {
            it1.warn = 2;
        }
        self.see_format_id = "D";
        true
    }

    fn variant_a(&mut self, ctx: &Ctx, d: &mut [u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let len = d.len();
        let mut i = 0x20usize;
        let mut max_cnt = 0i32;
        let mut it_max: Option<usize> = None;
        while i + 0x34 <= len {
            let r = &d[i..i + 0x34];
            let marks = [(0x15, b'A'), (0x16, b'T'), (0x17, b'C'), (0x1d, b'0'), (0x1e, b'0'), (0x1f, b'1')];
            if marks.iter().filter(|&&(o, c)| r[o] == c).count() < 3 {
                break;
            }
            let (k1, k2) = (d[i + 0x14], d[i + 0x1c]);
            for z in (0..0x14).chain(0x18..0x1c) {
                d[i + z] ^= k1;
            }
            for z in 0x20..0x34 {
                d[i + z] ^= k2;
            }
            let cnt = i32_at(d, i + 8);
            if max_cnt < cnt {
                max_cnt = cnt;
                it_max = Some(i);
            }
            i += 0x34;
        }
        let mut res1 = false;
        if self.last_cnt != max_cnt
            && let Some(m) = it_max
        {
            let r = &d[m..m + 0x34];
            let year = le_i16(r, 44).unwrap_or(0);
            if let Some(t) = encode_date_time(
                i32::from(year),
                u32::from(r[46]),
                u32::from(r[47]),
                u32::from(r[13]),
                u32::from(r[14]),
                u32::from(r[15]),
                0,
            ) {
                it1.pt = PT_GPS;
                it1.time = t + ctx.tz;
                it1.x = f64::from(i32_at(r, 24)) / 1e7;
                it1.y = f64::from(i32_at(r, 16)) / 1e7;
                it1.angle = f64::from(le_i16(r, 36).unwrap_or(0)) / 100.0;
                if it1.angle < 0.0 {
                    it1.angle += 360.0;
                }
                it1.speed = f64::from(i32_at(r, 32)) / 28.0;
                res1 = true;
                self.see_format_id = "A";
            }
        }
        self.last_cnt = max_cnt;
        let mut res2 = false;
        if i + 0xc <= len {
            let g = |at: usize| -f64::from(d[at] as i8) / 64.0;
            *it2 = Item { pt: PT_GSENSOR, x: g(i), z: g(i + 4), y: g(i + 8), ..Default::default() };
            res2 = it2.x != 0.0 || it2.y != 0.0 || it2.z != 0.0;
        }
        (res1, res2)
    }
}

impl Format for Dod {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, _data: &[u8]) -> DataTest {
        DataTest::No
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {
        self.last_cnt = 0;
        self.see_format_id = "";
    }

    fn after_load_data(&mut self, ctx: &mut Ctx) {
        if !self.see_format_id.is_empty() {
            ctx.info.format_id = format!("{}_{}", SPEC.id(), self.see_format_id);
        }
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let Some(payload) = line.get(LINE_PREFIX.len()..) else { return (false, false) };
        let mut d = payload.to_vec();
        if date_ok(&d, 0x54, 0x48, (0, 23)) {
            (self.variant_b(ctx, &d, it1), false)
        } else if date_ok(&d, 0x78, 0x60, (0, 23)) {
            self.variant_c(ctx, &d, it1, it2)
        } else if date_ok(&d, 0x4c, 0x2c, (-12, 47)) {
            (self.variant_d(ctx, &d, it1), false)
        } else {
            self.variant_a(ctx, &mut d, it1, it2)
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::gps::format::RawItem;
    use crate::gps::manager::{Env, load_items};
    use crate::gps::util::SEC;

    /// A layout-D payload (payload = atom from byte 16).
    pub fn payload_d(h: i32, m: i32, s: i32, lat: f32, lon: f32, knots: f32, status: u8) -> Vec<u8> {
        let mut d = vec![0u8; 0x4c];
        for (at, v) in [(0x20, h), (0x24, m), (0x28, s), (0x2c, 16), (0x30, 5), (0x34, 16)] {
            d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        d[0x38] = status;
        d[0x39] = b'N';
        d[0x3a] = b'E';
        d[0x3c..0x40].copy_from_slice(&lat.to_le_bytes());
        d[0x40..0x44].copy_from_slice(&lon.to_le_bytes());
        d[0x44..0x48].copy_from_slice(&knots.to_le_bytes());
        d[0x48..0x4c].copy_from_slice(&90.0f32.to_le_bytes());
        d
    }

    fn line(payload: &[u8]) -> Vec<u8> {
        [LINE_PREFIX, payload].concat()
    }

    fn run(items: Vec<RawItem>) -> Option<crate::gps::manager::Loaded> {
        let env = Env { name: "a.mov".into(), file_times: (0.0, 0.0), tz: 0.0, companion: &|_| None };
        load_items(&mut crate::gps::formats::all(), None, items, 0.0, "mov", 4.0 * SEC, &env)
    }

    #[test]
    fn layout_d() {
        let items = (0..3)
            .map(|k| {
                RawItem::timed(
                    f64::from(k) * SEC,
                    line(&payload_d(13, 6, 26 + k, 6003.8176 - k as f32 * 0.01, 3019.6213, 20.0, b'A')),
                )
            })
            .collect();
        let l = run(items).unwrap();
        assert_eq!(l.info.format_id, "movDOD_D");
        let gps: Vec<_> = l.points.iter().filter(|p| p.pt == PT_GPS && p.y != 0.0).collect();
        assert_eq!(gps.len(), 3);
        assert!((gps[0].y - 60.063_626).abs() < 1e-5 && (gps[0].x - 30.327_021).abs() < 1e-5);
        assert!((gps[0].speed - 20.0 * KNOTS_TO_KMH).abs() < 1e-9);
    }

    #[test]
    fn layout_a_xor_records() {
        let mut d = vec![0u8; 0x20 + 2 * 0x34 + 12];
        for (k, cnt) in [(0usize, 5i32), (1, 9)] {
            let mut r = [0u8; 0x34];
            r[8..12].copy_from_slice(&cnt.to_le_bytes());
            r[13] = 13;
            r[14] = 6;
            r[15] = 26 + k as u8;
            r[16..20].copy_from_slice(&600_636_270i32.to_le_bytes());
            r[24..28].copy_from_slice(&303_270_220i32.to_le_bytes());
            r[32..36].copy_from_slice(&(28 * 50i32).to_le_bytes());
            r[36..38].copy_from_slice(&(-9000i16).to_le_bytes());
            r[44..46].copy_from_slice(&2016i16.to_le_bytes());
            r[46] = 5;
            r[47] = 16;
            let (k1, k2) = (0x5au8, 0xa5u8);
            for z in (0..0x14).chain(0x18..0x1c) {
                r[z] ^= k1;
            }
            for b in &mut r[0x20..0x34] {
                *b ^= k2;
            }
            r[0x14] = k1;
            r[0x1c] = k2;
            r[0x15..0x18].copy_from_slice(b"ATC");
            r[0x1d..0x20].copy_from_slice(b"001");
            d[0x20 + k * 0x34..0x20 + (k + 1) * 0x34].copy_from_slice(&r);
        }
        let g = 0x20 + 2 * 0x34;
        d[g] = (-64i8) as u8;
        let l = run(vec![RawItem::timed(0.0, line(&d))]).unwrap();
        assert_eq!(l.info.format_id, "movDOD_A");
        let p = l.points.iter().find(|p| p.pt == PT_GPS && p.y != 0.0).unwrap();
        assert!((p.y - 60.063627).abs() < 1e-9 && (p.x - 30.327022).abs() < 1e-9);
        assert_eq!((p.speed, p.angle), (50.0, 270.0));
        let s = l.points.iter().find(|p| p.pt == PT_GSENSOR).unwrap();
        assert_eq!(s.x, 1.0);
    }

    #[test]
    fn garbage_blocks_do_not_panic() {
        for seed in 0..200u64 {
            let n = (seed as usize * 7) % 300;
            let d = crate::gps::test_support::noise(seed, n);
            let _ = run(vec![
                RawItem::timed(0.0, line(&d)),
                RawItem::timed(SEC, line(&payload_d(30, 70, 0, 1.0, 2.0, 0.0, 0))),
            ]);
        }
    }
}
