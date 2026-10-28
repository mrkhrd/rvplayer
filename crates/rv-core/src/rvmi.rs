//! Registrator Viewer metadata track (`RVMI` sample description, `data` handler).
//!
//! Written by Registrator Viewer when it saves video fragments. Each sample is one record,
//! little-endian, starting with a 4-byte tag:
//!
//! | tag    | payload                                                               |
//! |--------|-----------------------------------------------------------------------|
//! | `tReV` | f64 start time as Delphi `TDateTime` (days since 1899-12-30)           |
//! | `gReV` | i32 lon×1e6, i32 lat×1e6, i32 altitude, u16 speed×10 km/h, u8 course/2, u8 warning |
//! | `sReV` | i16 x, i16 y, i16 z in 1/1000 g                                        |
//!
//! A record's time is the start time plus the sample's presentation time.
//! GPS records with zero coordinates are placeholders written while there was no fix.

use std::io::{self, Read, Seek, SeekFrom};

use chrono::{NaiveDate, NaiveDateTime, TimeDelta};

use crate::mp4::Movie;
use crate::track::{GSensorSample, GpsPoint, Track};

pub const CODEC: [u8; 4] = *b"RVMI";

/// Decodes the RVMI track of `movie`, if present.
pub fn read_track<R: Read + Seek>(r: &mut R, movie: &Movie) -> io::Result<Option<Track>> {
    let Some(t) = movie.track_by_codec(&CODEC) else {
        return Ok(None);
    };
    let mut track = Track::default();
    let mut buf = Vec::new();
    for s in t.samples() {
        buf.resize(s.size as usize, 0);
        r.seek(SeekFrom::Start(s.offset))?;
        r.read_exact(&mut buf)?;
        decode_record(&buf, s.time, &mut track);
    }
    Ok(Some(track))
}

pub fn decode_record(rec: &[u8], time: f64, track: &mut Track) {
    let (Some(tag), payload) = (rec.get(..4), rec.get(4..).unwrap_or_default()) else {
        return;
    };
    let i32_at = |o: usize| {
        payload
            .get(o..o + 4)
            .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
    };
    let i16_at = |o: usize| {
        payload
            .get(o..o + 2)
            .map(|b| i16::from_le_bytes(b.try_into().unwrap()))
    };
    match tag {
        b"tReV" => {
            if let Some(b) = payload.get(..8) {
                track.start = from_tdatetime(f64::from_le_bytes(b.try_into().unwrap()));
            }
        }
        b"gReV" => {
            let (Some(x), Some(y), Some(h), Some(tail)) =
                (i32_at(0), i32_at(4), i32_at(8), payload.get(12..16))
            else {
                return;
            };
            if x == 0 && y == 0 {
                return;
            }
            track.gps.push(GpsPoint {
                time,
                lat: f64::from(y) / 1e6,
                lon: f64::from(x) / 1e6,
                altitude: f64::from(h),
                speed_kmh: f64::from(u16::from_le_bytes([tail[0], tail[1]])) / 10.0,
                course: f64::from(tail[2]) * 2.0,
                warning: tail[3],
            });
        }
        b"sReV" => {
            if let (Some(x), Some(y), Some(z)) = (i16_at(0), i16_at(2), i16_at(4)) {
                let g = |v: i16| f64::from(v) / 1000.0;
                track.gsensor.push(GSensorSample {
                    time,
                    x: g(x),
                    y: g(y),
                    z: g(z),
                });
            }
        }
        _ => {}
    }
}

/// Delphi `TDateTime`: days since 1899-12-30, fractional part is the time of day.
pub fn from_tdatetime(days: f64) -> Option<NaiveDateTime> {
    if !days.is_finite() || days <= 0.0 {
        return None;
    }
    let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)?.and_hms_opt(0, 0, 0)?;
    let ms = (days * 86_400_000.0).round() as i64;
    epoch.checked_add_signed(TimeDelta::try_milliseconds(ms)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4::read_movie;
    use crate::testutil::movie_with_track;

    fn g_rev(lon: i32, lat: i32, speed10: u16, course_half: u8) -> Vec<u8> {
        let mut v = b"gReV".to_vec();
        for w in [lon, lat, 0] {
            v.extend_from_slice(&w.to_le_bytes());
        }
        v.extend_from_slice(&speed10.to_le_bytes());
        v.extend_from_slice(&[course_half, 0, 0x0c]);
        v
    }

    #[test]
    fn decodes_records_from_sample_file() {
        // Values taken from "Fragment of 05161259_0066 ... .MOV" (first samples).
        let mut t_rev = b"tReV".to_vec();
        t_rev.extend_from_slice(&[0x2c, 0xca, 0xf3, 0x79, 0x51, 0xc1, 0xe4, 0x40, 0x0c]);
        let mut s_rev = b"sReV".to_vec();
        for v in [-120i16, 15, 980] {
            s_rev.extend_from_slice(&v.to_le_bytes());
        }
        let payloads = vec![
            t_rev,
            g_rev(0, 0, 0, 0),
            g_rev(30_327_022, 60_063_627, 701, 56),
            s_rev,
        ];
        let file = movie_with_track(b"data", b"RVMI", 50, &payloads);
        let mut r = io::Cursor::new(&file);
        let movie = read_movie(&mut r).unwrap();
        let track = read_track(&mut r, &movie).unwrap().unwrap();

        assert_eq!(
            track.start.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(),
            "2016-05-16 13:06:26"
        );
        assert_eq!(track.gps.len(), 1, "zero-coordinate placeholder is skipped");
        let p = track.gps[0];
        assert_eq!(
            (p.lat, p.lon, p.speed_kmh, p.course, p.time),
            (60.063627, 30.327022, 70.1, 112.0, 2.0)
        );
        assert_eq!(track.gsensor[0].z, 0.98);
    }

    #[test]
    fn rejects_invalid_tdatetime() {
        assert_eq!(from_tdatetime(0.0), None);
        assert_eq!(from_tdatetime(f64::NAN), None);
    }
}
