//! Delphi `TDateTime` values: days since 1899-12-30 in local time. Values <= 2.0 mean "no date"
//! (spec §4.1).

use chrono::{Datelike, NaiveDate, NaiveDateTime};

/// TDateTime values at or below this are "no date".
pub const NO_DATE: f64 = 2.0;
/// One second in days.
pub const SEC: f64 = 1.0 / 86_400.0;
pub const MS_PER_DAY: f64 = 86_400_000.0;

pub fn has_date(t: f64) -> bool {
    t > NO_DATE
}

fn epoch() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(1899, 12, 30)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .expect("valid epoch")
}

pub fn from_naive(dt: NaiveDateTime) -> f64 {
    let d = dt - epoch();
    d.num_seconds() as f64 / 86_400.0 + f64::from(d.subsec_nanos()) / 86_400e9
}

pub fn to_naive(t: f64) -> Option<NaiveDateTime> {
    rv_core::rvmi::from_tdatetime(t)
}

/// Delphi `DecodeDate(t)` reports 1 January.
pub fn is_first_of_january(t: f64) -> bool {
    to_naive(t).is_some_and(|d| d.month() == 1 && d.day() == 1)
}

/// File-system creation and last-write times as local TDateTime values, converted like the
/// original (`FileTimeToSystemTime` + the current local bias, i.e. `FileTimeToLocalFileTime`).
pub fn fs_times(meta: &std::fs::Metadata) -> (f64, f64) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        (
            crate::platform::filetime_to_local(meta.creation_time()),
            crate::platform::filetime_to_local(meta.last_write_time()),
        )
    }
    #[cfg(not(windows))]
    {
        let conv = |t: std::io::Result<std::time::SystemTime>| {
            t.ok()
                .map(|t| from_naive(chrono::DateTime::<chrono::Local>::from(t).naive_local()))
                .unwrap_or(0.0)
        };
        (conv(meta.created()), conv(meta.modified()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_tdatetime() {
        let dt = NaiveDate::from_ymd_opt(2016, 5, 16)
            .unwrap()
            .and_hms_milli_opt(13, 6, 26, 500)
            .unwrap();
        let t = from_naive(dt);
        assert!((t - 42_506.546_140_046).abs() < 1e-6, "{t}");
        assert_eq!(to_naive(t).unwrap(), dt);
        assert!(is_first_of_january(from_naive(
            NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(23, 0, 0).unwrap()
        )));
        assert!(!is_first_of_january(0.0));
    }
}
