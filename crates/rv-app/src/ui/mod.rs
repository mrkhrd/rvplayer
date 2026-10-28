//! Main window panels, one module per area of the original form.

pub mod controls;
pub mod file_list;
pub mod graphs;
pub mod icons;
pub mod map_view;
pub mod menu;
pub mod video;

use chrono::{NaiveDateTime, TimeDelta};

/// `m:ss` or `h:mm:ss`.
pub fn format_duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Wall-clock time at `offset` seconds into a recording that started at `start`.
pub fn clock_at(start: Option<NaiveDateTime>, offset: f64) -> Option<NaiveDateTime> {
    start?.checked_add_signed(TimeDelta::try_milliseconds((offset * 1000.0) as i64)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(5.4), "0:05");
        assert_eq!(format_duration(73.5), "1:14");
        assert_eq!(format_duration(3725.0), "1:02:05");
    }
}
