//! `UnitGps_GPRMC`: any text with an RMC sentence somewhere in the line (without the leading
//! '$', e.g. after a time stamp). The sentence is cut at most 100 characters from the line
//! start and parsed with its checksum.

use crate::gps::format::{Ctx, Format, Item, PT_GPS, Spec, default_test_load_item};
use crate::gps::util::{CharSet, SPLIT_KEEP_EMPTY, copy, parse_gprmc, pos, split, trim};

static SPEC: Spec = Spec {
    title: "GPRMC",
    def_ext: "",
    format_id: "",
    must_be_chars_in_line: CharSet::EMPTY,
    must_be_chars_in_data: CharSet::EMPTY,
    bad_chars: CharSet::EMPTY,
    must_be_strings_in_data: &[],
    data_prefix: b"",
    line_prefix: b"",
};

pub struct Gprmc;

pub fn create() -> Box<dyn Format> {
    Box::new(Gprmc)
}

/// `FindRMC`: 1-based position of the 'G' of "G?RMC,".
fn find_rmc(line: &[u8]) -> Option<usize> {
    let p = pos(b"RMC,", line)? + 1;
    let p = p.checked_sub(2).filter(|&p| p > 0)?;
    (line[p - 1] == b'G').then_some(p)
}

impl Format for Gprmc {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, line: &[u8]) -> bool {
        default_test_load_item(&SPEC, line) && find_rmc(line).is_some()
    }

    fn load_item1(&mut self, ctx: &mut Ctx, line: &[u8], it: &mut Item) -> bool {
        let Some(p) = find_rmc(line) else { return false };
        let mut s = b"$".to_vec();
        s.extend_from_slice(trim(copy(line, p, line.len().min(100))));
        let cols = split(&s, &CharSet::of(b","), SPLIT_KEEP_EMPTY);
        it.pt = PT_GPS;
        parse_gprmc(&cols, it, true, ctx.tz);
        it.time > 2.0
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    #[test]
    fn rmc_inside_other_text() {
        let log = fix_checksums(
            "2016/05/16 13:06:26 GPRMC,130626.000,A,6003.8176,N,03019.6213,E,10.0,90.0,160516,,,A*00\n\
             2016/05/16 13:06:27 GPRMC,130627.000,A,6003.8176,N,03019.6413,E,10.0,90.0,160516,,,A*00\n",
            &[0, 1],
        );
        let r = load_log("x.txt", log.as_bytes()).unwrap();
        assert_eq!(r.format, "GPRMC");
        assert_eq!(r.track.gps.len(), 2);
        assert_eq!(r.track.gps[1].time, 1.0);
    }
}
