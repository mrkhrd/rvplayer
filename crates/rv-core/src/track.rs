//! GPS / G-sensor track model shared by all formats.

use chrono::NaiveDateTime;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpsPoint {
    /// Seconds from the start of the video.
    pub time: f64,
    pub lat: f64,
    pub lon: f64,
    /// Meters; 0 when the camera does not record altitude.
    pub altitude: f64,
    pub speed_kmh: f64,
    /// Degrees clockwise from north.
    pub course: f64,
    /// Camera-specific warning flag (speed camera alerts and similar).
    pub warning: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GSensorSample {
    pub time: f64,
    /// Acceleration in g.
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl GSensorSample {
    pub fn magnitude(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    /// Wall-clock time of video time 0, as recorded by the camera.
    pub start: Option<NaiveDateTime>,
    pub gps: Vec<GpsPoint>,
    pub gsensor: Vec<GSensorSample>,
}

impl Track {
    pub fn is_empty(&self) -> bool {
        self.gps.is_empty() && self.gsensor.is_empty()
    }

    pub fn distance_km(&self) -> f64 {
        self.gps
            .windows(2)
            .map(|w| haversine_km(&w[0], &w[1]))
            .sum()
    }

    pub fn max_speed_kmh(&self) -> f64 {
        self.gps.iter().map(|p| p.speed_kmh).fold(0.0, f64::max)
    }

    /// Average speed over the time covered by GPS points.
    pub fn avg_speed_kmh(&self) -> f64 {
        match (self.gps.first(), self.gps.last()) {
            (Some(a), Some(b)) if b.time > a.time => {
                self.distance_km() / ((b.time - a.time) / 3600.0)
            }
            _ => 0.0,
        }
    }

    /// GPS state at video time `t`, linearly interpolated between neighbouring points.
    pub fn position_at(&self, t: f64) -> Option<GpsPoint> {
        let gps = &self.gps;
        let first = gps.first()?;
        if t <= first.time {
            return Some(*first);
        }
        let i = gps.partition_point(|p| p.time <= t);
        let Some(b) = gps.get(i) else {
            return gps.last().copied();
        };
        let a = &gps[i - 1];
        let k = if b.time > a.time {
            (t - a.time) / (b.time - a.time)
        } else {
            0.0
        };
        let lerp = |x: f64, y: f64| x + (y - x) * k;
        Some(GpsPoint {
            time: t,
            lat: lerp(a.lat, b.lat),
            lon: lerp(a.lon, b.lon),
            altitude: lerp(a.altitude, b.altitude),
            speed_kmh: lerp(a.speed_kmh, b.speed_kmh),
            course: lerp_angle(a.course, b.course, k),
            warning: a.warning,
        })
    }

    /// G-sensor sample nearest to video time `t`.
    pub fn gsensor_at(&self, t: f64) -> Option<GSensorSample> {
        let i = self.gsensor.partition_point(|s| s.time < t);
        let after = self.gsensor.get(i);
        let before = i.checked_sub(1).and_then(|j| self.gsensor.get(j));
        match (before, after) {
            (Some(a), Some(b)) => Some(if t - a.time <= b.time - t { *a } else { *b }),
            (a, b) => a.or(b).copied(),
        }
    }

    /// Video time of the GPS point closest to the given position.
    pub fn nearest_time(&self, lat: f64, lon: f64) -> Option<f64> {
        let target = GpsPoint {
            time: 0.0,
            lat,
            lon,
            altitude: 0.0,
            speed_kmh: 0.0,
            course: 0.0,
            warning: 0,
        };
        self.gps
            .iter()
            .min_by(|a, b| haversine_km(a, &target).total_cmp(&haversine_km(b, &target)))
            .map(|p| p.time)
    }
}

pub fn haversine_km(a: &GpsPoint, b: &GpsPoint) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0088;
    let (la1, la2) = (a.lat.to_radians(), b.lat.to_radians());
    let dlat = la2 - la1;
    let dlon = (b.lon - a.lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * h.sqrt().asin()
}

fn lerp_angle(a: f64, b: f64, k: f64) -> f64 {
    let d = ((b - a + 540.0) % 360.0) - 180.0;
    (a + d * k).rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(time: f64, lat: f64, lon: f64, speed: f64, course: f64) -> GpsPoint {
        GpsPoint {
            time,
            lat,
            lon,
            altitude: 0.0,
            speed_kmh: speed,
            course,
            warning: 0,
        }
    }

    #[test]
    fn distance_and_speeds() {
        // 0.01 degree of latitude is ~1.112 km
        let t = Track {
            gps: vec![
                pt(0.0, 60.0, 30.0, 40.0, 0.0),
                pt(60.0, 60.01, 30.0, 80.0, 0.0),
            ],
            ..Default::default()
        };
        assert!((t.distance_km() - 1.112).abs() < 0.002);
        assert!((t.avg_speed_kmh() - 66.7).abs() < 0.2);
        assert_eq!(t.max_speed_kmh(), 80.0);
    }

    #[test]
    fn interpolates_position_and_wraps_course() {
        let t = Track {
            gps: vec![
                pt(0.0, 60.0, 30.0, 40.0, 350.0),
                pt(2.0, 60.002, 30.0, 60.0, 10.0),
            ],
            ..Default::default()
        };
        let p = t.position_at(1.0).unwrap();
        assert!((p.lat - 60.001).abs() < 1e-9);
        assert_eq!(p.speed_kmh, 50.0);
        assert!(p.course.abs() < 1e-9 || (p.course - 360.0).abs() < 1e-9);
        assert_eq!(t.position_at(-5.0).unwrap().time, 0.0);
        assert_eq!(t.position_at(99.0).unwrap().time, 2.0);
        assert_eq!(t.nearest_time(60.0019, 30.0), Some(2.0));
    }
}
