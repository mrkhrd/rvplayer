//! `TGpsFormat_PLT.SaveFileV`: OziExplorer track. The header names the track and gives the
//! number of points; each good fix is a line
//! `lat,lon,break,altitude ft (-777 = 0 m),TDateTime UTC (7 decimals),date,time,speed km/h,`
//! with the date and time in UTC as the locale writes them. "break" is 1 for the first fix and
//! after a gap (`IsGPSBreak`).
//!
//! Deviation: a fix is compared with the previous GPS record. The original compared it with the
//! previous record of any kind, so with G-sensor samples between the fixes nearly every fix
//! started a new segment.

use std::fmt::Write as _;

use super::info::TrackInfo;
use super::text::{coord, is_break, po, round};
use super::{WriteOptions, good_fixes};
use crate::gps::format::PT_GPS;

pub(super) fn write(info: &TrackInfo, o: &WriteOptions) -> String {
    let pts = &info.points;
    let mut s = String::with_capacity(256 + pts.len() * 80);
    let count = good_fixes(info).count();
    s.push_str("OziExplorer Track Point File Version 2.1\r\nWGS 84\r\nAltitude is in Feet\r\nReserved 3\r\n");
    let _ = write!(s, "0,2,8323327,{},0,0,2,8421376\r\n{count}\r\n", info.name);
    for (i, p) in good_fixes(info) {
        let it = &p.item;
        let prev_gps = pts[..i].iter().rposition(|q| q.item.pt == PT_GPS);
        let brk = prev_gps.is_none_or(|k| is_break(it, &pts[k].item));
        let alt = if it.z == 0.0 { "-777".to_string() } else { round(it.z / 0.3048).to_string() };
        let t = it.time - o.tz;
        let _ = write!(
            s,
            "{},{},{},{alt},{},{},{},{},\r\n",
            coord(it.y),
            coord(it.x),
            u8::from(brk),
            po(t, 7, true),
            o.locale.date(t),
            o.locale.time(t),
            round(it.speed)
        );
    }
    s
}
