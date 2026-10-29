//! `TGpsFormat_GPX.SaveFileV`: GPX 1.0 with the bounds of the good fixes and one track segment
//! of the good fixes (one per second, see `one_fix_per_second`): position, UTC time, course,
//! speed in m/s and, when not 0, elevation — in that order, as the original wrote them.
//!
//! Deviation: the name and description are XML-escaped (see `xml_escape`).

use std::fmt::Write as _;

use super::info::TrackInfo;
use super::text::{coord, iso_z, po, round};
use super::{WriteOptions, good_fixes, xml_escape};

pub(super) fn write(info: &TrackInfo, o: &WriteOptions) -> String {
    let mut s = String::with_capacity(1024 + info.points.len() * 160);
    let start = iso_z(info.start - o.tz);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\r\n<gpx\r\n version=\"1.0\"\r\n creator=\"");
    s.push_str(&xml_escape(&o.creator));
    s.push_str("\"\r\n xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\r\n");
    s.push_str(" xmlns=\"http://www.topografix.com/GPX/1/0\"\r\n");
    s.push_str(" xsi:schemaLocation=\"http://www.topografix.com/GPX/1/0 http://www.topografix.com/GPX/1/0/gpx.xsd\">\r\n");
    let _ = write!(s, "<time>{start}</time>\r\n");
    let _ = write!(
        s,
        "<bounds minlat=\"{}\" minlon=\"{}\" maxlat=\"{}\" maxlon=\"{}\"/>\r\n",
        coord(info.br.1),
        coord(info.tl.0),
        coord(info.tl.1),
        coord(info.br.0)
    );
    let _ = write!(
        s,
        "<trk>\r\n  <name>{}</name>\r\n  <desc>{}</desc>\r\n  <time>{start}</time>\r\n  <trkseg>\r\n",
        xml_escape(&info.name),
        xml_escape(&info.desc)
    );
    for (_, p) in good_fixes(info) {
        let it = &p.item;
        let _ = write!(
            s,
            "\t<trkpt lat=\"{}\" lon=\"{}\"><time>{}</time><course>{}</course><speed>{}</speed>",
            coord(it.y),
            coord(it.x),
            iso_z(it.time - o.tz),
            round(it.angle),
            po(it.speed / 3.6, 2, false)
        );
        if it.z != 0.0 {
            let _ = write!(s, "<ele>{}</ele>", po(it.z, 1, false));
        }
        s.push_str("</trkpt>\r\n");
    }
    s.push_str("  </trkseg>\r\n</trk>\r\n</gpx>\r\n");
    s
}
