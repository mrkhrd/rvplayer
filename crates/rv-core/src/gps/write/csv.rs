//! `TGpsFormat_CSV.SaveFileV` (three writers: GPS and G-sensor, GPS only, G-sensor only).
//!
//! The column separator is ';' when the locale's decimal separator is ',', else ','. Header
//! `[Type,]Date,Time,ms,X,Y,H,Speed,Bearing,Trip` (with GPS) or `Date,Time,ms,X,Y,Z`
//! (G-sensor only); every row ends with a separator:
//! - GPS: `[gps],` (both only), locale date, time and milliseconds (empty without a time),
//!   longitude and latitude (6 decimals, "?" before them for a fix that is not good), altitude
//!   (always with '.'), speed and bearing (whole numbers; empty when unknown or not good), trip
//!   = whole metres from the source's first fix (empty when not good);
//! - G-sensor: `[gsensor],` (both only), date, time, ms, x, y, z in g with 3 decimals.

use std::fmt::Write as _;

use super::WriteOptions;
use super::info::TrackInfo;
use super::text::{coord, decode, int_n, po, round};
use crate::gps::format::{PT_GPS, PT_GSENSOR};

pub(super) fn write(info: &TrackInfo, o: &WriteOptions, gps: bool, gsensor: bool) -> String {
    let l = &o.locale;
    let sep = if l.decimal_separator == ',' { ';' } else { ',' };
    let both = gps && gsensor;
    let mut s = String::with_capacity(64 + info.points.len() * 80);
    if both {
        let _ = write!(s, "Type{sep}");
    }
    let _ = write!(s, "Date{sep}Time{sep}ms{sep}");
    if gps {
        let _ = write!(s, "X{sep}Y{sep}H{sep}Speed{sep}Bearing{sep}Trip\r\n");
    } else {
        let _ = write!(s, "X{sep}Y{sep}Z\r\n");
    }
    // `tv`: date, time and milliseconds, or three empty columns without a time
    let tv = |s: &mut String, t: f64| {
        if t >= 2.0 {
            let _ = write!(s, "{}{sep}{}{sep}{}", l.date(t), l.time(t), int_n(decode(t).ms.into(), 3));
        } else {
            let _ = write!(s, "{sep}{sep}");
        }
    };
    for p in &info.points {
        let it = &p.item;
        if it.pt == PT_GPS && gps {
            let good = it.warn == 0;
            let mark = if good { "" } else { "?" };
            if both {
                let _ = write!(s, "[gps]{sep}");
            }
            tv(&mut s, it.time);
            let _ = write!(
                s,
                "{sep}{mark}{}{sep}{mark}{}{sep}{}{sep}",
                l.num(&coord(it.x)),
                l.num(&coord(it.y)),
                po(it.z, 1, false)
            );
            if good && it.speed >= 0.0 {
                let _ = write!(s, "{}", round(it.speed));
            }
            s.push(sep);
            if good && it.angle >= 0.0 {
                let _ = write!(s, "{}", round(it.angle));
            }
            s.push(sep);
            if good {
                let _ = write!(s, "{}", round(p.track_pos));
            }
            let _ = write!(s, "{sep}\r\n");
        } else if it.pt == PT_GSENSOR && gsensor {
            if both {
                let _ = write!(s, "[gsensor]{sep}");
            }
            tv(&mut s, it.time);
            let g = |v: f64| l.num(&po(v, 3, true));
            let _ = write!(s, "{sep}{}{sep}{}{sep}{}{sep}\r\n", g(it.x), g(it.y), g(it.z));
        }
    }
    s
}
