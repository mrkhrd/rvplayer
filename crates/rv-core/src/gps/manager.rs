//! The load pipeline of `UnitGPSManager.TGpsInfoManager`: splitting data into records,
//! choosing the format per record, post-fixes and the BlackVue `.3gf` companion file.

use super::format::{Ctx, DataTest, Format, Info, Item, PT_GPS, PT_GSENSOR, RawItem};
use super::util::{SEC, be_i16, be_u32, median, split_lines};

/// Reads a companion file by extension: contents and last-write time (TDateTime).
pub type CompanionReader<'a> = &'a dyn Fn(&str) -> Option<(Vec<u8>, f64)>;

/// What the pipeline needs to know about the source besides its records.
pub struct Env<'a> {
    /// Path of the log or video.
    pub name: String,
    /// File-system (creation, last write) times as local TDateTime; (0, 0) when unknown.
    pub file_times: (f64, f64),
    /// Local time zone offset in days.
    pub tz: f64,
    /// Reads a companion file `<name up to the last '.'>.<ext>`: contents and last-write time.
    pub companion: CompanionReader<'a>,
}

/// The loaded points of one source (`TTrackInfo` after `LoadItems`).
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    pub points: Vec<Item>,
    pub info: Info,
    /// TDateTime of video time 0 when the records had video frame times.
    pub frame0_time: Option<f64>,
}

/// `SplitToItems`: the first format that splits the data itself, else text lines.
pub fn split_to_items(formats: &mut [Box<dyn Format>], data: &[u8]) -> (Vec<RawItem>, f64) {
    for f in formats.iter_mut() {
        if let Some(r) = f.split_raw_data(data) {
            return r;
        }
    }
    (split_lines(data).into_iter().map(RawItem::line).collect(), 0.0)
}

/// `LoadFile`: a whole log file.
pub fn load_file(formats: &mut [Box<dyn Format>], data: &[u8], env: &Env) -> Option<Loaded> {
    if data.is_empty() {
        return None;
    }
    let (items, frame0) = split_to_items(formats, data);
    load_items(formats, Some(data.to_vec()), items, frame0, "", 0.0, env)
}

/// `LoadItems`: records (with video frame times when they come from a video) to points.
/// `format_prefix` ("mov" for embedded data) is prepended to the format id.
pub fn load_items(
    formats: &mut [Box<dyn Format>],
    rawdata: Option<Vec<u8>>,
    mut items: Vec<RawItem>,
    frame0_time: f64,
    format_prefix: &str,
    duration: f64,
    env: &Env,
) -> Option<Loaded> {
    let mut info = Info { name: env.name.clone(), ..Default::default() };
    (info.date_time_start, info.date_time_finish) = env.file_times;
    if duration > SEC {
        info.date_time_start = info.date_time_finish - duration;
    }
    let max_t = items.iter().map(|i| i.frame_time).fold(0.0, f64::max);
    let mut frame0 = frame0_time;
    if max_t <= 0.0 {
        for i in &mut items {
            i.frame_time = -1.0;
        }
    } else if frame0 < 2.0 {
        frame0 = info.date_time_finish - max_t.max(duration);
    }
    let mut ctx = Ctx { items, duration, frame0_time: frame0, rawdata, info, tz: env.tz, ..Default::default() };
    load_items_cs(formats, &mut ctx);
    if !ctx.points.is_empty() {
        if !ctx.info.do_not_post_process_fix {
            fix_2_last_points(&mut ctx);
        }
        if max_t > 0.0 {
            shift_time_to_gps(&mut ctx);
        }
        if !ctx.info.format_id.is_empty() && !format_prefix.is_empty() {
            let mut id = format_prefix.to_string();
            let mut chars = ctx.info.format_id.chars();
            if let Some(c) = chars.next() {
                id.push(c.to_ascii_uppercase());
            }
            id.push_str(chars.as_str());
            ctx.info.format_id = id;
        }
        if formats.len() > 1 {
            load_3gf(&mut ctx, env);
        }
    }
    // CorrectGPS (GM-101, flip and shift settings of the viewer) is applied by the UI.
    let frame0_time = (max_t > 0.0).then_some(ctx.frame0_time);
    if ctx.points.is_empty() {
        return None;
    }
    Some(Loaded { points: ctx.points, info: ctx.info, frame0_time })
}

/// `LoadItemsCS`: picks the candidate formats, then offers every record to the format that
/// read the previous record first and to the others after it.
pub fn load_items_cs(formats: &mut [Box<dyn Format>], ctx: &mut Ctx) {
    let mut candidates: Vec<usize> = Vec::new();
    let mut prev: Option<usize> = None;
    match ctx.rawdata.take() {
        None => candidates = (0..formats.len()).collect(),
        Some(data) => {
            for (z, f) in formats.iter_mut().enumerate() {
                ctx.points.clear();
                match f.test_load_data(ctx, &data) {
                    DataTest::No => {}
                    DataTest::Maybe => candidates.push(z),
                    DataTest::Loaded => {
                        prev = Some(z);
                        candidates.clear();
                        break;
                    }
                }
            }
            ctx.rawdata = Some(data);
        }
    }
    for item in &mut ctx.items {
        item.gps_count = 0;
        item.gps_index = None;
    }
    ctx.last_gps_index = None;
    for &z in &candidates {
        formats[z].before_load_data(ctx);
    }
    if !candidates.is_empty() {
        let from_file = ctx.rawdata.is_some();
        let mut prev_line: Option<Vec<u8>> = None;
        for i in 0..ctx.items.len() {
            if ctx.points.is_empty() && ((i > 1000 && from_file) || (i > 99_000 && !from_file)) {
                break;
            }
            if ctx.items[i].line.len() <= 7 || prev_line.as_deref() == Some(ctx.items[i].line.as_slice()) {
                continue;
            }
            let original = ctx.items[i].line.clone();
            ctx.index = i;
            let len0 = ctx.points.len();
            // A format may rewrite the record (RoadHawk decodes it, Chelong trims it); like
            // the original, each call sees the record as it is now.
            let try_format = |f: &mut Box<dyn Format>, ctx: &mut Ctx| {
                let line = ctx.items[i].line.clone();
                if !f.test_load_item(ctx, &line) {
                    return false;
                }
                let line = ctx.items[i].line.clone();
                f.load_item(ctx, &line)
            };
            let mut done = false;
            if let Some(p) = prev {
                done = try_format(&mut formats[p], ctx);
            }
            if !done {
                for &z in &candidates {
                    if Some(z) != prev && try_format(&mut formats[z], ctx) {
                        prev = Some(z);
                        break;
                    }
                }
            }
            let len = ctx.points.len();
            let item = &mut ctx.items[i];
            item.gps_count = len - len0;
            item.gps_index =
                if item.gps_count == 1 { Some(len0) } else { (len0..len).find(|&k| ctx.points[k].pt == PT_GPS) };
            prev_line = Some(original);
        }
    }
    if let Some(p) = prev {
        ctx.info.format_id = formats[p].spec().id().to_string();
    }
    for &z in &candidates {
        formats[z].after_load_data(ctx);
    }
    if let Some(p) = prev {
        ctx.info.do_not_post_process_fix = !formats[p].do_post_process_fix();
    }
}

/// `Fix2LastPoints`: drops one or two trailing GPS fixes separated from the rest by a gap of
/// at least 11 s while being within 5 s of each other.
pub fn fix_2_last_points(ctx: &mut Ctx) {
    let gps: Vec<usize> = (0..ctx.points.len()).filter(|&i| ctx.points[i].pt == PT_GPS).collect();
    let [.., i1, i2, i3] = gps[..] else { return };
    if i1 == 0 {
        return;
    }
    const NEAR: f64 = 5.0 * SEC;
    const GAP: f64 = 11.0 * SEC;
    let t = |i: usize| ctx.points[i].time;
    if (t(i2) - t(i3)).abs() <= NEAR && (t(i1) - t(i2)).abs() >= GAP {
        ctx.points.truncate(i2);
    } else if (t(i2) - t(i3)).abs() >= GAP && (t(i1) - t(i2)).abs() <= NEAR {
        ctx.points.truncate(i3);
    }
}

/// `ShiftTimeToGPS`: for records with video frame times, the time of frame 0 becomes the
/// median of (GPS time - frame time) over the fixes (when there are any), and every record's
/// point gets frame 0 + its frame time. Fixes without a good fix are used only when there are no good
/// ones. Zero placeholders mark the start and the end of the video.
pub fn shift_time_to_gps(ctx: &mut Ctx) {
    let mut dt = Vec::new();
    for allow_warn in 0..3u8 {
        dt.clear();
        for it in &ctx.items {
            if let Some(g) = it.gps_index
                && it.frame_time >= 0.0
                && let Some(p) = ctx.points.get(g)
                && p.time > 2.0
                && p.warn <= allow_warn
            {
                dt.push(p.time - ctx.frame0_time - it.frame_time);
            }
        }
        if !dt.is_empty() {
            break;
        }
    }
    // Without any timed fix frame 0 keeps its estimate; the records are still placed by
    // their frame times.
    if !dt.is_empty() {
        ctx.frame0_time += median(&dt);
    }
    for it in &ctx.items {
        if let Some(g) = it.gps_index
            && it.frame_time >= 0.0
            && let Some(p) = ctx.points.get_mut(g)
        {
            p.time = ctx.frame0_time + it.frame_time;
        }
    }
    if ctx.duration > 0.0 {
        if ctx.items.last().is_some_and(|l| l.frame_time < ctx.duration) {
            ctx.points.push(Item { pt: PT_GPS, time: ctx.frame0_time + ctx.duration, ..Default::default() });
        }
        if ctx.items.first().is_some_and(|f| f.frame_time > 0.1 * SEC) {
            ctx.points.insert(0, Item { pt: PT_GPS, time: ctx.frame0_time, ..Default::default() });
        }
    }
}

/// `Load3GF`: BlackVue `.3gf` accelerometer file next to the source (10-byte big-endian
/// records: u32 ms, i16 x, y, z in 1/128 g). Replaces the G-sensor points; its last sample is
/// aligned with the last remaining point (or the `.3gf` write time).
pub fn load_3gf(ctx: &mut Ctx, env: &Env) {
    let Some((data, write_time)) = (env.companion)("3gf") else { return };
    let n = data.len() / 10;
    if n < 1 {
        return;
    }
    ctx.points.retain(|p| p.pt != PT_GSENSOR);
    let mut shift = ctx.points.last().map_or(write_time, |p| p.time);
    let len0 = ctx.points.len();
    let mut last_ms = 0u32;
    for r in data.chunks_exact(10) {
        let (Some(ms), Some(x), Some(y), Some(z)) = (be_u32(r, 0), be_i16(r, 4), be_i16(r, 6), be_i16(r, 8)) else {
            break;
        };
        last_ms = ms;
        let g = |v: i16| f64::from(v) / 128.0;
        ctx.points.push(Item::gsensor(f64::from(ms) / 86_400_000.0, g(x), g(y), g(z)));
    }
    shift -= f64::from(last_ms) / 86_400_000.0;
    for p in &mut ctx.points[len0..] {
        p.time += shift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_2_last_points_drops_a_late_pair() {
        let mut ctx = Ctx::default();
        for t in [0.0, 1.0, 2.0, 30.0, 31.0] {
            ctx.points.push(Item::gps(42_000.0 + t * SEC, 30.5, 60.5));
        }
        fix_2_last_points(&mut ctx);
        assert_eq!(ctx.points.len(), 3);
        let mut ctx = Ctx::default();
        for t in [0.0, 1.0, 2.0, 3.0, 40.0] {
            ctx.points.push(Item::gps(42_000.0 + t * SEC, 30.5, 60.5));
        }
        fix_2_last_points(&mut ctx);
        assert_eq!(ctx.points.len(), 4);
    }
}
