//! `UnitGps_GPX`: GPS eXchange Format. Like the original this is a text scan, not an XML
//! parse, so truncated files (camera switched off mid-write) still load: every complete
//! `<trkpt ...>...</trkpt>` gives a fix from its `lat`, `lon`, `ele`, `time` (UTC), `course`
//! and `speed` (m/s) values. Points without both coordinates or a time are skipped.

use crate::gps::format::{Ctx, DataTest, Format, Item, PT_GPS, Spec, default_test_load_data};
use crate::gps::util::{CharSet, format_tz_to_date_time, pos, str_to_float};

static SPEC: Spec = Spec {
    title: "GPS eXchange Format",
    def_ext: "gpx",
    must_be_chars_in_data: CharSet::of(b"\"/<=>"),
    must_be_strings_in_data: &["<?xml", "<gpx"],
    ..super::EMPTY_SPEC
};

pub struct Gpx;

pub fn create() -> Box<dyn Format> {
    Box::new(Gpx)
}

/// `GetValue`: the text after `name` in `tag`, skipping `"`, `<`, `=`, `>` and ending at the
/// next of them.
pub(crate) fn get_value<'a>(name: &[u8], tag: &'a [u8]) -> &'a [u8] {
    let Some(p) = pos(name, tag) else { return &[] };
    let delim = |c: u8| c == b'"' || (b'<'..=b'>').contains(&c);
    let mut i = p + name.len();
    while i < tag.len() && delim(tag[i]) {
        i += 1;
    }
    let start = i;
    while i < tag.len() && !delim(tag[i]) {
        i += 1;
    }
    &tag[start..i]
}

/// `LoadV`.
fn load_v(data: &[u8], tz: f64, points: &mut Vec<Item>) {
    const OPEN: &[u8] = b"<trkpt ";
    const CLOSE: &[u8] = b"</trkpt>";
    let mut i = 0usize;
    let mut start: Option<usize> = None;
    while i + 17 < data.len() {
        match start {
            None if data[i..].starts_with(OPEN) => start = Some(i),
            Some(s) if data[i..].starts_with(CLOSE) => {
                let tag = &data[s..i];
                start = None;
                let mut it = Item { pt: PT_GPS, ..Default::default() };
                it.y = str_to_float(get_value(b"lat", tag), 0.0);
                it.x = str_to_float(get_value(b"lon", tag), 0.0);
                it.z = str_to_float(get_value(b"ele", tag), 0.0);
                it.time = format_tz_to_date_time(get_value(b"time", tag));
                if it.x != 0.0 && it.y != 0.0 && it.time > 0.0 {
                    it.time += tz;
                    it.angle = str_to_float(get_value(b"course", tag), -1.0);
                    it.speed = str_to_float(get_value(b"speed", tag), -1.0);
                    if it.speed > 0.0 {
                        it.speed *= 3.6;
                    }
                    points.push(it);
                }
            }
            _ => {}
        }
        i += 1;
    }
}

impl Format for Gpx {
    fn spec(&self) -> &Spec {
        &SPEC
    }

    fn test_load_data(&mut self, ctx: &mut Ctx, data: &[u8]) -> DataTest {
        if !default_test_load_data(&SPEC, data) {
            return DataTest::No;
        }
        load_v(data, ctx.tz, &mut ctx.points);
        DataTest::Loaded
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, _line: &[u8]) -> bool {
        false
    }

    fn is_allow_frac_seconds(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::gps::test_support::*;

    const GPX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="cam"><trk><trkseg>
<trkpt lat="60.063627" lon="30.327022"><ele>12.5</ele><time>2016-05-16T13:06:26Z</time><course>112.0</course><speed>10</speed></trkpt>
<trkpt lat="60.063527" lon="30.327522"><ele>13</ele><time>2016-05-16T13:06:27Z</time></trkpt>
<trkpt lat="0" lon="30.3"><time>2016-05-16T13:06:28Z</time></trkpt>
<trkpt lat="60.063427" lon="30.328022"><time>2016-05-16T13:06:29Z</time></trkpt>
<trkpt lat="60.063327" lon="30.328522"><time>2016-05-16T13:06:30Z</time>
"#;

    #[test]
    fn track_points_with_utc_times() {
        let r = load_log("trip.gpx", GPX.as_bytes()).unwrap();
        assert_eq!(r.format, "gpx");
        let t = &r.track;
        // the zero-latitude point and the unterminated last one are skipped
        assert_eq!(t.gps.len(), 3);
        assert_eq!(t.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2016-05-16 13:06:26");
        let p = t.gps[0];
        assert_eq!((p.lat, p.lon, p.altitude, p.course), (60.063627, 30.327022, 12.5, 112.0));
        assert!((p.speed_kmh - 36.0).abs() < 1e-9, "m/s to km/h");
        assert_eq!(t.gps[2].time, 3.0);
        // a missing speed is computed from the positions
        assert!(t.gps[1].speed_kmh > 0.0);
    }

    #[test]
    fn local_time_zone_is_added() {
        let r = crate::gps::load_log_bytes(
            "a.gpx",
            GPX.as_bytes(),
            &crate::gps::LoadOptions { tz: 3.0 / 24.0, file_times: (0.0, 0.0) },
            &|_| None,
        )
        .unwrap();
        assert_eq!(r.track.start.unwrap().format("%H:%M:%S").to_string(), "16:06:26");
    }

    #[test]
    fn needs_xml_header_and_gpx_tag() {
        let no_header = GPX.replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>", "");
        assert!(load_log("a.gpx", no_header.as_bytes()).is_err());
        fuzz("a.gpx", GPX.as_bytes());
    }
}
