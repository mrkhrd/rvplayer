//! `UnitGps_Panorama`: "pan" binary logs of Panorama cameras: "GPS INFORMATION", one byte, an
//! i32 record count, then records of an i32 video frame number (30 frames per second) and 128
//! bytes of text. The text is read like an SRT cue (see [`super::srt`]: "N55.123 E37.123
//! 2016/05/16 12:30:45", "X:" / "Y:" / "Z:" G-sensor values); a fix needs a date and a time of
//! day (local clock). The records' video times re-time the fixes (`ShiftTimeToGPS`).

use crate::gps::format::{Ctx, Format, Item, RawItem, Spec};
use crate::gps::formats::{EMPTY_SPEC, srt};
use crate::gps::util::{SEC, copy, le_i32, trim};

static SPEC: Spec = Spec { title: "pan", data_prefix: b"GPS INFORMATION", line_prefix: b"MyPaNoRa", ..EMPTY_SPEC };

/// Bytes of text per record.
const TEXT_LEN: usize = 0x80;
/// Frames per second of the record numbers.
const FPS: f64 = 30.0;

pub struct Panorama;

pub fn create() -> Box<dyn Format> {
    Box::new(Panorama)
}

impl Format for Panorama {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    /// Records as lines "MyPaNoRa" + trimmed text, timed by their frame numbers. A count of a
    /// million or more means "not this format"; the count does not limit the records read.
    fn split_raw_data(&mut self, data: &[u8]) -> Option<(Vec<RawItem>, f64)> {
        let prefix = SPEC.data_prefix;
        if copy(data, 1, prefix.len()) != prefix {
            return None;
        }
        // The original raises (< 17 bytes) or reads past the end (< 20 bytes) here.
        let n = le_i32(data, prefix.len() + 1)?;
        if n >= 1_000_000 {
            return None;
        }
        let mut items = Vec::new();
        let mut po = prefix.len() + 6; // 1-based
        while data.len() >= po + TEXT_LEN {
            let frame = le_i32(data, po - 1)?;
            po += 4;
            let mut line = SPEC.line_prefix.to_vec();
            line.extend_from_slice(trim(copy(data, po, TEXT_LEN)));
            items.push(RawItem::timed(f64::from(frame) / FPS * SEC, line));
            po += TEXT_LEN;
        }
        (!items.is_empty()).then_some((items, 0.0))
    }

    fn load_item2(&mut self, _ctx: &mut Ctx, line: &[u8], it1: &mut Item, it2: &mut Item) -> (bool, bool) {
        let text = copy(line, SPEC.line_prefix.len() + 1, line.len());
        srt::parse(text, it1, 0.0, it2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::format::DataTest;
    use crate::gps::test_support::*;

    fn pan(count: i32, records: &[(i32, &str)]) -> Vec<u8> {
        let mut d = b"GPS INFORMATION\0".to_vec();
        d.extend_from_slice(&count.to_le_bytes());
        for (frame, text) in records {
            d.extend_from_slice(&frame.to_le_bytes());
            let mut t = text.as_bytes().to_vec();
            t.resize(TEXT_LEN, 0);
            d.extend_from_slice(&t);
        }
        d
    }

    const RECORDS: [(i32, &str); 4] = [
        (30, "N55.750100 E37.610200 2016/05/16 12:30:46 X:0.1 Y:0.2 Z:0.9"),
        (45, "N55.750150 E37.610300"),
        (60, "  N55.750200 E37.610400 2016/05/16 12:30:47 X:0.2 Y:0.3 Z:1.0"),
        (90, "N55.750300 E37.610600 16.05.2016 12:30:48"),
    ];

    #[test]
    fn records_with_frame_numbers() {
        let r = load_log("a.pan", &pan(4, &RECORDS)).unwrap();
        assert_eq!(r.format, "pan");
        let t = &r.track;
        // the record without date and time is not a fix
        assert_eq!(t.gps.len(), 3, "{:?}", t.gps);
        // track times are video times: the start is video time 0, one second before the first fix
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 12:30:45");
        let times: Vec<f64> = t.gps.iter().map(|p| p.time).collect();
        assert_eq!(times, [1.0, 2.0, 3.0]);
        assert_eq!((t.gps[0].lat, t.gps[0].lon, t.gps[0].warning), (55.7501, 37.6102, 0));
        assert_eq!((t.gps[2].lat, t.gps[2].lon), (55.7503, 37.6106));
        let g: Vec<(f64, f64, f64, f64)> = t.gsensor.iter().map(|s| (s.time, s.x, s.y, s.z)).collect();
        assert_eq!(g, [(1.0, 0.1, 0.2, 0.9), (2.0, 0.2, 0.3, 1.0)]);
    }

    #[test]
    fn split_and_detection() {
        let data = pan(1, &RECORDS[..2]);
        let (items, frame0) = Panorama.split_raw_data(&data).unwrap();
        assert_eq!((items.len(), frame0), (2, 0.0), "the count does not limit the records");
        assert_eq!(items[0].line, b"MyPaNoRaN55.750100 E37.610200 2016/05/16 12:30:46 X:0.1 Y:0.2 Z:0.9");
        assert_eq!(items[1].frame_time, 1.5 * SEC);
        // a short last record is read up to the end of the data
        let (items, _) = Panorama.split_raw_data(&data[..data.len() - 3]).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(Panorama.split_raw_data(&data[..data.len() - 4]).unwrap().0.len(), 1);
        assert!(Panorama.split_raw_data(&pan(1_000_000, &RECORDS)).is_none());
        assert!(Panorama.split_raw_data(&pan(4, &[])).is_none());
        assert!(Panorama.split_raw_data(b"GPS INFORMATION").is_none());
        let mut other = pan(4, &RECORDS);
        other[0] = b'g';
        assert!(Panorama.split_raw_data(&other).is_none());
        let mut ctx = Ctx::default();
        assert_eq!(Panorama.test_load_data(&mut ctx, &other), DataTest::No);
        assert_eq!(Panorama.test_load_data(&mut ctx, &data), DataTest::Maybe);
        assert!(!Panorama.test_load_item(&mut ctx, b"N55.750100 E37.610200 2016/05/16 12:30:46"));
    }

    #[test]
    fn fuzz_panorama() {
        fuzz("a.pan", &pan(4, &RECORDS));
    }
}
