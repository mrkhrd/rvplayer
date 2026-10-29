//! The combined GPS / G-sensor track embedded in the output as RVMI records.
//!
//! Ports `TMovingInfo.ExportData` (UnitMovingInfo L1313-L1338), `TFormCropping.AppendGPS`
//! (L559-L602), `TFormCropping.FilterForSpeedUp` (L529-L553) and the "fragment" saver
//! `TGpsFormat_RegV.SaveTrack` (UnitGps_RegV L143-L181). Times are seconds relative to an
//! arbitrary epoch instead of `TDateTime` days.

use chrono::{NaiveDate, NaiveDateTime};
use rv_core::track::Track;

use crate::source::round;

/// Segment-break threshold: a gap between joined pieces larger than this (1 hour).
const SEGMENT_GAP: f64 = 3600.0;
/// Short pieces (< 20 s) also take the GPS point before the window if it is < 11 s older.
const SHORT_WINDOW: f64 = 20.0;
const PREV_POINT_MAX_AGE: f64 = 11.0;
/// Thinning after concatenation: at most one GPS fix per 0.21 s, one G-sensor sample per 0.03 s.
const GPS_MIN_STEP: f64 = 0.21;
const GSENSOR_MIN_STEP: f64 = 0.03;
/// Time scale of the RVMI data track.
pub(crate) const RVMI_SCALE: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PointKind {
    /// A GPS fix; all-zero (lat = lon = 0) for the piece markers.
    Gps {
        lat: f64,
        lon: f64,
        altitude: f64,
        speed_kmh: f64,
        course: f64,
        /// Camera warning byte; set to 1 on the last fix before a segment break.
        flag: u8,
    },
    GSensor {
        x: f64,
        y: f64,
        z: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Point {
    pub time: f64,
    pub kind: PointKind,
}

impl Point {
    fn marker(time: f64) -> Point {
        Point {
            time,
            kind: PointKind::Gps {
                lat: 0.0,
                lon: 0.0,
                altitude: 0.0,
                speed_kmh: 0.0,
                course: 0.0,
                flag: 0,
            },
        }
    }
}

/// `ExportData`: the points of `track` in the window `[t1, t2]` (G-sensor `[t1, t2)`), GPS and
/// G-sensor interleaved by time. `base` is the time of the track's time 0 on the same clock.
pub(crate) fn export_window(track: &Track, base: f64, t1: f64, t2: f64) -> Vec<Point> {
    let gps = &track.gps;
    let gs = &track.gsensor;
    let gps_t = |i: usize| base + gps[i].time;
    let gs_t = |i: usize| base + gs[i].time;
    let gps_point = |i: usize| {
        let p = &gps[i];
        Point {
            time: gps_t(i),
            kind: PointKind::Gps {
                lat: p.lat,
                lon: p.lon,
                altitude: p.altitude,
                speed_kmh: p.speed_kmh,
                course: p.course,
                flag: p.warning,
            },
        }
    };
    let gs_point = |i: usize| Point {
        time: gs_t(i),
        kind: PointKind::GSensor {
            x: gs[i].x,
            y: gs[i].y,
            z: gs[i].z,
        },
    };
    let mut out = Vec::new();
    let mut ig = 0;
    while ig < gps.len() && gps_t(ig) < t1 {
        ig += 1;
    }
    let mut is = 0;
    while is < gs.len() && gs_t(is) < t1 {
        is += 1;
    }
    if t2 - t1 < SHORT_WINDOW
        && ig > 0
        && ig < gps.len()
        && gps_t(ig) - gps_t(ig - 1) < PREV_POINT_MAX_AGE
    {
        out.push(gps_point(ig - 1));
    }
    while ig < gps.len() && gps_t(ig) <= t2 {
        while is < gs.len() && gs_t(is) < gps_t(ig) {
            out.push(gs_point(is));
            is += 1;
        }
        out.push(gps_point(ig));
        ig += 1;
    }
    while is < gs.len() && gs_t(is) < t2 {
        out.push(gs_point(is));
        is += 1;
    }
    out
}

/// The combined track (`TTrackInfo`): start, finish and points on one clock.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Combined {
    pub start: Option<f64>,
    pub finish: f64,
    pub points: Vec<Point>,
}

impl Combined {
    /// `AppendGPS`: appends a piece covering `[start, finish]` of its own clock, played at
    /// `speed`. The gap to the current finish is removed, times are compressed by `1 / speed`,
    /// and marker fixes are put at both ends of the piece.
    pub fn append(&mut self, start: f64, finish: f64, points: &[Point], speed: f64) {
        let k = 1.0 / speed;
        if self.start.is_none() {
            self.start = Some(start);
            self.finish = start;
        }
        let f = self.finish;
        let gap = start - f;
        self.finish = ((finish - gap) - f) * k + f;
        if gap.abs() > SEGMENT_GAP
            && let Some(PointKind::Gps { flag, .. }) = self
                .points
                .iter_mut()
                .rev()
                .map(|p| &mut p.kind)
                .find(|k| matches!(k, PointKind::Gps { .. }))
        {
            *flag = 1;
        }
        self.points.push(Point::marker(f));
        for p in points {
            let mut time = p.time - gap;
            if k != 1.0 {
                time = (time - f) * k + f;
            }
            self.points.push(Point { time, ..*p });
        }
        self.points.push(Point::marker((finish - start) * k + f));
    }

    /// `FilterForSpeedUp`: drops GPS fixes closer than 0.21 s and G-sensor samples closer than
    /// 0.03 s to the previous kept one. Markers (zero coordinates) are always kept.
    pub fn thin(&mut self) {
        let mut last_gps: Option<f64> = None;
        let mut last_gs: Option<f64> = None;
        self.points.retain(|p| {
            let (last, step) = match p.kind {
                PointKind::Gps { lat, lon, .. } if lat != 0.0 || lon != 0.0 => {
                    (&mut last_gps, GPS_MIN_STEP)
                }
                PointKind::GSensor { .. } => (&mut last_gs, GSENSOR_MIN_STEP),
                PointKind::Gps { .. } => return true,
            };
            if last.is_none_or(|l| step <= p.time - l) {
                *last = Some(p.time);
                true
            } else {
                false
            }
        });
    }

    /// `SaveTrack` of the RegV ("fragment") format: the start record, then one record per point,
    /// each with its time in RVMI ticks relative to the start. `epoch` is the wall-clock time
    /// of time 0 of the combined clock.
    pub fn records(&self, epoch: NaiveDateTime) -> Vec<(i64, Vec<u8>)> {
        let start = self.start.unwrap_or(0.0);
        let mut out = Vec::with_capacity(self.points.len() + 1);
        let mut t_rev = b"tReV".to_vec();
        t_rev.extend_from_slice(&to_tdatetime(epoch, start).to_le_bytes());
        t_rev.push(0x0c);
        out.push((0, t_rev));
        // The record buffer is reused: negative speed / course keep the previous value.
        let (mut speed10, mut course2) = (0u16, 0u8);
        for p in &self.points {
            let ticks = round((p.time - start) * f64::from(RVMI_SCALE));
            let rec = match p.kind {
                PointKind::Gps {
                    lat,
                    lon,
                    altitude,
                    speed_kmh,
                    course,
                    flag,
                } => {
                    if speed_kmh >= 0.0 {
                        speed10 = round(speed_kmh * 10.0).clamp(0, 0xffff) as u16;
                    }
                    if course >= 0.0 {
                        course2 = round(course / 2.0).clamp(0, 0xff) as u8;
                    }
                    let mut r = b"gReV".to_vec();
                    r.extend_from_slice(&(clamp_i32(round(lon * 1e6))).to_le_bytes());
                    r.extend_from_slice(&(clamp_i32(round(lat * 1e6))).to_le_bytes());
                    r.extend_from_slice(&(altitude as f32).to_le_bytes());
                    r.extend_from_slice(&speed10.to_le_bytes());
                    r.extend_from_slice(&[course2, flag, 0x0c]);
                    r
                }
                PointKind::GSensor { x, y, z } => {
                    let mut r = b"sReV".to_vec();
                    for v in [x, y, z] {
                        let g = round(v * 1000.0).clamp(i64::from(i16::MIN), i64::from(i16::MAX));
                        r.extend_from_slice(&(g as i16).to_le_bytes());
                    }
                    r.push(0x0c);
                    r
                }
            };
            out.push((ticks, rec));
        }
        out
    }
}

fn clamp_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Delphi `TDateTime` (days since 1899-12-30) of `epoch + seconds`.
pub(crate) fn to_tdatetime(epoch: NaiveDateTime, seconds: f64) -> f64 {
    let base = NaiveDate::from_ymd_opt(1899, 12, 30)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .expect("valid date");
    let ms = (epoch - base).num_milliseconds() as f64;
    (ms / 1000.0 + seconds) / 86_400.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_core::track::{GSensorSample, GpsPoint};

    fn gps(time: f64, lat: f64) -> GpsPoint {
        GpsPoint {
            time,
            lat,
            lon: 30.0,
            altitude: 0.0,
            speed_kmh: 50.0,
            course: 90.0,
            warning: 0,
        }
    }

    fn gs(time: f64) -> GSensorSample {
        GSensorSample {
            time,
            x: 0.1,
            y: 0.2,
            z: 1.0,
        }
    }

    #[test]
    fn window_interleaves_and_takes_previous_fix_for_short_pieces() {
        let track = Track {
            start: None,
            gps: (0..10).map(|i| gps(f64::from(i), 60.0)).collect(),
            gsensor: (0..40).map(|i| gs(f64::from(i) * 0.25)).collect(),
        };
        let pts = export_window(&track, 100.0, 102.5, 104.0);
        let times: Vec<f64> = pts.iter().map(|p| p.time).collect();
        // the fix before the window (short piece), G-sensor samples before each fix, the fix
        // at the window end; no G-sensor sample at the end itself
        assert_eq!(
            times,
            [
                102.0, 102.5, 102.75, 103.0, 103.0, 103.25, 103.5, 103.75, 104.0
            ]
        );
        let gps_at = |i: usize| matches!(pts[i].kind, PointKind::Gps { .. });
        assert!(gps_at(0) && !gps_at(1) && !gps_at(2));
        assert!(
            gps_at(3) && !gps_at(4),
            "a fix goes before a G-sensor sample of equal time"
        );
        assert!(gps_at(8));
        // long pieces do not take the previous fix
        let pts = export_window(&track, 100.0, 102.5, 130.0);
        assert_eq!(pts[0].time, 102.5);
    }

    #[test]
    fn append_removes_gap_compresses_and_marks() {
        let mut c = Combined::default();
        let p = |time: f64| Point {
            time,
            kind: PointKind::GSensor {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        };
        c.append(10.0, 20.0, &[p(10.0), p(15.0)], 1.0);
        assert_eq!(c.start, Some(10.0));
        assert_eq!(c.finish, 20.0);
        // second piece 2 hours later at speed 4: gap removed, times compressed
        c.append(7210.0, 7230.0, &[p(7210.0), p(7218.0)], 4.0);
        assert_eq!(c.finish, 25.0);
        let times: Vec<f64> = c.points.iter().map(|p| p.time).collect();
        assert_eq!(times, [10.0, 10.0, 15.0, 20.0, 20.0, 20.0, 22.0, 25.0]);
        // the gap > 1 h flags the last fix before it (the end marker of the first piece)
        assert!(matches!(c.points[3].kind, PointKind::Gps { flag: 1, .. }));
        assert!(matches!(c.points[4].kind, PointKind::Gps { flag: 0, .. }));
    }

    #[test]
    fn thinning_keeps_markers() {
        let fix = |time: f64, lat: f64| Point {
            time,
            kind: PointKind::Gps {
                lat,
                lon: lat,
                altitude: 0.0,
                speed_kmh: 0.0,
                course: 0.0,
                flag: 0,
            },
        };
        let mut c = Combined {
            start: Some(0.0),
            finish: 1.0,
            points: vec![
                fix(0.0, 0.0),
                fix(0.0, 1.0),
                fix(0.1, 1.0),
                fix(0.1, 0.0),
                fix(0.25, 1.0),
            ],
        };
        c.thin();
        let kept: Vec<(f64, bool)> = c
            .points
            .iter()
            .map(|p| {
                (
                    p.time,
                    matches!(p.kind, PointKind::Gps { lat, .. } if lat == 0.0),
                )
            })
            .collect();
        assert_eq!(
            kept,
            [(0.0, true), (0.0, false), (0.1, true), (0.25, false)]
        );
    }

    #[test]
    fn records_match_rvmi_layout() {
        let epoch = NaiveDate::from_ymd_opt(2016, 5, 16)
            .unwrap()
            .and_hms_opt(13, 6, 26)
            .unwrap();
        let mut c = Combined::default();
        let fix = Point {
            time: 1.0,
            kind: PointKind::Gps {
                lat: 60.063627,
                lon: 30.327022,
                altitude: 12.5,
                speed_kmh: 70.1,
                course: 112.0,
                flag: 0,
            },
        };
        c.append(0.0, 2.0, &[fix], 1.0);
        let recs = c.records(epoch);
        assert_eq!(recs.len(), 4);
        assert_eq!((recs[0].0, recs[0].1.len()), (0, 13));
        assert_eq!(&recs[0].1[..4], b"tReV");
        assert_eq!((recs[2].0, recs[2].1.len()), (50, 21));
        let mut track = Track::default();
        for (t, r) in &recs {
            rv_core::rvmi::decode_record(r, *t as f64 / 50.0, &mut track);
        }
        assert_eq!(track.start, Some(epoch));
        assert_eq!(
            track.gps.len(),
            1,
            "markers are zero-coordinate placeholders"
        );
        let p = track.gps[0];
        assert_eq!(
            (p.lat, p.lon, p.speed_kmh, p.course, p.time),
            (60.063627, 30.327022, 70.1, 112.0, 1.0)
        );
    }
}
