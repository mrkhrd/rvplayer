//! `UnitGps_HDW`: HDW logs. A line may hold a G-sensor sample `$GS:z,x,y` (integers in 1/1000 g,
//! separated by ' ', ',', '_' or '$') and / or a fix
//! `$G:YYYY-MM-DD hh:mm:ss <N|S>lat <E|W>lon <?>speed` at fixed positions after "$G:"
//! ("N-" / "E-" mean south / west). The time is the camera's local time; coordinates are in
//! degrees, speed in km/h (its first character is skipped), course unknown.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::util::{
    CharSet, copy, gp_str_to_date_time, half_sphere_x, half_sphere_y, pos, split, str_to_float, str_to_int,
};

static SPEC: Spec = Spec { title: "HDW", ..crate::gps::formats::EMPTY_SPEC };

pub struct Hdw;

pub fn create() -> Box<dyn Format> {
    Box::new(Hdw)
}

/// `StringReplace(s, from, to, [rfReplaceAll])`.
fn replace_all(s: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if !from.is_empty() && s[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

/// `Copy(s, 2, Length(s))`: the field without its first character.
fn tail(s: &[u8]) -> &[u8] {
    copy(s, 2, s.len())
}

impl Format for Hdw {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        pos(b"$GS:", line).is_some() || pos(b"$G:", line).is_some()
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item, it2: &mut Item) -> (bool, bool) {
        let mut res1 = false;
        let mut res2 = false;
        let mut data = line.to_vec();
        // 1-based positions as in the original
        if let Some(p) = pos(b"$GS:", &data).map(|p| p + 1) {
            let cols = split(copy(&data, p + 4, data.len().min(16)), &CharSet::of(b" $,_"), 0);
            if cols.len() >= 3 {
                let g = |i: usize| f64::from(str_to_int(&cols[i], 99_999)) / 1000.0;
                *it = Item { pt: PT_GSENSOR, time: 0.0, z: g(0), x: g(1), y: g(2), ..*it };
                res1 = true;
            }
        }
        if let Some(q) = pos(b"$G:", &data).map(|p| p + 1) {
            let p = q + 3;
            it2.pt = PT_GPS;
            let two = |d: &[u8], at: usize| copy(d, at, 2).to_vec();
            let date = [two(&data, p + 8), two(&data, p + 5), two(&data, p + 2)].concat();
            let time = [two(&data, p + 11), two(&data, p + 14), two(&data, p + 17)].concat();
            it2.time = gp_str_to_date_time(&date, &time, ctx.tz);
            if it2.time > 2.0 {
                data = replace_all(&data, b"E-", b"W");
                data = replace_all(&data, b"N-", b"S");
                let cols = split(copy(&data, p + 20, data.len().min(40)), &CharSet::of(b" $,-_"), 0);
                if cols.len() >= 3 {
                    // the camera clock is local time
                    it2.time -= ctx.tz;
                    it2.y = str_to_float(tail(&cols[0]), 0.0);
                    it2.x = str_to_float(tail(&cols[1]), 0.0);
                    // split without empty fields: every field has a first character
                    half_sphere_y(&mut it2.y, copy(&cols[0], 1, 1));
                    half_sphere_x(&mut it2.x, copy(&cols[1], 1, 1));
                    it2.speed = str_to_float(tail(&cols[2]), -1.0);
                    it2.angle = -1.0;
                    res2 = true;
                }
            }
        }
        (res1, res2)
    }
}

#[cfg(test)]
mod tests {
    use super::replace_all;
    use crate::gps::test_support::*;

    const LOG: &str = "\
$G:2016-05-16 13:06:26 N55.502057 E37.502057 V45.5
$GS:980,-12,34
$G:2016-05-16 13:06:27 N-55.502157 E-37.502157 V46.5
$GS:990_-10_30
$GS:1000 0 20 $G:2016-05-16 13:06:28 N55.502257 E37.502257 V47.5
";

    #[test]
    fn replace() {
        assert_eq!(replace_all(b"N-1 E-2 N-", b"N-", b"S"), b"S1 E-2 S");
    }

    #[test]
    fn local_time_fixes_and_gsensor() {
        let r = load_log("hdw.txt", LOG.as_bytes()).unwrap();
        assert_eq!(r.format, "HDW");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        assert_eq!(t.gps.len(), 3);
        let p = t.gps[0];
        assert_eq!((p.lat, p.lon, p.speed_kmh), (55.502057, 37.502057, 45.5));
        let p = t.gps[1];
        assert_eq!((p.time, p.lat, p.lon), (1.0, -55.502157, -37.502157));
        assert_eq!(t.gps[2].time, 2.0);
        let g = &t.gsensor;
        assert_eq!(g.len(), 3);
        assert_eq!((g[0].x, g[0].y, g[0].z), (-0.012, 0.034, 0.98));
        assert_eq!((g[2].x, g[2].y, g[2].z), (0.0, 0.02, 1.0));
    }

    #[test]
    fn time_is_not_shifted_by_the_zone() {
        use crate::gps::{LoadOptions, load_log_bytes};
        let opts = LoadOptions { tz: 3.0 / 24.0, file_times: (0.0, 0.0) };
        let r = load_log_bytes("hdw.txt", LOG.as_bytes(), &opts, &|_| None).unwrap();
        assert_eq!(r.track.start.unwrap().format("%H:%M:%S").to_string(), "13:06:26");
    }

    #[test]
    fn fuzz_hdw() {
        fuzz("hdw.txt", LOG.as_bytes());
    }
}
