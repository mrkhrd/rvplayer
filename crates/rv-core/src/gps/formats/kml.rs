//! `UnitGps_KML`: Google Earth KML. Only the first `<coordinates>` block is read
//! ("lon,lat,alt" tuples separated by blanks, tabs, line breaks or ';'). KML has no times:
//! the points are spread evenly over the file's creation..write window, ending at the write
//! time, at 0.2–5 s per point (else 1 s).

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, Spec, default_test_load_data};
use crate::gps::util::{CharSet, SEC, SPLIT_KEEP_EMPTY, pos, split, str_to_float};

static SPEC: Spec = Spec {
    title: "Google Earth",
    def_ext: "kml",
    must_be_chars_in_data: CharSet::of(b"/<>"),
    must_be_strings_in_data: &["<kml", "<coordinates>"],
    ..super::EMPTY_SPEC
};

pub struct Kml;

pub fn create() -> Box<dyn Format> {
    Box::new(Kml)
}

/// `LoadV`.
fn load_v(data: &[u8], (created, written): (f64, f64), points: &mut Vec<Item>) {
    if pos(b"<kml", data).is_none() {
        return;
    }
    let (Some(open), Some(end)) = (pos(b"<coordinates>", data), pos(b"</coordinates>", data)) else { return };
    if open >= end {
        return;
    }
    let sep = |c: u8| matches!(c, b'\t' | b'\n' | b'\r' | b' ' | b';');
    let comma = CharSet::of(b",");
    let first = points.len();
    let mut i = open + 13;
    while i < end {
        while i <= end && sep(data[i]) {
            i += 1;
        }
        let start = i;
        while i <= end && !sep(data[i]) {
            i += 1;
        }
        let cols = split(&data[start..i], &comma, SPLIT_KEEP_EMPTY);
        if cols.len() == 3 {
            let mut it = Item { pt: PT_GPS, ..Default::default() };
            it.x = str_to_float(&cols[0], 0.0);
            it.y = str_to_float(&cols[1], 0.0);
            it.z = str_to_float(&cols[2], 0.0);
            it.speed = -1.0;
            it.angle = -1.0;
            points.push(it);
        }
    }
    let n = points.len() - first;
    if n == 0 {
        return;
    }
    let mut dt = if n < 2 || written <= created { 0.0 } else { (written - created) / (n - 1) as f64 };
    if !(2.314_814_814_814_814_8e-6..=5.787_037_037_037_037e-5).contains(&dt) {
        dt = SEC;
    }
    for (k, p) in points[first..].iter_mut().enumerate() {
        p.time = written - (n - 1 - k) as f64 * dt;
    }
}

impl Format for Kml {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_data(&mut self, ctx: &mut Ctx, data: &[u8]) -> DataTest {
        if !default_test_load_data(&SPEC, data) {
            return DataTest::No;
        }
        load_v(data, (ctx.info.date_time_start, ctx.info.date_time_finish), &mut ctx.points);
        DataTest::Loaded
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, _line: &[u8]) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;
    use crate::gps::util::SEC;

    const KML: &str = "<?xml version=\"1.0\"?><kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document><Placemark>\
        <LineString><coordinates>\n 30.327022,60.063627,12\t30.327522,60.063527,13;30.328022,60.063427,14\r\n\
        bad,1 30.328522,60.063327,15</coordinates></LineString></Placemark></Document></kml>";

    #[test]
    fn coordinates_timed_by_file_window() {
        let t0 = 42_506.5;
        let r = load_log_at("trip.kml", KML.as_bytes(), (t0, t0 + 6.0 * SEC)).unwrap();
        assert_eq!(r.format, "kml");
        let t = &r.track;
        assert_eq!(t.gps.len(), 4);
        let p = t.gps[0];
        assert_eq!((p.lon, p.lat, p.altitude), (30.327022, 60.063627, 12.0));
        // 4 points over 6 s: 2 s apart, ending at the write time
        assert_eq!(t.gps[3].time, 6.0);
        assert_eq!(t.start.unwrap().format("%H:%M:%S").to_string(), "12:00:00");
        // the last tuple runs into "</coordinates>": its altitude is not a number
        assert_eq!(t.gps[3].altitude, t.gps[2].altitude);
        // a window that gives more than 5 s per point falls back to 1 s
        let r = load_log_at("trip.kml", KML.as_bytes(), (t0, t0 + 600.0 * SEC)).unwrap();
        assert_eq!(r.track.gps[3].time, 3.0);
        fuzz("a.kml", KML.as_bytes());
    }
}
