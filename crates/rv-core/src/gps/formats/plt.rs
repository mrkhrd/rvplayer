//! `UnitGps_PLT`: OziExplorer track (.plt). The data must mention "OziExplorer" in its first
//! 4 KiB and contain no NUL or tab. From the 7th line on, each line is
//! `lat,lon,break,altitude ft (-777 = none),TDateTime (UTC),...`.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, Spec};
use crate::gps::util::{CharSet, SPLIT_KEEP_EMPTY, split, str_to_float, trim};

static SPEC: Spec = Spec {
    title: "Ozi Explorer",
    def_ext: "plt",
    must_be_chars_in_data: CharSet::of(b" ,."),
    bad_chars: CharSet::of(b"\0\t"),
    must_be_strings_in_data: &["ozi", "explorer"],
    ..super::EMPTY_SPEC
};

pub struct Plt;

pub fn create() -> Box<dyn Format> {
    Box::new(Plt)
}

impl Format for Plt {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn load_item1(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        if ctx.index <= 5 {
            return false;
        }
        let cols = split(line, &CharSet::of(b","), SPLIT_KEEP_EMPTY);
        if cols.len() <= 4 {
            return false;
        }
        let num = |i: usize| str_to_float(trim(&cols[i]), 0.0);
        it.pt = PT_GPS;
        it.y = num(0);
        it.x = num(1);
        it.z = num(3);
        it.z = if it.z == -777.0 { 0.0 } else { it.z * 0.3048 };
        it.time = num(4) + ctx.tz;
        it.angle = -1.0;
        it.speed = -1.0;
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    const PLT: &str = "OziExplorer Track Point File Version 2.1\r\n\
WGS 84\r\n\
Altitude is in Feet\r\n\
Reserved 3\r\n\
0,2,255,trip,0,0,2,8421376\r\n\
3\r\n\
  60.063627,  30.327022,0, 100.0,42506.5459027778,16-May-16,13:06:06\r\n\
  60.063527,  30.327522,0,-777,42506.5459143519,16-May-16,13:06:07\r\n\
  60.063427,  30.328022,0, 120.0,42506.5459259259,16-May-16,13:06:08\r\n";

    #[test]
    fn track_lines_after_the_header() {
        let r = load_log("trip.plt", PLT.as_bytes()).unwrap();
        assert_eq!(r.format, "plt");
        let t = &r.track;
        assert_eq!(t.gps.len(), 3);
        let p = t.gps[0];
        assert_eq!((p.lat, p.lon), (60.063627, 30.327022));
        assert!((p.altitude - 30.48).abs() < 1e-9, "feet to metres");
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:06");
        assert!((t.gps[2].time - 2.0).abs() < 1e-3);
        assert!(t.gps[1].speed_kmh > 0.0, "speed computed from positions");
        assert!(load_log("a.plt", PLT.replace("OziExplorer", "Track").as_bytes()).is_err());
        fuzz("a.plt", PLT.as_bytes());
    }
}
