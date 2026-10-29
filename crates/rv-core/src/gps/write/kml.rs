//! `TGpsFormat_KML.SaveFileV`: a Google Earth document with
//! - a "Track" placemark: the good fixes as line strings, a new one after each gap
//!   (`IsGPSBreak`);
//! - a hidden "Points" folder: a placemark per good fix whose description shows its number,
//!   the track name, the time since the previous point (when over 1.5 s) and since the start,
//!   date and time, position, altitude (when not 0), speed, bearing and track length;
//! - an "Animation" `gx:Track` of the fixes with their times;
//! - a "Tour" that flies a "Car" placemark along the fixes: fixes closer than 10 s and 500 m
//!   to the previous tour point are skipped, each step lasts the time the car needs at its
//!   speed (1/10 of real time, at most 5 s; steps under 0.2 s are skipped), and turns over
//!   45° are split into 45° steps.
//!
//! Deviations: `<begin>`, `<end>` and `<when>` are UTC (local − tz) — the original wrote the
//! local time with a "Z", so Google Earth showed the time shifted by the time-zone offset; the
//! name and description are XML-escaped (see `xml_escape`).

use std::fmt::Write as _;

use super::info::TrackInfo;
use super::text::{MS, coord, distance, iso_z, is_break, po, round};
use super::{WriteOptions, good_fixes, xml_escape};
use crate::gps::util::SEC;

const STYLES: &str = concat!(
    "<Style id=\"yellowLine\"><LineStyle><color>7f00ffff</color><width>3</width></LineStyle></Style>\r\n",
    "<Style id=\"multiTrack_n\"><IconStyle><Icon><href>http://earth.google.com/images/kml-icons/track-directional/track-0.png</href></Icon></IconStyle><LineStyle><color>99ffac59</color><width>6</width></LineStyle></Style>\r\n",
    "<Style id=\"multiTrack_h\"><IconStyle><scale>1.2</scale><Icon><href>http://earth.google.com/images/kml-icons/track-directional/track-0.png</href></Icon></IconStyle><LineStyle><color>99ffac59</color><width>7.2</width></LineStyle></Style>\r\n",
    "<StyleMap id=\"multiTrack\"><Pair><key>normal</key><styleUrl>#multiTrack_n</styleUrl></Pair><Pair><key>highlight</key><styleUrl>#multiTrack_h</styleUrl></Pair></StyleMap>\r\n",
);

/// `sbAddTimeMS`: `TimeToStr` of a time span plus its tenths of a second when not 0. The tenths
/// come from the span rounded to the millisecond, like the shown time; the original truncated
/// the raw value, so float noise showed an exact 2 s as "00:00:02.9".
fn add_time_ms(s: &mut String, t: f64, o: &WriteOptions) {
    s.push_str(&o.locale.time(t.fract()));
    let tenths = round(t / MS).rem_euclid(1000) / 100;
    if tenths != 0 {
        let _ = write!(s, ".{tenths}");
    }
}

pub(super) fn write(info: &TrackInfo, o: &WriteOptions) -> String {
    let pts = &info.points;
    let w = &o.words;
    let utc = |t: f64| iso_z(t - o.tz);
    let mut s = String::with_capacity(4096 + pts.len() * 1200);

    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<kml creator=\"");
    s.push_str(&xml_escape(&o.creator));
    s.push_str("\" xmlns=\"http://www.opengis.net/kml/2.2\" xmlns:gx=\"http://www.google.com/kml/ext/2.2\">\r\n");
    s.push_str("<Document id=\"RVTrackDoc\">\r\n");
    let _ = write!(s, "  <name>{}</name>\r\n", xml_escape(&info.name));
    let _ = write!(s, "  <description>{}</description>\r\n", xml_escape(&info.desc));
    s.push_str("  <open>1</open>\r\n  <LookAt>\r\n");
    let _ = write!(s, "    <longitude>{}</longitude>\r\n", coord(info.center.0));
    let _ = write!(s, "    <latitude>{}</latitude>\r\n", coord(info.center.1));
    let _ = write!(s, "    <range>{}</range>\r\n", round(info.width_m.max(info.height_m) * 1.2).max(200));
    s.push_str("    <altitude>0</altitude>\r\n    <tilt>0</tilt>\r\n    <heading>0</heading>\r\n    <gx:TimeSpan>\r\n");
    let _ = write!(s, "      <begin>{}</begin>\r\n      <end>{}</end>\r\n", utc(info.start), utc(info.finish));
    s.push_str("    </gx:TimeSpan>\r\n  </LookAt>\r\n");
    s.push_str(STYLES);

    // the track line
    s.push_str("<Placemark><name>Track</name><styleUrl>#yellowLine</styleUrl>");
    s.push_str("<MultiGeometry><LineString><coordinates>\r\n");
    let mut prev: Option<usize> = None;
    for (i, p) in good_fixes(info) {
        // like the original, a fix at index 0 is never compared with the next one
        if let Some(k) = prev.filter(|&k| k > 0)
            && is_break(&p.item, &pts[k].item)
        {
            s.push_str("</coordinates></LineString><LineString><coordinates>\r\n");
        }
        prev = Some(i);
        let it = &p.item;
        let _ = write!(s, "{},{},{}\r\n", coord(it.x), coord(it.y), po(it.z, 1, false));
    }
    s.push_str("</coordinates></LineString></MultiGeometry></Placemark>\r\n");

    // the points
    s.push_str("<Folder><name>Points</name><visibility>0</visibility>");
    s.push_str("<Style><ListStyle><listItemType>checkHideChildren</listItemType></ListStyle></Style>\r\n");
    let mut prev = 0usize;
    for (n, (i, p)) in good_fixes(info).enumerate() {
        let it = &p.item;
        s.push_str("<Placemark><Point><coordinates>");
        let _ = write!(s, "{},{},{}", coord(it.x), coord(it.y), po(it.z, 1, false));
        s.push_str("</coordinates></Point><description><![CDATA[<nobr><b>( ");
        let _ = write!(s, "{} ) {}</b><br><br><font color=\"#666666\">( ", n + 1, xml_escape(&info.name));
        let since_prev = it.time - pts[prev].item.time;
        if since_prev > 1.5 * SEC {
            s.push('+');
            add_time_ms(&mut s, since_prev, o);
            s.push_str(" = ");
        }
        add_time_ms(&mut s, it.time - info.start, o);
        s.push_str(" ) - ");
        s.push_str(&o.locale.date_time(it.time));
        s.push_str("</font></nobr><br><br>");
        let _ = write!(
            s,
            "{}{}, {}{}<br>",
            if it.y < 0.0 { "S" } else { "N" },
            coord(it.y.abs()),
            if it.x < 0.0 { "W" } else { "E" },
            coord(it.x.abs())
        );
        if it.z != 0.0 {
            let _ = write!(s, "<b>{}</b>: {} {}<br>", w.altitude, po(it.z, 1, false), w.meters);
        }
        let _ = write!(s, "<br><b>{}</b>: {} {}<br>", w.speed, po(it.speed, 1, false), w.km_per_h);
        let _ = write!(s, "<b>{}</b>: {}\u{b0}<br>", w.bearing, round(it.angle));
        let _ = write!(s, "<b>{}</b>: {} {}<br>", w.track_length, round(p.track_pos), w.meters);
        s.push_str("]]></description></Placemark>\r\n");
        prev = i;
    }
    s.push_str("</Folder>\r\n");

    // the animation
    s.push_str("<Placemark><name>Animation</name><styleUrl>#multiTrack</styleUrl><gx:Track>\r\n");
    for (_, p) in good_fixes(info) {
        let it = &p.item;
        let _ = write!(
            s,
            "<when>{}</when><gx:coord>{} {} {}</gx:coord>\r\n",
            utc(it.time),
            coord(it.x),
            coord(it.y),
            po(it.z, 1, false)
        );
    }
    s.push_str("</gx:Track></Placemark>\r\n");

    tour(&mut s, info);
    s.push_str("</Document>\r\n</kml>\r\n");
    s
}

/// The "Tour" playlist.
fn tour(s: &mut String, info: &TrackInfo) {
    let pts = &info.points;
    s.push_str("<gx:Tour><name>Tour</name><gx:Playlist>\r\n");
    let mut prev = 0usize;
    let mut n = 0usize;
    for (i, p) in good_fixes(info) {
        let it = &p.item;
        let pv = &pts[prev].item;
        let mut dt = 1.0;
        if n > 0 {
            let near = it.time - pv.time < 10.0 * SEC;
            if near && distance(it.x, it.y, pv.x, pv.y) < 500.0 {
                continue;
            }
            dt = if it.speed > 1.0 {
                distance(it.x, it.y, pv.x, pv.y) / (it.speed + pv.speed) * 2.0 * 3.6 / 10.0
            } else {
                (it.time - pv.time) / (10.0 * SEC)
            };
        }
        if dt > 5.0 {
            dt = 5.0;
        }
        if dt < 0.2 {
            continue;
        }
        let mut a2 = round(it.angle);
        let mut a1 = if n == 0 { a2 } else { round(pv.angle) };
        if (a1 - a2).abs() > 180 {
            if a1 < a2 {
                a1 += 360;
            } else {
                a2 += 360;
            }
        }
        let turn = (a1 - a2).abs();
        let q = (turn / 45).max(1);
        dt /= q as f64;
        for z in 1..=q {
            let (cx, cy, ca, cs) = if q == 1 {
                (it.x, it.y, it.angle, it.speed)
            } else {
                let (zf, rf, qf) = (z as f64, (q - z) as f64, q as f64);
                (
                    (zf * it.x + rf * pv.x) / qf,
                    (zf * it.y + rf * pv.y) / qf,
                    (round((a2 * z + (q - z) * a1) as f64 / qf) + 360).rem_euclid(360) as f64,
                    (zf * it.speed + rf * pv.speed) / qf,
                )
            };
            let alt = po(it.z, 1, false);
            let _ = write!(
                s,
                "<gx:AnimatedUpdate><gx:duration>{}</gx:duration><Update><targetHref></targetHref>",
                po(dt, 2, true)
            );
            s.push_str(if n == 0 {
                "<Create><Document targetId=\"RVTrackDoc\"><Placemark id=\"Car\"><styleUrl>#multiTrack_n</styleUrl>"
            } else {
                "<Change><Placemark targetId=\"Car\">"
            });
            let _ = write!(s, "<Style><IconStyle><heading>{}</heading></IconStyle></Style>", round(ca));
            let _ = write!(
                s,
                "<Point><coordinates>{},{},{}</coordinates></Point></Placemark>",
                coord(cx),
                coord(cy),
                alt
            );
            s.push_str(if n == 0 { "</Document></Create>" } else { "</Change>" });
            s.push_str("</Update></gx:AnimatedUpdate>\r\n<gx:FlyTo>");
            if n == 0 {
                s.push_str("<gx:duration>2</gx:duration><gx:flyToMode>bounce</gx:flyToMode>");
            } else {
                let _ = write!(
                    s,
                    "<gx:duration>{}</gx:duration><gx:flyToMode>smooth</gx:flyToMode>",
                    po(dt, 2, true)
                );
            }
            let _ = write!(
                s,
                "<LookAt><longitude>{}</longitude><latitude>{}</latitude> <altitude>{}</altitude>",
                coord(cx),
                coord(cy),
                alt
            );
            let _ = write!(s, "<altitudeMode>relativeToGround</altitudeMode><heading>{}</heading>", round(ca));
            // a turning car is looked at as if slower
            let cs = cs * (1.0 - turn as f64 / 90.0).max(0.0);
            let _ = write!(s, "<tilt>{}</tilt>", round(cs).min(45) + 20);
            let _ = write!(s, "<range>{}</range>", 150 - round(cs).min(50));
            s.push_str("</LookAt></gx:FlyTo>\r\n");
        }
        prev = i;
        n += 1;
    }
    s.push_str("</gx:Playlist></gx:Tour>\r\n");
}
