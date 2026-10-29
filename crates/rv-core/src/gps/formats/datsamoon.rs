//! `UnitGps_DatSamoon`: Samoon ".dat" logs, text lines with every byte shifted by a Caesar
//! key. A decoded line is "x<TAB>y<TAB>z<TAB>$GPRMC,..." (G-sensor in 1/1000 g, then an
//! RMC or GGA sentence of any talker). The key comes from a line holding one of five camera
//! numbers, or is searched for: the smallest shift that turns the line into upper-case NMEA
//! characters with a tab. The fix is read only when the G-sensor values parse; RMC times
//! are UTC + local offset, GGA takes its date from the file's write time. The data may not
//! contain tabs, commas or dots (it is encoded).

use crate::gps::format::{Ctx, Format, Item, PT_GPS, PT_GSENSOR, Spec};
use crate::gps::formats::EMPTY_SPEC;
use crate::gps::util::{
    CharSet, DIGITS, SPLIT_KEEP_EMPTY, copy, parse_gpgga, parse_gprmc, pos, split, str_to_float, str_to_int,
};

static SPEC: Spec =
    Spec { title: "Samoon", def_ext: "dat", format_id: "datSM", bad_chars: CharSet::of(b"\t,."), ..EMPTY_SPEC };

/// `allowChas`: the characters of a decoded line.
const ALLOW_CHARS: CharSet = CharSet::of(b"\t$*,-.0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ");
/// `colSep`: must occur in a decoded line.
const COL_SEP: CharSet = CharSet::of(b"\t");

/// Lines holding these numbers set the key (camera models).
const KEYS: [(i32, i32); 5] =
    [(0x2b47_9455, 0x18), (0x2eae_e3d9, 0x22), (0x208a_7a05, 0x28), (0x2b26_acae, 0x37), (0x215c_17d7, 0x39)];

pub struct DatSamoon {
    /// The Caesar key, 0 = unknown.
    key: i32,
}

pub fn create() -> Box<dyn Format> {
    Box::new(DatSamoon { key: 0 })
}

/// `FindCezarKey`: the smallest k in 0..=128 for which every byte minus k is in `allow` and
/// all of `must` occur; 0 when none, or as soon as a byte not ruled out before is <= k.
fn find_cezar_key(s: &[u8], allow: &CharSet, must: &CharSet) -> i32 {
    'keys: for k in 0..=0x80i32 {
        for &c in s {
            if i32::from(c) <= k {
                return 0;
            }
            if !allow.contains(c.wrapping_sub(k as u8)) {
                continue 'keys;
            }
        }
        let mut chs = *must;
        for &c in s {
            chs.remove(c.wrapping_sub(k as u8));
        }
        if chs.is_empty() {
            return k;
        }
    }
    0
}

/// `DecodeCezar`.
fn decode_cezar(s: &[u8], key: i32) -> Vec<u8> {
    s.iter().map(|&c| c.wrapping_sub(key as u8)).collect()
}

/// `GetNextS`: the text up to the first `sep` (or all of it), removed from `s` with the
/// separator.
fn get_next_s(s: &mut Vec<u8>, sep: &[u8]) -> Vec<u8> {
    let po = pos(sep, s).map_or(s.len() + 1, |p| p + 1);
    let token = copy(s, 1, po - 1).to_vec();
    let n = (sep.len() + po - 1).min(s.len());
    s.drain(..n);
    token
}

impl Format for DatSamoon {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn before_load_data(&mut self, _ctx: &mut Ctx) {
        self.key = 0;
    }

    fn load_item2(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item, it2: &mut Item) -> (bool, bool) {
        let mut line = line.to_vec();
        match line.first() {
            // a range error in the original
            None => return (false, false),
            Some(0) => {
                line.remove(0);
            }
            Some(_) => {}
        }
        if line.len() < 11 && line.iter().all(|&c| DIGITS.contains(c)) {
            let n = str_to_int(&line, 0);
            self.key = KEYS.iter().find(|k| k.0 == n).map_or(0, |k| k.1);
            return (false, false);
        }
        if self.key == 0 {
            self.key = find_cezar_key(&line, &ALLOW_CHARS, &COL_SEP);
        }
        if self.key == 0 {
            return (false, false);
        }
        let mut s;
        loop {
            s = decode_cezar(&line, self.key);
            it2.pt = PT_GSENSOR;
            it2.time = 0.0;
            let mut next = || str_to_float(&get_next_s(&mut s, b"\t"), -1e6) / 1000.0;
            it2.x = next();
            it2.y = next();
            it2.z = next();
            if it2.x.abs() < 100.0 && it2.y.abs() < 100.0 && it2.z.abs() < 100.0 {
                break;
            }
            let k = find_cezar_key(&line, &ALLOW_CHARS, &COL_SEP);
            if k == 0 || k == self.key {
                return (false, false);
            }
            self.key = k;
        }
        let mut res1 = false;
        if s.len() > 10 {
            let cols = split(&s, &CharSet::of(b","), SPLIT_KEEP_EMPTY);
            if cols.len() > 10 {
                let mut head = cols[0].clone();
                if head.len() > 3 {
                    head[2] = b'_';
                }
                if head == b"$G_GGA" {
                    parse_gpgga(&cols, it, ctx.info.date_time_finish, ctx.tz);
                } else if head == b"$G_RMC" {
                    parse_gprmc(&cols, it, true, ctx.tz);
                }
                if it.time > 2.0 {
                    it.pt = PT_GPS;
                    res1 = true;
                }
            }
        }
        (res1, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::test_support::*;
    use crate::gps::util::encode_date_time;

    fn nmea(body: &str) -> String {
        let x = body.bytes().fold(0u8, |a, c| a ^ c);
        format!("${body}*{x:02X}")
    }

    fn encode(plain: &str, key: u8) -> Vec<u8> {
        plain.bytes().map(|c| c.wrapping_add(key)).collect()
    }

    const RMC: [&str; 3] = [
        "GNRMC,130626.000,A,6003.8176,N,03019.6213,E,10.0,90.0,160516,,,A",
        "GPRMC,130627.000,A,6003.8176,N,03019.6413,E,10.0,90.0,160516,,,A",
        "GPRMC,130628.000,A,6003.8176,N,03019.6613,E,12.0,91.0,160516,,,A",
    ];

    fn log(magic: Option<&str>, key: u8, bodies: &[&str]) -> Vec<u8> {
        let mut lines: Vec<Vec<u8>> = magic.iter().map(|m| m.as_bytes().to_vec()).collect();
        for (i, body) in bodies.iter().enumerate() {
            let g = format!("{}\t-{}\t{}\t", 120 + i, 340 + i, 980 + i);
            lines.push(encode(&(g + &nmea(body)), key));
        }
        lines.join(&b"\r\n"[..])
    }

    #[test]
    fn key_from_camera_number() {
        let data = log(Some("726111317"), 0x18, &RMC);
        let r = load_log("a.dat", &data).unwrap();
        assert_eq!(r.format, "datSM");
        let t = &r.track;
        assert_eq!(t.gps.len(), 3, "{:?}", t.gps);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let times: Vec<f64> = t.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [0.0, 1.0, 2.0]);
        let p = t.gps[0];
        assert!((p.lat - 60.063_626_666).abs() < 1e-8 && (p.lon - 30.327_021_666).abs() < 1e-8);
        assert!((p.speed_kmh - 10.0 * 1.85325).abs() < 1e-9);
        let g: Vec<(f64, f64, f64)> = t.gsensor.iter().map(|s| (s.x, s.y, s.z)).collect();
        assert_eq!(g, [(0.12, -0.34, 0.98), (0.121, -0.341, 0.981), (0.122, -0.342, 0.982)]);
    }

    #[test]
    fn key_searched_for() {
        let r = load_log("a.dat", &log(None, 0x22, &RMC)).unwrap();
        assert_eq!(r.format, "datSM");
        assert_eq!(r.track.gps.len(), 3);
        // a wrong camera number: the key is searched for again
        let r = load_log("a.dat", &log(Some("545946117"), 0x22, &RMC)).unwrap();
        assert_eq!(r.track.gps.len(), 3);
        assert_eq!(find_cezar_key(&encode("1\t2", 0x37), &ALLOW_CHARS, &COL_SEP), 0x37);
        assert_eq!(find_cezar_key(&encode("1 2", 0x37), &ALLOW_CHARS, &COL_SEP), 0, "blank not allowed");
        assert_eq!(find_cezar_key(b"\x01abc", &ALLOW_CHARS, &COL_SEP), 0);
    }

    #[test]
    fn gga_date_from_file_time() {
        let gga = [
            "GPGGA,130626.000,6003.8176,N,03019.6213,E,1,08,1.0,25.5,M,17.0,M,,0000",
            "GPGGA,130627.000,6003.8176,N,03019.6413,E,1,08,1.0,26.5,M,17.0,M,,0000",
        ];
        let write = encode_date_time(2016, 5, 16, 15, 0, 0, 0).unwrap();
        let r = load_log_at("a.dat", &log(None, 0x18, &gga), (write - 0.1, write)).unwrap();
        let t = &r.track;
        assert_eq!(t.gps.len(), 2);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        assert_eq!((t.gps[0].altitude, t.gps[1].altitude), (25.5, 26.5));
    }

    #[test]
    fn fix_needs_gsensor_values() {
        let mut data = log(None, 0x18, &RMC[..2]);
        data.extend_from_slice(b"\r\n");
        data.extend(encode(&format!("X\tY\tZ\t{}", nmea(RMC[2])), 0x18));
        let r = load_log("a.dat", &data).unwrap();
        assert_eq!(r.track.gps.len(), 2);
        // plain text (tabs, commas): not this format
        let plain = format!("120\t-340\t980\t{}\n", nmea(RMC[0]));
        assert_ne!(load_log("a.dat", plain.as_bytes()).map(|r| r.format).ok().as_deref(), Some("datSM"));
    }

    #[test]
    fn next_field() {
        let mut s = b"12\t34".to_vec();
        assert_eq!(get_next_s(&mut s, b"\t"), b"12");
        assert_eq!(get_next_s(&mut s, b"\t"), b"34");
        assert!(s.is_empty());
        assert!(get_next_s(&mut s, b"\t").is_empty());
    }

    #[test]
    fn fuzz_datsamoon() {
        fuzz("a.dat", &log(Some("726111317"), 0x18, &RMC));
        fuzz("a.dat", &log(None, 0x22, &RMC));
    }
}
