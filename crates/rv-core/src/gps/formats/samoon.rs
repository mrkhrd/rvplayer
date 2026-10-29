//! `UnitGps_Samoon` (a `TGpsFormat_GPRMC` descendant): files starting with "samoon", a
//! 16-byte header and 132-byte records. A record holds three little-endian i16 G-sensor
//! values (x, z, y in 1/1000 g), then an NMEA RMC sentence with every byte shifted by a
//! per-record key (the 7th byte is '$' + key); the sentence ends at its LF. Records become
//! lines "MySaMoOn" + record and are read with the GPRMC parser (checksum checked, UTC +
//! local offset, knots to km/h); each line also gives one G-sensor sample (no time of its
//! own).

use crate::gps::format::{Ctx, Format, Item, PT_GSENSOR, RawItem, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::formats::gprmc::Gprmc;
use crate::gps::util::copy;

static SPEC: Spec = Spec { title: "samn", data_prefix: b"samoon", line_prefix: b"MySaMoOn", ..EMPTY_SPEC };

/// Header length after "samoon" (`po = Length(dataPrefix) + 11` is 1-based).
const HEADER_REST: usize = 0xb;
const RECORD: usize = 0x84;

pub struct Samoon;

pub fn create() -> Box<dyn Format> {
    Box::new(Samoon)
}

/// Decodes the sentence of a record in place: from the 7th byte on, every byte minus
/// (7th byte - '$'), up to and including the first LF; the rest is dropped.
fn decode_record(s: &mut Vec<u8>) {
    if s.len() <= 7 {
        return;
    }
    let k = s[6].wrapping_sub(0x24);
    for i in 6..s.len() {
        s[i] = s[i].wrapping_sub(k);
        if s[i] == b'\n' {
            s.truncate(i + 1);
            break;
        }
    }
}

impl Format for Samoon {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn split_raw_data(&mut self, data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
        if data.len() <= 0x1d || copy(data, 1, SPEC.data_prefix.len()) != SPEC.data_prefix {
            return None;
        }
        let mut po = SPEC.data_prefix.len() + HEADER_REST; // 1-based
        let mut items = Vec::new();
        while data.len() >= po + 0x42 {
            let mut s = copy(data, po, RECORD).to_vec();
            decode_record(&mut s);
            let mut line = SPEC.line_prefix.to_vec();
            line.extend(s);
            items.push(RawItem::line(line));
            po += RECORD;
        }
        (!items.is_empty()).then_some((items, 0.0))
    }

    /// Only the line prefix (no character checks).
    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        copy(line, 1, SPEC.line_prefix.len()) == SPEC.line_prefix
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let res1 = Gprmc.load_item1(ctx, line, it1);
        let n = SPEC.line_prefix.len();
        if line.len() > n {
            // the original reads the three words even past the end of a short line
            let w = |o: usize| {
                let b = |i: usize| line.get(n + o + i).copied().unwrap_or(0);
                f64::from(i16::from_le_bytes([b(0), b(1)]))
            };
            it2.pt = PT_GSENSOR;
            it2.x = w(0) / 1000.0;
            it2.y = w(4) / 1000.0;
            it2.z = w(2) / 1000.0;
        }
        (res1, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::test_support::*;

    fn nmea(body: &str) -> String {
        let x = body.bytes().fold(0u8, |a, c| a ^ c);
        format!("${body}*{x:02X}\r\n")
    }

    fn record(g: [i16; 3], sentence: &str, key: u8) -> Vec<u8> {
        let mut r: Vec<u8> = g.iter().flat_map(|v| v.to_le_bytes()).collect();
        r.extend(sentence.bytes().map(|c| c.wrapping_add(key)));
        r.resize(RECORD, 0x5a);
        r
    }

    fn log() -> Vec<u8> {
        let mut data = b"samoon0123456789".to_vec();
        let fixes = [
            ([120, -340, 980], "GPRMC,130626.000,A,6003.8176,N,03019.6213,E,10.0,90.0,160516,,,A", 0x10),
            ([121, -341, 981], "GPRMC,130627.000,A,6003.8176,N,03019.6413,E,10.0,90.0,160516,,,A", 0x31),
            ([122, -342, 982], "GPRMC,130628.000,A,6003.8176,N,03019.6613,E,12.0,91.0,160516,,,A", 0xf0),
        ];
        for (g, body, key) in fixes {
            data.extend(record(g, &nmea(body), key));
        }
        data
    }

    #[test]
    fn records_with_gsensor() {
        let r = load_log("a.dat", &log()).unwrap();
        assert_eq!(r.format, "samn");
        let t = &r.track;
        assert_eq!(t.gps.len(), 3, "{:?}", t.gps);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let times: Vec<f64> = t.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [0.0, 1.0, 2.0]);
        let p = t.gps[0];
        assert!((p.lat - 60.063_626_666).abs() < 1e-8 && (p.lon - 30.327_021_666).abs() < 1e-8);
        assert!((p.speed_kmh - 10.0 * 1.85325).abs() < 1e-9);
        assert_eq!(t.gps[2].course, 91.0);
        // words: x, z, y
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.12, 0.98, -0.34), (0.121, 0.981, -0.341), (0.122, 0.982, -0.342)]);
    }

    #[test]
    fn split_and_decode() {
        let mut f = Samoon;
        let (items, t0) = f.split_raw_data(&log()).unwrap();
        assert_eq!((items.len(), t0), (3, 0.0));
        let l = &items[2].line;
        assert!(l.starts_with(b"MySaMoOn"));
        assert_eq!(&l[14..21], b"$GPRMC,");
        assert!(l.ends_with(b"\r\n"), "cut after the LF");
        assert_eq!(f.split_raw_data(b"samoon is too short"), None);
        assert_eq!(f.split_raw_data(&[b'x'; 300]), None);
        // a record needs at least 67 bytes
        assert_eq!(f.split_raw_data(&log()[..16 + 66]), None);
        assert_eq!(f.split_raw_data(&log()[..16 + 67]).map(|r| r.0.len()), Some(1));
    }

    #[test]
    fn bad_checksum_keeps_gsensor() {
        let mut data = log();
        data[16 + 6 + 20] ^= 1; // a digit of the first fix
        let r = load_log("a.dat", &data).unwrap();
        assert_eq!(r.track.gps.len(), 2);
        assert_eq!(r.track.gsensor.len(), 3);
    }

    #[test]
    fn fuzz_samoon() {
        fuzz("a.dat", &log());
    }
}
