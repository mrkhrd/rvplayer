//! What the writers write (`TTrackInfo`), built from one or more tracks like
//! `TMovingInfo.AddGpsInfo` builds it for "Export Track" (FL-070) and the GPS converter's
//! "Merge into one file" (GM-192).

use super::WriteOptions;
use super::text::{MS, distance, duration_str, for_file, general, round};
use crate::gps::format::{Item, PT_GPS};
use crate::gps::naive_to_tdatetime;
use crate::gps::util::SEC;
use crate::track::Track;

/// One record of the export (`TTrackInfoItem`): a GPS fix or a G-sensor sample with a local
/// TDateTime, plus the GPS `trackPos`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InfoPoint {
    pub item: Item,
    /// GPS: distance along the track from the first fix of its source, in metres.
    pub track_pos: f64,
}

/// `TTrackInfo`: the merged records and their summary. Build it with [`TrackInfo::add`] or
/// [`track_info`](super::track_info); `name` and `desc` may be replaced before writing.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackInfo {
    /// "Track 2016.05.16 13-06-26" (start time plus the user clock shift).
    pub name: String,
    /// "Duration: 0:12:34. Length: 5678m. AvgSpeed: 27.1km/h."
    pub desc: String,
    /// `tl`: (lowest longitude, highest latitude) of the good fixes; (1000, -1000) without any.
    pub tl: (f64, f64),
    /// `br`: (highest longitude, lowest latitude) of the good fixes; (-1000, 1000) without any.
    pub br: (f64, f64),
    /// Middle of `tl`..`br` (lon, lat).
    pub center: (f64, f64),
    /// East-west and north-south extent through the centre, metres.
    pub width_m: f64,
    pub height_m: f64,
    /// Local TDateTime of the start and end of the sources; 0 = none.
    pub start: f64,
    pub finish: f64,
    /// Sum of the sources' track lengths, metres.
    pub track_len: f64,
    /// km/h over `start..finish`.
    pub avg_speed: f64,
    /// Each source's GPS and G-sensor records merged by time; sources one after another.
    pub points: Vec<InfoPoint>,
}

impl Default for TrackInfo {
    /// `emptyGPSInfo`.
    fn default() -> TrackInfo {
        TrackInfo {
            name: String::new(),
            desc: String::new(),
            tl: (1000.0, -1000.0),
            br: (-1000.0, 1000.0),
            center: (0.0, 0.0),
            width_m: 0.0,
            height_m: 0.0,
            start: 0.0,
            finish: 0.0,
            track_len: 0.0,
            avg_speed: 0.0,
            points: Vec::new(),
        }
    }
}

/// G-sensor samples go after GPS fixes up to this much later (`AddGpsInfo`: 1 ms).
const MERGE_SLACK: f64 = MS;

impl TrackInfo {
    /// Whether there is at least one record (FL-070 "has GPS").
    pub fn has_points(&self) -> bool {
        !self.points.is_empty()
    }

    /// `TMovingInfo.AddGpsInfo`: appends one source.
    ///
    /// `duration` is the length of the source in seconds when it is a video: the source then
    /// spans `track.start .. track.start + duration` (`CreateAutoSelectSource` with the file's
    /// time window), which places SRT cues and the KML time span on the video. Without it the
    /// source spans its first to last record (and `track.start`).
    ///
    /// The track's times are seconds from `track.start`; a track without a start gets times
    /// from TDateTime 0, which the writers treat as "no time".
    pub fn add(&mut self, track: &Track, duration: Option<f64>, opts: &WriteOptions) {
        let base = track.start.map_or(0.0, naive_to_tdatetime);
        let gps: Vec<Item> = track
            .gps
            .iter()
            .map(|p| Item {
                pt: PT_GPS,
                time: base + p.time * SEC,
                x: p.lon,
                y: p.lat,
                z: p.altitude,
                angle: p.course,
                speed: p.speed_kmh,
                warn: p.warning,
            })
            .collect();
        let gs: Vec<Item> =
            track.gsensor.iter().map(|s| Item::gsensor(base + s.time * SEC, s.x, s.y, s.z)).collect();
        let track_pos = track_positions(&gps);

        // minTime / maxTime of the source
        let times = gps.iter().chain(&gs).map(|it| it.time);
        let (mut min_t, mut max_t) = times.fold((f64::INFINITY, 0.0f64), |(a, b), t| (a.min(t), b.max(t)));
        if track.start.is_some() {
            min_t = min_t.min(base);
        }
        if let (Some(d), Some(_)) = (duration, track.start) {
            min_t = base;
            max_t = base + d * SEC;
        }
        if !min_t.is_finite() {
            min_t = 0.0;
        }
        if min_t > 0.0 && (self.start == 0.0 || min_t < self.start) {
            self.start = min_t;
        }
        self.finish = self.finish.max(max_t);

        // merge GPS and G-sensor records by time
        self.points.reserve(gps.len() + gs.len());
        let (mut ng, mut ns) = (0, 0);
        let mut next_t = 0.0;
        while ng < gps.len() || ns < gs.len() {
            let mut stopped = false;
            while ng < gps.len() {
                if next_t <= gps[ng].time {
                    next_t = gps[ng].time;
                    stopped = true;
                    break;
                }
                let it = gps[ng];
                if it.warn == 0 {
                    self.tl = (self.tl.0.min(it.x), self.tl.1.max(it.y));
                    self.br = (self.br.0.max(it.x), self.br.1.min(it.y));
                }
                self.points.push(InfoPoint { item: it, track_pos: track_pos[ng] });
                ng += 1;
            }
            if !stopped {
                next_t = 1e7;
            }
            let mut stopped = false;
            while ns < gs.len() {
                if next_t <= gs[ns].time {
                    next_t = gs[ns].time + MERGE_SLACK;
                    stopped = true;
                    break;
                }
                self.points.push(InfoPoint { item: gs[ns], track_pos: 0.0 });
                ns += 1;
            }
            if !stopped {
                next_t = 1e7;
            }
        }

        self.center = ((self.tl.0 + self.br.0) / 2.0, (self.tl.1 + self.br.1) / 2.0);
        self.width_m = distance(self.tl.0, self.center.1, self.br.0, self.center.1);
        self.height_m = distance(self.center.0, self.tl.1, self.center.0, self.br.1);
        if let (Some(first), Some(last)) = (track_pos.first(), track_pos.last()) {
            self.track_len += last - first;
        }
        self.avg_speed = if self.finish <= self.start {
            0.0
        } else {
            self.track_len / 1000.0 / ((self.finish - self.start) * 24.0)
        };
        self.name = format!("Track {}", for_file(self.start + opts.user_shift));
        let avg = round(self.avg_speed * 10.0) as f64 / 10.0;
        self.desc = format!(
            "Duration: {}. Length: {}m. AvgSpeed: {}km/h.",
            duration_str(self.finish - self.start).replace(':', &opts.locale.time_separator.to_string()),
            round(self.track_len),
            opts.locale.num(&general(avg, 15, 0)),
        );
    }
}

/// `trackPos` as `TMovingInfo.CalculateEmptyDatas` sets it: each fix carries the position of
/// the last good fix before it, and a good fix adds its distance from that one unless the step
/// looks like a jump (500 m/s or faster, 1 km or more, or 20 s or more apart).
fn track_positions(gps: &[Item]) -> Vec<f64> {
    let mut out = vec![0.0; gps.len()];
    let mut last: Option<usize> = None;
    for (i, p) in gps.iter().enumerate() {
        if let Some(l) = last {
            out[i] = out[l];
            if p.warn == 0 {
                let q = &gps[l];
                let d = distance(p.x, p.y, q.x, q.y);
                let dt = (q.time - p.time).abs();
                if d / ((MS + dt) / SEC) < 500.0 && d < 1000.0 && dt < 20.0 * SEC {
                    out[i] += d;
                }
            }
        }
        if p.warn == 0 {
            last = Some(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{GSensorSample, GpsPoint};

    fn gp(time: f64, lat: f64, lon: f64, warning: u8) -> GpsPoint {
        GpsPoint { time, lat, lon, altitude: 10.0, speed_kmh: 40.0, course: 0.0, warning }
    }

    #[test]
    fn merges_and_summarises() {
        let start = chrono::NaiveDate::from_ymd_opt(2016, 5, 16).unwrap().and_hms_opt(13, 6, 26).unwrap();
        let track = Track {
            start: Some(start),
            gps: vec![
                gp(0.0, 60.0, 30.0, 0),
                gp(1.0, 60.0001, 30.0, 0),
                gp(2.0, 60.0002, 30.0, 2),
                gp(3.0, 60.0003, 30.0, 0),
                gp(60.0, 60.1, 30.0, 0),
            ],
            gsensor: vec![
                GSensorSample { time: 0.5, x: 0.1, y: 0.0, z: 1.0 },
                GSensorSample { time: 1.0, x: 0.2, y: 0.0, z: 1.0 },
            ],
        };
        let mut info = TrackInfo::default();
        info.add(&track, None, &WriteOptions::default());
        let kinds: Vec<u8> = info.points.iter().map(|p| p.item.pt).collect();
        assert_eq!(kinds, [1, 2, 1, 2, 1, 1, 1], "a fix goes before a sample of the same time");
        assert_eq!(info.name, "Track 2016.05.16 13-06-26");
        // the bad fix is skipped by the bounds and the length; the 57 s jump adds nothing
        let pos: Vec<i64> = info.points.iter().filter(|p| p.item.pt == PT_GPS).map(|p| round(p.track_pos)).collect();
        assert_eq!(pos, [0, 11, 11, 33, 33]);
        assert_eq!(round(info.track_len), 33);
        assert_eq!(info.tl, (30.0, 60.1));
        assert_eq!(info.br, (30.0, 60.0));
        assert!((info.height_m - 11_119.5).abs() < 1.0, "{}", info.height_m);
        assert_eq!(info.desc, "Duration: 0:01:00. Length: 33m. AvgSpeed: 2km/h.");
        // a second source extends the span; the name keeps the first start
        let later = Track { start: Some(start + chrono::TimeDelta::hours(1)), ..track.clone() };
        info.add(&later, Some(120.0), &WriteOptions::default());
        assert_eq!(info.points.len(), 14);
        assert_eq!(info.name, "Track 2016.05.16 13-06-26");
        assert_eq!(round(info.track_len), 67);
        assert!(info.desc.starts_with("Duration: 1:02:00."), "{}", info.desc);
    }
}
