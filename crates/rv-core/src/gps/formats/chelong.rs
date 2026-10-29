//! `UnitGps_Chelong`: Chelong records `$lon,lat,speed,course,alt,...,gx,gy,gz$`: only digits,
//! ',' and '$' (no signs). Longitude and latitude are integers in 1/10000 degree, speed (km/h),
//! course (0 = unknown) and altitude are integers (the altitude only with 11 or more fields),
//! the last three fields are the G-sensor x, y, z: the low byte of the number as a signed byte
//! in 1/128 g.
//!
//! The records carry no time: both the fix and the G-sensor sample get the source's start time
//! plus the record's video frame time (for a log without frame times that is the file's
//! creation time minus one day). The clean-up passes are off for this data (`doPostProcessFix`).

use crate::gps::format::{Ctx, Format, Item, PT_GPS, Spec, default_test_load_item};
use crate::gps::util::{CharSet, SPLIT_KEEP_EMPTY, split, str_to_int, trim};

static SPEC: Spec = Spec {
    title: "Chelong",
    format_id: "Che",
    must_be_chars_in_line: CharSet::of(b"$,"),
    // everything but '$', ',' and the digits
    bad_chars: CharSet::ranges(&[(0x00, 0x23), (0x25, 0x2b), (0x2d, 0x2f), (0x3a, 0xff)]),
    ..crate::gps::formats::EMPTY_SPEC
};

pub struct Chelong;

pub fn create() -> Box<dyn Format> {
    Box::new(Chelong)
}

impl Format for Chelong {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    /// Accepts `$...$` (after trimming) and stores the trimmed line back into the record.
    fn test_load_item(&mut self, ctx: &mut Ctx, line: &[u8]) -> bool {
        let t = trim(line);
        if !(t.len() > 5 && t.first() == Some(&b'$') && t.last() == Some(&b'$')) {
            return false;
        }
        if t != line
            && let Some(item) = ctx.items.get_mut(ctx.index)
        {
            item.line = t.to_vec();
        }
        default_test_load_item(&SPEC, t)
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item, it2: &mut Item) -> (bool, bool) {
        // The original reads the record's (trimmed) line, not the line it was offered.
        let line = ctx.items.get(ctx.index).map_or(line, |i| i.line.as_slice()).to_vec();
        let cols = split(&line, &CharSet::of(b"$,"), SPLIT_KEEP_EMPTY);
        let n = cols.len();
        if n <= 8 {
            return (false, false);
        }
        let int = |i: usize| str_to_int(&cols[i], 0);
        it.pt = PT_GPS;
        it.x = f64::from(int(1)) / 10_000.0;
        it.y = f64::from(int(2)) / 10_000.0;
        if n > 10 {
            it.z = f64::from(int(5));
        }
        it.speed = f64::from(int(3));
        it.angle = f64::from(int(4));
        if it.angle == 0.0 {
            it.angle = -1.0;
        }
        it.time = ctx.info.date_time_start + ctx.frame_time();
        // `ShortInt(StrToIntDef(..))`: the low byte as a signed byte
        let g = |i: usize| f64::from(int(i) as u8 as i8) / 128.0;
        *it2 = Item::gsensor(it.time, g(n - 3), g(n - 2), g(n - 1));
        (true, true)
    }

    fn do_post_process_fix(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::format::{PT_GPS, PT_GSENSOR, RawItem};
    use crate::gps::formats;
    use crate::gps::manager::{Env, load_items};
    use crate::gps::test_support::*;
    use crate::gps::util::SEC;

    const LINES: [&str; 3] = [
        "$303457,600636,45,90,12,1,2,3,10,246,127$",
        "  $303467,600646,46,0,13,1,2,3,0,0,128$ ",
        "$303477,600656,47,92,14,1,2,3,64,192,128$",
    ];

    #[test]
    fn records_timed_by_video_frames() {
        let finish = 42_506.5;
        let duration = 10.0 * SEC;
        let items: Vec<RawItem> =
            LINES.iter().enumerate().map(|(i, l)| RawItem::timed((i as f64 + 1.0) * SEC, l.as_bytes())).collect();
        let none = |_: &str| None;
        let env = Env { name: "a.avi".into(), file_times: (finish - 1.0, finish), tz: 0.0, companion: &none };
        let loaded = load_items(&mut formats::all(), None, items, 0.0, "mov", duration, &env).unwrap();
        assert_eq!(loaded.info.format_id, "movChe");
        assert!(loaded.info.do_not_post_process_fix);
        let gps: Vec<_> = loaded.points.iter().filter(|p| p.pt == PT_GPS && p.x != 0.0).collect();
        let gs: Vec<_> = loaded.points.iter().filter(|p| p.pt == PT_GSENSOR).collect();
        assert_eq!((gps.len(), gs.len()), (3, 3));
        let start = finish - duration;
        for (i, p) in gps.iter().enumerate() {
            assert!((p.time - (start + (i as f64 + 1.0) * SEC)).abs() < 1e-9);
            assert_eq!(gs[i].time, p.time);
        }
        assert_eq!((gps[0].x, gps[0].y, gps[0].z), (30.3457, 60.0636, 12.0));
        assert_eq!((gps[0].speed, gps[0].angle), (45.0, 90.0));
        assert_eq!(gps[1].angle, -1.0, "course 0 is unknown");
        assert_eq!((gs[0].x, gs[0].y, gs[0].z), (10.0 / 128.0, -10.0 / 128.0, 127.0 / 128.0));
        assert_eq!((gs[2].x, gs[2].y, gs[2].z), (0.5, -0.5, -1.0));
    }

    #[test]
    fn log_files() {
        let create = 42_506.5;
        // A file passes only without line breaks or blanks (bad characters): one record, timed
        // at the creation time minus one day (frame time -1).
        let r = load_log_at("a.txt", LINES[0].as_bytes(), (create, create + 0.01)).unwrap();
        assert_eq!(r.format, "Che");
        let t = &r.track;
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-15 12:00:00");
        assert_eq!((t.gps.len(), t.gsensor.len()), (1, 1));
        assert!(load_log_at("a.txt", LINES.join("\n").as_bytes(), (create, create)).is_err());
        // a sign is a bad character; short records are ignored
        assert!(load_log_at("a.txt", b"$303457,-600636,45,90,12,1,2,3,10,246,127$", (create, create)).is_err());
        assert!(load_log_at("a.txt", b"$303457,600636,45,90,1,2,3$", (create, create)).is_err());
    }

    #[test]
    fn fuzz_chelong() {
        fuzz("a.txt", LINES[0].as_bytes());
        // records from a video: every format is offered every record
        let none = |_: &str| None;
        let env = Env { name: "a.avi".into(), file_times: (0.0, 42_506.5), tz: 0.0, companion: &none };
        let alphabet = b"$,0123456789 -";
        for seed in 0..60u64 {
            let items: Vec<RawItem> = (0..20u64)
                .map(|k| {
                    let raw = noise(seed * 31 + k, (seed as usize * 7 + k as usize * 3) % 70);
                    let mut l: Vec<u8> = raw.iter().map(|&c| alphabet[usize::from(c) % alphabet.len()]).collect();
                    if seed % 2 == 0 {
                        l.insert(0, b'$');
                        l.push(b'$');
                    }
                    RawItem::timed(k as f64 * SEC, l)
                })
                .collect();
            let _ = load_items(&mut formats::all(), None, items, 0.0, "mov", 30.0 * SEC, &env);
        }
    }
}
