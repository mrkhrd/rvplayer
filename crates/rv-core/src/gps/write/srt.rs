//! `TGpsFormat_SRT.SaveFileV` with `UnitSubtitles`: SubRip subtitles, a cue per GPS fix (good
//! or not), UTF-8 with a byte-order mark.
//!
//! Cue timing (`SubtitlesConvert` with video start 0 and speed 1): a cue starts at the fix's
//! time minus [`TrackInfo::start`], is skipped when it starts less than 0.45 s after the
//! previous cue, and lasts until the next cue starts, 1 s for the last one; start and end are
//! kept within 0..`finish − start`, and the end is written 1 ms early. So the last cue of a
//! source that ends with its last fix (a log) has no length and its end is written before its
//! start, as in the original; a video source given its duration ends the cue 1 s later. The
//! text is the template with its placeholders expanded (`SubtitleExpandTemplate`).

use std::fmt::Write as _;

use super::info::{InfoPoint, TrackInfo};
use super::text::{MS, coord, decode, int2, int_n, po, round, srt_time};
use super::{WriteOptions, default_srt_template};
use crate::gps::format::PT_GPS;
use crate::gps::util::SEC;

const KM_PER_MILE: f64 = 1.609344;
const M_PER_FOOT: f64 = 0.3048;

struct Cue {
    text: String,
    start: f64,
    end: f64,
}

pub(super) fn write(info: &TrackInfo, o: &WriteOptions) -> String {
    let template = match &o.srt_template {
        Some(t) if t.len() >= 2 => t.clone(),
        _ => default_srt_template(o.imperial, &o.words),
    };
    let max_finish = info.finish - info.start;
    let mut cues: Vec<Cue> = Vec::new();
    let mut prev: Option<f64> = None;
    for p in info.points.iter().filter(|p| p.item.pt == PT_GPS) {
        let t = p.item.time;
        if prev.is_some_and(|pt| t - pt < 0.45 * SEC) {
            continue;
        }
        let start = (t - info.start).min(max_finish).max(0.0);
        let end = (start + SEC).min(max_finish).max(0.0);
        if let Some(last) = cues.last_mut() {
            last.end = start;
        }
        cues.push(Cue { text: expand(&template, p, o), start, end });
        prev = Some(t);
    }
    let mut s = String::with_capacity(16 + cues.len() * (template.len() + 64));
    s.push('\u{feff}');
    for (n, c) in cues.iter().enumerate() {
        let _ = write!(s, "{}\r\n{} --> {}\r\n{}\r\n\r\n", n + 1, srt_time(c.start), srt_time(c.end - MS), c.text);
    }
    s
}

/// `SubtitleExpandTemplate`.
fn expand(template: &str, p: &InfoPoint, o: &WriteOptions) -> String {
    let it = &p.item;
    let d = decode(it.time);
    let mut s = template.to_string();
    let month = if o.imperial { d.mo.to_string() } else { int2(d.mo.into()) };
    let lat = format!("{}{}", if it.y < 0.0 { 'S' } else { 'N' }, coord(it.y.abs()));
    let lng = format!("{}{}", if it.x < 0.0 { 'W' } else { 'E' }, coord(it.x.abs()));
    let (speed, track, alt) = if o.imperial {
        (
            round(it.speed / KM_PER_MILE).to_string(),
            po(p.track_pos / 1000.0 / KM_PER_MILE, 3, true),
            round(it.z / M_PER_FOOT).to_string(),
        )
    } else {
        (round(it.speed).to_string(), round(p.track_pos).to_string(), round(it.z).to_string())
    };
    let values = [
        ("%y%", int_n(d.y.into(), 4)),
        ("%m%", month),
        ("%d%", int2(d.d.into())),
        ("%h%", int2(d.h.into())),
        ("%mi%", int2(d.mi.into())),
        ("%s%", int2(d.s.into())),
        ("%date%", o.locale.date(it.time)),
        ("%time%", o.locale.time(it.time)),
        ("%lat%", lat),
        ("%lng%", lng),
        ("%bearing%", round(it.angle).to_string()),
        ("%speed%", speed),
        ("%track%", track),
        ("%alt%", alt),
    ];
    for (key, value) in values {
        s = replace_ignore_case(&s, key, &value);
    }
    s
}

/// `StringReplace(s, key, value, [rfReplaceAll, rfIgnoreCase])` for an ASCII key.
fn replace_ignore_case(s: &str, key: &str, value: &str) -> String {
    let (hay, key_b) = (s.as_bytes(), key.as_bytes());
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut copied = 0;
    while i + key_b.len() <= hay.len() {
        if hay[i..i + key_b.len()].eq_ignore_ascii_case(key_b) {
            out.push_str(&s[copied..i]);
            out.push_str(value);
            i += key_b.len();
            copied = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&s[copied..]);
    out
}
