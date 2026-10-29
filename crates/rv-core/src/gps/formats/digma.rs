//! `UnitGps_Digma` (a `TGpsFormat_GPRMC` descendant): Digma cameras' GPS and G-sensor data
//! (as in a video's user data, which the original splits with the same `SplitRawData`), in
//! two layouts:
//! - "plain": boxes (big-endian u32 size + 4-character id) "TIME" (i32 Unix times, UTC),
//!   "GPSI" (records of 128 bytes holding an RMC sentence, or of 28 bytes: NUL-terminated
//!   NMEA "DDMM.MMMM" latitude in 12 bytes and "DDDMM.MMMM" longitude in 16, without
//!   hemispheres) and "SENS" (3 x f32 per sample, in g: x, z, y);
//! - "planar": records "TIME" + 4 bytes, "GPSI" + 128 bytes, "SENS" + 12 bytes one after the
//!   other.
//!
//! Records become lines "GPSI$GPRMC..." (read as RMC with checksum, UTC + time zone),
//! "GPSX" + 28 bytes and "SENS" + 12 bytes. The 28-byte fixes have no clock of their own: their
//! time is the file's start time plus the "TIME" value relative to the first one; they have
//! no speed and course (0).

use crate::gps::format::{Ctx, Format, Item, PT_GPS, PT_GSENSOR, RawItem, Spec};
use crate::gps::formats::{EMPTY_SPEC, gprmc::Gprmc};
use crate::gps::util::{
    DIGITS, copy, gp_int_to_coordinate, le_f32, le_i32, str_to_float, system_time_to_date_time_def, trim,
};

static SPEC: Spec = Spec { title: "digma", ..EMPTY_SPEC };

/// Bytes per RMC record of a "GPSI" box.
const GPSI_LEN: usize = 0x80;
/// Bytes per coordinate record of a "GPSI" box ("GPS2").
const GPSX_LEN: usize = 0x1c;
/// Bytes per "SENS" sample.
const SENS_LEN: usize = 0xc;

pub struct Digma;

pub fn create() -> Box<dyn Format> {
    Box::new(Digma)
}

/// The boxes of the "plain" layout (`ExtractSubnodes`): the last "TIME", "GPSI" and "SENS".
#[derive(Default)]
struct Nodes {
    time: Vec<u8>,
    gpsi: Vec<u8>,
    sens: Vec<u8>,
}

fn extract_subnodes(data: &[u8]) -> Nodes {
    let mut nodes = Nodes::default();
    let mut s = data;
    while s.len() > 8 {
        let Some(&[a, b, c, d]) = s.first_chunk::<4>() else { break };
        let l = i32::from_be_bytes([a, b, c, d]);
        let Ok(l) = usize::try_from(l) else { break };
        if l < 8 || s.len() < l {
            break;
        }
        let node = s[8..l].to_vec();
        match &s[4..8] {
            b"TIME" => nodes.time = node,
            b"GPSI" => nodes.gpsi = node,
            b"SENS" => nodes.sens = node,
            _ => {}
        }
        s = &s[l..];
    }
    nodes
}

/// `TimeToDateTime`: Unix seconds (UTC) to TDateTime; 0 outside 1901..2049.
fn time_to_date_time(t: i32) -> f64 {
    use chrono::{Datelike, Timelike};
    let Some(dt) = chrono::DateTime::from_timestamp(i64::from(t), 0) else { return 0.0 };
    let n = dt.naive_utc();
    system_time_to_date_time_def(
        n.year(),
        n.month() as i32,
        n.day() as i32,
        n.hour() as i32,
        n.minute() as i32,
        n.second() as i32,
        0,
        0.0,
    )
}

/// `SplitPlaint`: one GPS line and one "SENS" line per sample; as many samples as all three
/// boxes have. 28-byte fixes are timed by "TIME" relative to the first sample, whose time is
/// returned as the time of video frame 0.
fn split_plain(nodes: &Nodes) -> Option<(Vec<RawItem>, f64)> {
    let g = &nodes.gpsi;
    let gps2 = g.len() >= GPSX_LEN && DIGITS.contains(g[0]) && g[11] == 0 && DIGITS.contains(g[12]) && g[27] == 0;
    let gps_size = if gps2 { GPSX_LEN } else { GPSI_LEN };
    let n = (g.len() / gps_size).min(nodes.sens.len() / SENS_LEN).min(nodes.time.len() / 4);
    let mut frame0 = 0.0;
    let mut items = Vec::with_capacity(2 * n);
    for i in 0..n {
        let mut gps = RawItem::default();
        if gps2 {
            gps.line = b"GPSX".to_vec();
            gps.line.extend_from_slice(copy(g, i * GPSX_LEN + 1, GPSX_LEN));
            let t = time_to_date_time(le_i32(&nodes.time, i * 4)?);
            if i == 0 {
                frame0 = t;
            }
            gps.frame_time = t - frame0;
        } else {
            gps.line = b"GPSI".to_vec();
            gps.line.extend_from_slice(trim(copy(g, i * GPSI_LEN + 1, GPSI_LEN)));
        }
        items.push(gps);
        let mut sens = b"SENS".to_vec();
        sens.extend_from_slice(copy(&nodes.sens, i * SENS_LEN + 1, SENS_LEN));
        items.push(RawItem::line(sens));
    }
    (!items.is_empty()).then_some((items, frame0))
}

/// `SplitPlanar`: the "GPSI" and "SENS" records (id included) up to the first unknown id;
/// "TIME" records are skipped.
fn split_planar(data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
    let mut items = Vec::new();
    let mut p = 0usize;
    while data.len() > p + 5 {
        let p0 = p;
        // the id is read as a C string
        let id = &data[p..p + 4];
        let id = id.iter().position(|&c| c == 0).map_or(id, |z| &id[..z]);
        p += 4;
        p += match id {
            b"TIME" => {
                p += 4;
                continue;
            }
            b"SENS" => SENS_LEN,
            b"GPSI" => GPSI_LEN,
            _ => break,
        };
        items.push(RawItem::line(copy(data, p0 + 1, p - p0)));
    }
    (!items.is_empty()).then_some((items, 0.0))
}

/// `GPStrToCoordinate(s, 2, 0)` of a NUL-padded field: the number ends at the first NUL
/// (Delphi's `TextToFloat` reads a C string).
fn nmea_coordinate(field: &[u8]) -> f64 {
    if field.len() < 4 {
        return 0.0;
    }
    let s = field.iter().position(|&c| c == 0).map_or(field, |z| &field[..z]);
    gp_int_to_coordinate(str_to_float(s, 0.0))
}

impl Format for Digma {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn split_raw_data(&mut self, data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
        let nodes = extract_subnodes(data);
        if !nodes.time.is_empty() || !nodes.gpsi.is_empty() || !nodes.sens.is_empty() {
            return split_plain(&nodes);
        }
        if copy(data, 1, 4) == b"TIME" && copy(data, 9, 4) == b"GPSI" {
            return split_planar(data);
        }
        None
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        match copy(line, 1, 4) {
            b"GPSI" => line.get(4) == Some(&b'$'),
            b"GPSX" => line.len() == 4 + GPSX_LEN,
            b"SENS" => line.len() == 4 + SENS_LEN,
            _ => false,
        }
    }

    fn load_item1(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        match copy(line, 1, 4) {
            b"GPSI" => Gprmc.load_item1(ctx, line, it),
            b"GPSX" => {
                it.pt = PT_GPS;
                it.time = ctx.info.date_time_start + ctx.frame_time();
                it.x = nmea_coordinate(copy(line, 17, 16));
                it.y = nmea_coordinate(copy(line, 5, 12));
                true
            }
            b"SENS" => {
                let (Some(x), Some(z), Some(y)) = (le_f32(line, 4), le_f32(line, 8), le_f32(line, 12)) else {
                    return false;
                };
                if !([x, y, z].iter().all(|v| *v != 0.0 && v.abs() < 10.0)) {
                    return false;
                }
                it.pt = PT_GSENSOR;
                it.x = f64::from(x);
                it.y = f64::from(y);
                it.z = f64::from(z);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::test_support::*;
    use crate::gps::util::SEC;

    /// 2016-05-16 12:30:45 UTC as Unix time.
    const UNIX0: i32 = 1_463_401_845;
    /// 2016-05-16 12:00:00 (file creation time).
    const T0: f64 = 42_506.5;

    fn boxed(id: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut b = u32::try_from(payload.len() + 8).unwrap().to_be_bytes().to_vec();
        b.extend_from_slice(id);
        b.extend_from_slice(payload);
        b
    }

    fn padded(s: &str, n: usize) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.resize(n, 0);
        v
    }

    fn sens(x: f32, y: f32, z: f32) -> Vec<u8> {
        [x, z, y].iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn rmc(time: &str, lat: &str, lon: &str) -> String {
        let body = format!("GPRMC,{time},A,{lat},N,{lon},E,10.0,90.0,160516,,,A");
        let x = body.bytes().fold(0u8, |a, c| a ^ c);
        format!("${body}*{x:02X}\r\n")
    }

    fn plain_gps2() -> Vec<u8> {
        let mut time = Vec::new();
        let mut gpsi = Vec::new();
        let mut s = Vec::new();
        for (i, (lat, lon)) in
            [("5545.0060", "03736.6120"), ("5545.0120", "03736.6240"), ("5545.0180", "03736.6360")].iter().enumerate()
        {
            time.extend_from_slice(&(UNIX0 + i as i32).to_le_bytes());
            gpsi.extend(padded(lat, 12));
            gpsi.extend(padded(lon, 16));
            let k = (i + 1) as f32;
            s.extend(sens(0.1 * k, -0.2 * k, 1.0 + 0.01 * k));
        }
        [boxed(b"udta", b"xx"), boxed(b"TIME", &time), boxed(b"GPSI", &gpsi), boxed(b"SENS", &s)].concat()
    }

    fn plain_rmc() -> Vec<u8> {
        let mut gpsi = Vec::new();
        gpsi.extend(padded(&rmc("123045.000", "5545.0060", "03736.6120"), GPSI_LEN));
        gpsi.extend(padded(&rmc("123046.000", "5545.0120", "03736.6240"), GPSI_LEN));
        let s = [sens(0.1, 0.2, 0.9), sens(0.2, 0.3, 1.0)].concat();
        [boxed(b"SENS", &s), boxed(b"GPSI", &gpsi), boxed(b"TIME", &[0; 12])].concat()
    }

    fn planar() -> Vec<u8> {
        let mut d = Vec::new();
        for (i, (t, lat)) in [("123045.000", "5545.0060"), ("123046.000", "5545.0120")].iter().enumerate() {
            d.extend_from_slice(b"TIME");
            d.extend_from_slice(&(UNIX0 + i as i32).to_le_bytes());
            d.extend_from_slice(b"GPSI");
            d.extend(padded(&rmc(t, lat, "03736.6120"), GPSI_LEN));
            d.extend_from_slice(b"SENS");
            d.extend(sens(0.1 + i as f32, 0.2, 0.9));
        }
        d
    }

    #[test]
    fn plain_coordinate_records() {
        let r = load_log_at("a.bin", &plain_gps2(), (T0, T0 + 60.0 * SEC)).unwrap();
        assert_eq!(r.format, "digma");
        let t = &r.track;
        assert_eq!(t.gps.len(), 3, "{:?}", t.gps);
        // file creation time + TIME relative to the first sample
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 12:00:00");
        for (k, p) in t.gps.iter().enumerate() {
            assert!((p.time - k as f64).abs() < 1e-3, "{}", p.time);
            assert!((p.lat - (55.75 + 0.0001 * (k + 1) as f64)).abs() < 1e-9);
            assert!((p.lon - (37.61 + 0.0002 * (k + 1) as f64)).abs() < 1e-9);
        }
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        let f = |v: f32| f64::from(v);
        assert_eq!(g, [(f(0.1), f(-0.2), f(1.01)), (f(0.2), f(-0.4), f(1.02)), (f(0.3), f(-0.6), f(1.03))]);
        // without a file time the coordinate records have no date
        assert!(load_log("a.bin", &plain_gps2()).map_or(true, |r| r.track.gps.is_empty()));
    }

    #[test]
    fn plain_rmc_records() {
        let r = load_log("a.bin", &plain_rmc()).unwrap();
        assert_eq!(r.format, "digma");
        let t = &r.track;
        assert_eq!(t.gps.len(), 2);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 12:30:45");
        assert_eq!(t.gps[1].time, 1.0);
        assert!((t.gps[0].lat - 55.7501).abs() < 1e-9 && (t.gps[0].lon - 37.6102).abs() < 1e-9);
        assert!((t.gps[0].speed_kmh - 18.5325).abs() < 1e-9);
        assert_eq!(t.gps[0].course, 90.0);
        let g: Vec<(f64, f64)> = t.gsensor.iter().map(|s| (s.time, s.z)).collect();
        assert_eq!(g, [(0.0, f64::from(0.9f32)), (1.0, f64::from(1.0f32))]);
    }

    #[test]
    fn planar_records() {
        let data = planar();
        let (items, _) = Digma.split_raw_data(&data).unwrap();
        assert_eq!(items.len(), 4, "TIME records are skipped");
        assert_eq!(items[0].line.len(), 4 + GPSI_LEN);
        assert_eq!(&items[1].line[..4], b"SENS");
        let r = load_log("a.bin", &data).unwrap();
        assert_eq!(r.format, "digma");
        assert_eq!(r.track.gps.len(), 2);
        assert_eq!(r.track.gps[1].time, 1.0);
        assert_eq!(r.track.gsensor.len(), 2);
        assert!((r.track.gsensor[1].x - 1.1).abs() < 1e-6);
    }

    #[test]
    fn records_and_lines() {
        let mut ctx = Ctx::default();
        let mut d = Digma;
        assert!(d.test_load_item(&mut ctx, b"GPSI$GPRMC,1,2,3"));
        assert!(!d.test_load_item(&mut ctx, b"GPSI GPRMC,1,2,3"));
        assert!(d.test_load_item(&mut ctx, &[b"SENS".as_slice(), &[1; 12]].concat()));
        assert!(!d.test_load_item(&mut ctx, &[b"SENS".as_slice(), &[1; 13]].concat()));
        assert!(!d.test_load_item(&mut ctx, &[b"GPSX".as_slice(), &[1; 27]].concat()));
        // a zero axis is not a sample
        let mut it = Item::default();
        assert!(!d.load_item1(&mut ctx, &[b"SENS".as_slice(), &sens(0.0, 0.2, 0.9)].concat(), &mut it));
        assert!(!d.load_item1(&mut ctx, &[b"SENS".as_slice(), &sens(10.0, 0.2, 0.9)].concat(), &mut it));
        assert!(d.load_item1(&mut ctx, &[b"SENS".as_slice(), &sens(-9.5, 0.2, 0.9)].concat(), &mut it));
        // fewer samples in one box limit all of them; nothing without all three boxes
        let short = [boxed(b"TIME", &[0; 8]), boxed(b"GPSI", &[b'1'; 3 * GPSI_LEN]), boxed(b"SENS", &[1; 36])].concat();
        assert_eq!(Digma.split_raw_data(&short).unwrap().0.len(), 4);
        assert!(Digma.split_raw_data(&boxed(b"SENS", &[1; 36])).is_none());
        assert!(Digma.split_raw_data(b"TIME....GPSX").is_none());
        assert_eq!(time_to_date_time(UNIX0), 42_506.0 + (12.0 * 3600.0 + 30.0 * 60.0 + 45.0) / 86_400.0);
    }

    #[test]
    fn fuzz_digma() {
        fuzz("a.bin", &plain_gps2());
        fuzz("a.bin", &plain_rmc());
        fuzz("a.bin", &planar());
    }
}
