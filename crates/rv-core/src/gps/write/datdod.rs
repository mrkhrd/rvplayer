//! `TGpsFormat_DatDOD.SaveFileV`: DOD GS600 / Carcam Q3 ".dat", tab-separated, in record order
//! (one fix per second, see `one_fix_per_second`):
//! - `[G]\tYYYY-MM-DD hh:mm:ss\tN|S<lat>\tE|W<lon>\t<speed km/h>\t<course>` for a good fix
//!   (local time, degrees with 6 decimals);
//! - `[S]\t<x>\t<y>\t<z + 1>` for a G-sensor sample, in 1/1000 g.
//!
//! As in the original, 1 g is added to z and the reader does not take it off again (a round
//! trip raises z by 1 g); the time gets ".mmm" only for a fix at 0°, 0°.

use std::fmt::Write as _;

use super::info::TrackInfo;
use super::text::{decode, fixed, int2, int_n, round};
use crate::gps::format::{PT_GPS, PT_GSENSOR};

pub(super) fn write(info: &TrackInfo) -> String {
    let mut s = String::with_capacity(info.points.len() * 64);
    for p in &info.points {
        let it = &p.item;
        if it.pt == PT_GSENSOR {
            let _ = write!(
                s,
                "[S]\t{}\t{}\t{}\r\n",
                round(it.x * 1000.0),
                round(it.y * 1000.0),
                round((it.z + 1.0) * 1000.0)
            );
        } else if it.pt == PT_GPS && it.warn == 0 {
            let d = decode(it.time);
            let _ = write!(
                s,
                "[G]\t{}-{}-{} {}:{}:{}",
                int_n(d.y.into(), 4),
                int2(d.mo.into()),
                int2(d.d.into()),
                int2(d.h.into()),
                int2(d.mi.into()),
                int2(d.s.into())
            );
            if it.x == 0.0 && it.y == 0.0 {
                let _ = write!(s, ".{}", int_n(d.ms.into(), 3));
            }
            let _ = write!(
                s,
                "\t{}{}\t{}{}\t{}\t{}\r\n",
                if it.y < 0.0 { 'S' } else { 'N' },
                fixed(it.y.abs(), 6),
                if it.x < 0.0 { 'W' } else { 'E' },
                fixed(it.x.abs(), 6),
                round(it.speed),
                round(it.angle)
            );
        }
    }
    s
}
