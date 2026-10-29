//! `UnitGps_NMEA`: NMEA 0183 logs. Fixes come from RMC sentences (checksum checked), the
//! altitude from the last GGA. Also reads the G-sensor lines some cameras interleave:
//! `$GSENSOR,x,y,z` / `$GS,...` (1/1000 g), `$GSENS,...` (1/128 g, ".ifo"), `$GSENSORD,y,x,z`
//! ("Mio") and `$A x y z` ("Atc"), and BlackVue's "[ms]$GPRMC..." lines ("gpsBV").
//! `$PARKINGMODE` marks a parking-mode recording.

use crate::gps::format::{
    Ctx, DataTest, Format, Item, PT_GPS, PT_GSENSOR, Spec, default_test_load_data, default_test_load_item,
};
use crate::gps::util::{CharSet, SPLIT_KEEP_EMPTY, parse_gprmc, pos, split, str_to_float, str_to_int, trim};

static SPEC: Spec = Spec {
    title: "National Marine Electronics Association",
    def_ext: "nmea",
    format_id: "NMEA",
    must_be_chars_in_line: CharSet::EMPTY,
    must_be_chars_in_data: CharSet::of(b"$,"),
    bad_chars: CharSet::EMPTY,
    must_be_strings_in_data: &[],
    data_prefix: b"",
    line_prefix: b"",
};

#[derive(Default)]
pub struct Nmea {
    last_h: f64,
    /// 0 plain, 1 BlackVue, 2 ifo, 3 Mio, 4 Atc.
    variant: u8,
}

pub fn create() -> Box<dyn Format> {
    Box::new(Nmea::default())
}

impl Format for Nmea {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn all_ext(&self) -> Vec<&'static str> {
        vec!["nmea", "ifo", "gps", "txt"]
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, data: &[u8]) -> DataTest {
        let mut ok = default_test_load_data(&SPEC, data);
        if !ok && data.len() > 10 && data[1] == b'$' {
            ok = default_test_load_data(&SPEC, &data[1..]);
        }
        DataTest::from(ok)
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        let Some(&first) = line.first() else { return false };
        let ok = first == b'$' || pos(b"$", line).is_some_and(|p| p < 0x7f);
        ok && default_test_load_item(&SPEC, line)
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {
        self.last_h = 0.0;
        self.variant = 0;
    }

    fn after_load_data(&mut self, ctx: &mut Ctx) {
        let id = match self.variant {
            1 => "gpsBV",
            2 => "ifo",
            3 => "Mio",
            4 => "Atc",
            _ => return,
        };
        ctx.info.format_id = id.to_string();
    }

    fn load_item1(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let mut s: Vec<u8> = Vec::new();
        if let Some(i) = pos(b"$", line) {
            s = trim(&line[i..]).to_vec();
            if line[0] == b'[' && i >= 1 && line[i - 1] == b']' {
                self.variant = 1;
            }
        }
        if s == b"$PARKINGMODE" {
            ctx.info.parking_mode = true;
            return false;
        }
        if s.len() < 10 || s[0] != b'$' {
            return false;
        }
        if s[1] == b'A' && s[2] == b' ' {
            for c in &mut s {
                if *c == b' ' {
                    *c = b',';
                }
            }
        }
        let cols = split(&s, &CharSet::of(b","), SPLIT_KEEP_EMPTY);
        let Some(head) = cols.first().map(|c| c.as_slice()) else { return false };
        if cols.len() > 3 && matches!(head, b"$GSENSOR" | b"$GS" | b"$GSENS") {
            let div = if head == b"$GSENS" {
                self.variant = 2;
                128.0
            } else {
                1000.0
            };
            let v = |i: usize| f64::from(str_to_int(&cols[i], 0)) / div;
            *it = Item::gsensor(0.0, v(1), v(2), v(3));
            return true;
        }
        if cols.len() >= 4 && matches!(head, b"$GSENSORD" | b"$A") {
            self.variant = if head == b"$A" { 4 } else { 3 };
            let v = |i: usize| str_to_float(trim(&cols[i]), 0.0);
            *it = Item { pt: PT_GSENSOR, y: v(1), x: v(2), z: v(3), ..Default::default() };
            return true;
        }
        if head.len() > 5 && head[1] == b'G' {
            if &head[3..6] == b"RMC" {
                if parse_gprmc(&cols, it, true, ctx.tz) && it.time > 2.0 {
                    it.pt = PT_GPS;
                    it.z = self.last_h;
                    return true;
                }
                return false;
            }
            if &head[3..6] == b"GGA"
                && let Some(h) = cols.get(9)
            {
                self.last_h = str_to_float(h, 0.0);
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    const LOG: &str = "\
$GPGGA,130626.000,6003.8176,N,03019.6213,E,1,08,1.0,25.5,M,17.0,M,,0000*00
$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,37.85,112.00,160516,,,A*00
$GPGGA,130627.000,6003.8100,N,03019.6600,E,1,08,1.0,26.5,M,17.0,M,,0000*00
$GPRMC,130627.000,A,6003.8100,N,03019.6600,E,37.90,112.10,160516,,,A*00
$GPRMC,130628.000,A,6003.8000,N,03019.7000,E,38.00,112.20,160516,,,A*00
$GPRMC,130629.000,V,6003.7900,N,03019.7400,E,38.10,112.30,160516,,,N*00
$GSENSOR,12,-34,980
";

    #[test]
    fn rmc_with_gga_altitude() {
        // every checksum fixed except line 5 (index 4)
        let log = fix_checksums(LOG, &[0, 1, 2, 3, 5]);
        let r = load_log("trip.nmea", log.as_bytes()).unwrap();
        assert_eq!(r.format, "NMEA");
        let t = &r.track;
        // the RMC with a bad checksum is dropped; the void fix is kept with a warning
        assert_eq!(t.gps.len(), 3, "{:?}", t.gps);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert!((p.lat - 60.063_626_666).abs() < 1e-8 && (p.lon - 30.327_021_666).abs() < 1e-8);
        assert!((p.speed_kmh - 37.85 * 1.85325).abs() < 1e-9);
        assert_eq!((p.course, p.altitude, p.warning), (112.0, 25.5, 0));
        assert_eq!(t.gps[1].altitude, 26.5);
        assert_eq!((t.gps[2].time, t.gps[2].warning), (3.0, 2));
        assert_eq!(t.gsensor.len(), 0, "a single G-sensor sample is dropped");
    }

    #[test]
    fn checksum_variants_and_noise() {
        // no checksum on a 13-field RMC with a one-letter mode; leading junk before '$'
        let log = "xx$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,1.0,10.0,160516,,,A\n\
                   noise line without dollar\n\
                   $GPRMC,130627.000,A,6003.8276,N,03019.6213,E,1.0,10.0,160516,,,A\n";
        let r = load_log("a.nmea", log.as_bytes()).unwrap();
        assert_eq!(r.track.gps.len(), 2);
        assert!(load_log("a.nmea", b"$GPRMC,1,2,3\n").is_err());
    }

    #[test]
    fn gsensor_variants() {
        let log = fix_checksums(
            "$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,1.0,10.0,160516,,,A*00
             $GSENS,128,-64,256
$GSENS,64,-64,128
             $GPRMC,130627.000,A,6003.8276,N,03019.6213,E,1.0,10.0,160516,,,A*00
",
            &[0, 3],
        );
        let r = load_log("a.ifo", log.as_bytes()).unwrap();
        assert_eq!(r.format, "ifo");
        let g = &r.track.gsensor;
        assert_eq!(g.len(), 2);
        assert_eq!((g[0].x, g[0].y, g[0].z), (1.0, -0.5, 2.0));
        assert_eq!(g[0].time, 0.0);
        let bv = fix_checksums(
            "[1000]$GPRMC,130626.000,A,6003.8176,N,03019.6213,E,1.0,10.0,160516,,,A*00
             [2000]$GPRMC,130627.000,A,6003.8276,N,03019.6213,E,1.0,10.0,160516,,,A*00
",
            &[0, 1],
        );
        assert_eq!(load_log("a.gps", bv.as_bytes()).unwrap().format, "gpsBV");
    }

    #[test]
    fn fuzz_nmea() {
        fuzz("a.nmea", fix_checksums(LOG, &[0, 1, 2, 3, 5]).as_bytes());
    }
}
