//! Loaded points to a [`Track`]: the parts of `UnitMovingInfo` that decide which records are
//! used and when they happened (GM-103 steps 1–4, GM-104 drop rules and missing speed,
//! course and altitude). The display clean-ups (GM-102, G-sensor normalisation GM-103 5–8)
//! are left to the viewer.

use super::format::{Item, PT_GPS, PT_GSENSOR};
use super::util::{SEC, median};
use crate::track::{GSensorSample, GpsPoint, Track, haversine_km};

/// TDateTime values at or below this are "no time".
const NO_TIME: f64 = 2.0;

#[derive(Debug, Clone, Copy)]
struct GSample {
    time: f64,
    x: f64,
    y: f64,
    z: f64,
    /// Index into the GPS list of the fix that precedes this sample, for an untimed sample.
    link: Option<usize>,
}

/// Builds the track. `base` is the TDateTime of video time 0 for data embedded in a video;
/// for logs the track starts at the first timed record. `file_times` (creation, write) place
/// G-sensor samples of a log without any time (GM-103 step 2). `post_process` = false keeps
/// G-sensor times as recorded.
pub fn to_track(points: &[Item], base: Option<f64>, file_times: (f64, f64), post_process: bool) -> Track {
    let mut gps: Vec<Item> = Vec::new();
    let mut gs: Vec<GSample> = Vec::new();
    let mut last_gps: Option<usize> = None;
    for p in points {
        match p.pt {
            PT_GPS => {
                gps.push(*p);
                last_gps = Some(gps.len() - 1);
            }
            PT_GSENSOR => {
                let link = if p.time == 0.0 { last_gps } else { None };
                gs.push(GSample { time: p.time, x: p.x, y: p.y, z: p.z, link });
                last_gps = None;
            }
            _ => {}
        }
    }
    if post_process {
        gsensor_times(&mut gs, &gps, file_times);
    }
    // GM-104: points without a time or with an integer-degree coordinate (including the zero
    // placeholders) are not used.
    gps.retain(|p| p.time > NO_TIME && p.time.is_finite() && p.x.fract() != 0.0 && p.y.fract() != 0.0);
    gps.sort_by(|a, b| a.time.total_cmp(&b.time));
    gs.retain(|s| s.time > NO_TIME && s.time.is_finite());
    gs.sort_by(|a, b| a.time.total_cmp(&b.time));

    let first = gps.first().map(|p| p.time).into_iter().chain(gs.first().map(|s| s.time)).fold(f64::INFINITY, f64::min);
    let base = base.filter(|b| *b > NO_TIME).or(first.is_finite().then_some(first));
    let Some(base) = base else { return Track::default() };
    // TDateTime resolution is about a microsecond: round to it.
    let rel = |t: f64| ((t - base) / SEC * 1e6).round() / 1e6;
    let mut out: Vec<GpsPoint> = gps
        .iter()
        .map(|p| GpsPoint {
            time: rel(p.time),
            lat: p.y,
            lon: p.x,
            altitude: if p.z.is_finite() { p.z } else { 0.0 },
            speed_kmh: if p.speed.is_finite() { p.speed } else { -1.0 },
            course: if p.angle.is_finite() { p.angle } else { -1.0 },
            warning: p.warn,
        })
        .collect();
    fill_missing(&mut out);
    Track {
        start: crate::rvmi::from_tdatetime(base),
        gps: out,
        gsensor: gs.iter().map(|s| GSensorSample { time: rel(s.time), x: s.x, y: s.y, z: s.z }).collect(),
    }
}

/// `ExtropaleteGSensorTimes` (GM-103 steps 1–4).
fn gsensor_times(gs: &mut Vec<GSample>, gps: &[Item], (create, write): (f64, f64)) {
    if gs.len() == 1 {
        gs.clear();
    }
    if gs.is_empty() {
        return;
    }
    for s in gs.iter_mut() {
        if let Some(k) = s.link
            && s.time <= NO_TIME
            && let Some(g) = gps.get(k)
        {
            s.time = g.time;
        }
    }
    let mut prev = 0.0;
    for s in gs.iter_mut() {
        if s.time <= NO_TIME || s.time == prev {
            s.time = 0.0;
        } else {
            prev = s.time;
        }
    }
    let (mut prev, mut local_dt) = (0.0, 0.0);
    for s in gs.iter_mut().rev() {
        if s.time > 0.0 {
            s.time += local_dt;
            if prev == 0.0 || (prev < s.time + 1.0 && s.time < prev) {
                prev = s.time;
            } else {
                local_dt -= s.time - prev;
                s.time = 0.0;
            }
        }
    }
    let n = gs.len();
    let timed = gs.iter().filter(|s| s.time > NO_TIME).count();
    let mut avg_dts = Vec::new();
    if timed == 0 {
        let mut create = create;
        if write - create <= 0.0 || write - create > 1.0 / 6.0 {
            create = write - 0.1015 * SEC * n as f64;
        }
        gs[0].time = create;
        gs[n - 1].time = write;
    } else {
        avg_dts.push(0.1015 * SEC);
    }
    let (mut prev, mut prev_index) = (0.0, 0usize);
    let (mut first, mut last) = (None, None);
    for i in 0..n {
        let t = gs[i].time;
        if t > NO_TIME {
            if prev > 0.0 && prev < t {
                let k = i - prev_index;
                let d = (t - prev) / k as f64;
                for z in 1..k {
                    gs[prev_index + z].time = prev + z as f64 * d;
                }
                avg_dts.push(d);
            }
            first.get_or_insert(i);
            last = Some(i);
            prev_index = i;
            prev = t;
        }
    }
    let avg = 0.99 * median(&avg_dts);
    if let Some(f) = first.filter(|&f| f > 0) {
        let mut t = gs[f].time;
        for s in gs[..f].iter_mut().rev() {
            t -= avg;
            s.time = t;
        }
    }
    if let Some(l) = last {
        let mut t = gs[l].time;
        for s in gs[l + 1..].iter_mut() {
            t += avg;
            s.time = t;
        }
    }
    let gps_times: Vec<f64> = gps.iter().map(|p| p.time).filter(|&t| t > NO_TIME).collect();
    if let ([g0, .., gn], Some(s0), Some(sn)) = (gps_times.as_slice(), gs.first(), gs.last()) {
        let (g0, gn, s0, sn) = (*g0, *gn, s0.time, sn.time);
        let partial = gn.min(sn) - g0.max(s0) < (gn - g0).min(sn - s0);
        if partial {
            let mut d = gn - sn;
            let t = g0 - s0;
            if t.abs() < d.abs() {
                d = t;
            }
            for s in gs.iter_mut() {
                s.time += d;
            }
        }
    }
}

/// GM-104: unknown speeds from the distance covered around the point, unknown courses from
/// the points two before and four after, missing first/last altitude from the neighbour.
fn fill_missing(gps: &mut [GpsPoint]) {
    let n = gps.len();
    if n == 0 {
        return;
    }
    let mut cum = vec![0.0; n];
    for i in 1..n {
        cum[i] = cum[i - 1] + haversine_km(&gps[i - 1], &gps[i]);
    }
    let speeds: Vec<f64> = (0..n)
        .map(|i| {
            if gps[i].speed_kmh >= 0.0 {
                return gps[i].speed_kmh;
            }
            for w in 1..=10usize {
                let a = i.saturating_sub(w);
                let b = (i + w).min(n - 1);
                let dt = gps[b].time - gps[a].time;
                if dt > 0.0 {
                    return (cum[b] - cum[a]) / (dt / 3600.0);
                }
            }
            0.0
        })
        .collect();
    let courses: Vec<f64> = (0..n)
        .map(|i| {
            if gps[i].course >= 0.0 {
                return gps[i].course;
            }
            let (a, b) = (&gps[i.saturating_sub(2)], &gps[(i + 4).min(n - 1)]);
            let dx = (b.lon - a.lon) * a.lat.to_radians().cos();
            let dy = b.lat - a.lat;
            if dx == 0.0 && dy == 0.0 { -1.0 } else { dx.atan2(dy).to_degrees().rem_euclid(360.0) }
        })
        .collect();
    let first_known = courses.iter().copied().find(|c| *c >= 0.0).unwrap_or(0.0);
    let mut prev = first_known;
    for (p, (s, c)) in gps.iter_mut().zip(speeds.into_iter().zip(courses)) {
        p.speed_kmh = s;
        p.course = if c >= 0.0 { c } else { prev };
        prev = p.course;
    }
    if n > 1 {
        if gps[0].altitude == 0.0 {
            gps[0].altitude = gps[1].altitude;
        }
        if gps[n - 1].altitude == 0.0 {
            gps[n - 1].altitude = gps[n - 2].altitude;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_times_and_missing_values() {
        let t0 = 42_506.5;
        let pts = vec![
            Item::gps(t0, 30.5, 60.5),
            Item::gps(t0 + SEC, 30.5, 60.5001),
            Item::gps(t0 + 2.0 * SEC, 30.0, 60.5002), // integer longitude: dropped
            Item::gps(t0 + 3.0 * SEC, 30.5, 60.5003),
        ];
        let t = to_track(&pts, None, (0.0, 0.0), true);
        assert_eq!(t.gps.len(), 3);
        assert_eq!(t.start.unwrap().format("%H:%M:%S").to_string(), "12:00:00");
        assert_eq!(t.gps[2].time, 3.0);
        // 0.0001 degree of latitude per second is ~40 km/h, heading north
        assert!((t.gps[1].speed_kmh - 40.0).abs() < 1.0, "{}", t.gps[1].speed_kmh);
        assert!(t.gps[1].course < 1e-6 || t.gps[1].course > 359.99);
    }

    #[test]
    fn gsensor_samples_take_times_from_fixes_and_neighbours() {
        let t0 = 42_506.5;
        let pts = vec![
            Item::gps(t0, 30.5, 60.5),
            Item::gsensor(0.0, 0.1, 0.0, 1.0),
            Item::gsensor(0.0, 0.2, 0.0, 1.0),
            Item::gps(t0 + SEC, 30.5, 60.5001),
            Item::gsensor(0.0, 0.3, 0.0, 1.0),
            Item::gsensor(0.0, 0.4, 0.0, 1.0),
        ];
        let t = to_track(&pts, None, (0.0, 0.0), true);
        let times: Vec<f64> = t.gsensor.iter().map(|s| (s.time * 1000.0).round() / 1000.0).collect();
        assert_eq!(times, [0.0, 0.5, 1.0, 1.495]);
        // no times at all: spread over the file window
        let only_g = vec![
            Item::gsensor(0.0, 0.1, 0.0, 1.0),
            Item::gsensor(0.0, 0.2, 0.0, 1.0),
            Item::gsensor(0.0, 0.3, 0.0, 1.0),
        ];
        let t = to_track(&only_g, None, (t0, t0 + 10.0 * SEC), true);
        let times: Vec<f64> = t.gsensor.iter().map(|s| (s.time * 1000.0).round() / 1000.0).collect();
        assert_eq!(times, [0.0, 5.0, 10.0]);
        assert!(to_track(&only_g, None, (0.0, 0.0), true).is_empty());
    }
}
