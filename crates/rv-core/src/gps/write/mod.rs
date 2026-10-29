//! GPS track writers: "Export Track" (FL-071), "Open Track in Google Earth" (FL-072) and the GPS
//! log converter (GM-192). Ported from the `SaveFileV` methods of `UnitGps_KML`, `UnitGps_PLT`,
//! `UnitGps_GPX`, `UnitGps_DatDOD`, `UnitGps_CSV` and `UnitGps_SRT`, the common wrapper
//! `IGpsInfoFormat.SaveFile` and the subtitle helpers of `UnitSubtitles`.
//!
//! Use: build a [`TrackInfo`] from one or more tracks ([`track_info`], or [`TrackInfo::add`]
//! per source; merging = sources appended one after another, each source's GPS and G-sensor
//! records merged by time), then [`write`] or [`write_file`] it with an id from [`formats`].
//!
//! Output is UTF-8 with CR LF line ends (the original's texts were ASCII apart from translated
//! words and the degree sign, which it wrote in UTF-8). Times in the data are local wall-clock
//! times; GPX, PLT and KML times are written in UTC (local − [`WriteOptions::tz`]; the original
//! left KML times local, see `kml`). What depended on Windows regional settings in the original
//! (decimal separator, date and time formats) comes from [`Locale`].
//!
//! Records written: KML, GPX, PLT and DatDOD write only good fixes (warning 0) — DatDOD also the
//! G-sensor samples; the CSV writers write every fix, marking the others with "?", and/or the
//! G-sensor samples; SRT makes a cue of every fix. GPX and DatDOD keep one fix per second
//! (`isAllowFracSeconds` = false).

mod csv;
mod datdod;
mod gpx;
mod info;
mod kml;
mod plt;
mod srt;
mod text;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub use info::{InfoPoint, TrackInfo};

use crate::gps::format::PT_GPS;
use crate::track::Track;

/// A writer as the save dialogs list it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriterSpec {
    /// Id for [`write`].
    pub id: &'static str,
    /// The original's format title.
    pub title: &'static str,
    /// `SaveFilterCaption`: "<title> (.<ext>)".
    pub caption: &'static str,
    /// Default extension, without the dot.
    pub ext: &'static str,
}

impl WriterSpec {
    /// `SaveFilterString`: "<caption>|*.<ext>|" (a Windows filter pair).
    pub fn filter(&self) -> String {
        format!("{}|*.{}|", self.caption, self.ext)
    }

    /// FL-071: the path with "." + `ext` appended unless it already ends with it (any case).
    pub fn with_extension(&self, path: &Path) -> PathBuf {
        let dotted = format!(".{}", self.ext);
        let s = path.as_os_str().to_string_lossy();
        if s.len() >= dotted.len() && s[s.len() - dotted.len()..].eq_ignore_ascii_case(&dotted) {
            path.to_path_buf()
        } else {
            let mut p = path.as_os_str().to_os_string();
            p.push(&dotted);
            PathBuf::from(p)
        }
    }
}

const fn spec(id: &'static str, title: &'static str, caption: &'static str, ext: &'static str) -> WriterSpec {
    WriterSpec { id, title, caption, ext }
}

/// `gpsSavers` in the original's order (UnitGPSManager initialization).
static FORMATS: [WriterSpec; 8] = [
    spec("kml", "Google Earth", "Google Earth (.kml)", "kml"),
    spec("plt", "Ozi Explorer", "Ozi Explorer (.plt)", "plt"),
    spec("gpx", "GPS eXchange Format", "GPS eXchange Format (.gpx)", "gpx"),
    spec("datdod", "GS600, Q3", "GS600, Q3 (.dat)", "dat"),
    spec(
        "csv",
        "Comma Separated Values: GPS and GSensor",
        "Comma Separated Values: GPS and GSensor (.csv)",
        "csv",
    ),
    spec("csv_gps", "Comma Separated Values: GPS", "Comma Separated Values: GPS (.csv)", "csv"),
    spec("csv_gsensor", "Comma Separated Values: GSensor", "Comma Separated Values: GSensor (.csv)", "csv"),
    spec("srt", "SubRip Subtitles", "SubRip Subtitles (.srt)", "srt"),
];

/// The writers in the original's export order: KML, PLT, GPX, DatDOD, the three CSV variants,
/// SRT.
pub fn formats() -> &'static [WriterSpec] {
    &FORMATS
}

/// The writer with this id.
pub fn find(id: &str) -> Option<&'static WriterSpec> {
    FORMATS.iter().find(|f| f.id == id)
}

/// The Windows regional settings the original's texts depended on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    /// `DecimalSeparator`: used by CSV numbers, where ',' also makes the column separator ';'
    /// (else ','), and by the average speed in [`TrackInfo::desc`].
    pub decimal_separator: char,
    /// `TimeSeparator` of the duration in [`TrackInfo::desc`].
    pub time_separator: char,
    /// `ShortDateFormat` (`DateToStr`) as a Qt-style pattern: `d dd M MM yy yyyy`, text in
    /// single quotes, anything else literal.
    pub date_format: String,
    /// `LongTimeFormat` (`TimeToStr`) as a Qt-style pattern: `h hh` (12-hour when the pattern
    /// has `AP`/`ap`), `H HH m mm s ss z zzz AP ap`.
    pub time_format: String,
}

impl Default for Locale {
    /// '.' decimals with 24-hour day-first dates: "16.05.2016", "13:06:26".
    fn default() -> Locale {
        Locale {
            decimal_separator: '.',
            time_separator: ':',
            date_format: "dd.MM.yyyy".to_string(),
            time_format: "HH:mm:ss".to_string(),
        }
    }
}

/// Translated words (`TranslateW`) in KML point descriptions and the default SRT template.
/// The original's translation ids are given per field; [`Words::from_translation`] fills them
/// from a lookup by those ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Words {
    /// "Altitude".
    pub altitude: String,
    /// "Meters": "m".
    pub meters: String,
    /// "Speed".
    pub speed: String,
    /// "KmetersPerH": "km/h".
    pub km_per_h: String,
    /// "Azimut": "Bearing".
    pub bearing: String,
    /// "Property_Track_Length": "Track Length".
    pub track_length: String,
    /// "Details_Track_Length": "Track".
    pub track: String,
    /// "Miles": "miles".
    pub miles: String,
    /// "MilesPerH": "mph".
    pub miles_per_h: String,
}

impl Default for Words {
    /// The original's English translation.
    fn default() -> Words {
        Words::from_translation(|id| {
            match id {
                "Altitude" => "Altitude",
                "Meters" => "m",
                "Speed" => "Speed",
                "KmetersPerH" => "km/h",
                "Azimut" => "Bearing",
                "Property_Track_Length" => "Track Length",
                "Details_Track_Length" => "Track",
                "Miles" => "miles",
                _ => "mph",
            }
            .to_string()
        })
    }
}

impl Words {
    /// The words from a lookup by the original's translation ids (see the fields).
    pub fn from_translation(tr: impl Fn(&str) -> String) -> Words {
        Words {
            altitude: tr("Altitude"),
            meters: tr("Meters"),
            speed: tr("Speed"),
            km_per_h: tr("KmetersPerH"),
            bearing: tr("Azimut"),
            track_length: tr("Property_Track_Length"),
            track: tr("Details_Track_Length"),
            miles: tr("Miles"),
            miles_per_h: tr("MilesPerH"),
        }
    }
}

/// Settings of a write.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteOptions {
    /// Local time-zone offset in days (`timeZoneLocalSystem`): UTC = local − `tz` in GPX, PLT
    /// and KML times and the file times of [`write_file`].
    pub tz: f64,
    /// User clock shift in days (`timeZoneUserShift`); only the track name uses it.
    pub user_shift: f64,
    pub locale: Locale,
    /// The original's "miles" setting: SRT speed in mph, track in miles, altitude in feet, and
    /// the imperial default template.
    pub imperial: bool,
    /// SRT cue template (the "subtitleTemplate" setting); `None` or shorter than 2 characters =
    /// [`default_srt_template`]. Placeholders, case-insensitive: `%y% %m% %d% %h% %mi% %s%`
    /// (date and time parts), `%date% %time%` (locale texts), `%lat% %lng%` ("N60.063627"),
    /// `%bearing%`, `%speed%`, `%track%` (distance from the source's first fix), `%alt%`.
    pub srt_template: Option<String>,
    pub words: Words,
    /// `creator` attribute of KML and GPX; the original wrote "DATAKAM PLAYER <its version>".
    pub creator: String,
}

impl Default for WriteOptions {
    /// UTC as local time, no user shift, [`Locale::default`], metric, English.
    fn default() -> WriteOptions {
        WriteOptions {
            tz: 0.0,
            user_shift: 0.0,
            locale: Locale::default(),
            imperial: false,
            srt_template: None,
            words: Words::default(),
            creator: concat!("DATAKAM PLAYER ", env!("CARGO_PKG_VERSION")).to_string(),
        }
    }
}

impl WriteOptions {
    /// [`WriteOptions::default`] with the computer's current time-zone offset.
    pub fn local() -> WriteOptions {
        WriteOptions { tz: crate::gps::local_tz_offset_days(), ..WriteOptions::default() }
    }
}

/// `SubtitleGetTemplateUTF8Def`: the default SRT cue text, e.g.
/// "2016.05.16  13:06:26 \r\nN60.063627   E30.327022 \r\nTrack: 125m    Speed: 70km/h".
pub fn default_srt_template(imperial: bool, words: &Words) -> String {
    if imperial {
        format!(
            "%m%-%d%, %y%   %h%:%mi%:%s% \r\n%lat%   %lng% \r\n{}: %track%{}    {}: %speed%{}",
            words.track, words.miles, words.speed, words.miles_per_h
        )
    } else {
        format!(
            "%y%.%m%.%d%  %h%:%mi%:%s% \r\n%lat%   %lng% \r\n{}: %track%{}    {}: %speed%{}",
            words.track, words.meters, words.speed, words.km_per_h
        )
    }
}

/// The export of the given tracks, one after another (FL-070 / GM-192 merge). Each source spans
/// its records; use [`TrackInfo::add`] to give a video's duration.
pub fn track_info(tracks: &[&Track], opts: &WriteOptions) -> TrackInfo {
    let mut info = TrackInfo::default();
    for t in tracks {
        info.add(t, None, opts);
    }
    info
}

impl TrackInfo {
    /// FL-071 default file name: the name with ':' as '-' and the characters Windows does not
    /// allow in names (`"*<>?|`) as '_' (`filterFileName`).
    pub fn default_file_name(&self) -> String {
        self.name
            .chars()
            .map(|c| match c {
                ':' => '-',
                '"' | '*' | '<' | '>' | '?' | '|' => '_',
                c => c,
            })
            .collect()
    }
}

/// The file contents of a writer (`IGpsInfoFormat.SaveFile`).
pub fn render(id: &str, info: &TrackInfo, opts: &WriteOptions) -> io::Result<String> {
    let unknown = || io::Error::new(io::ErrorKind::InvalidInput, format!("unknown GPS writer {id:?}"));
    let spec = find(id).ok_or_else(unknown)?;
    Ok(match spec.id {
        "kml" => kml::write(info, opts),
        "plt" => plt::write(info, opts),
        "gpx" => gpx::write(&one_fix_per_second(info), opts),
        "datdod" => datdod::write(&one_fix_per_second(info)),
        "csv" => csv::write(info, opts, true, true),
        "csv_gps" => csv::write(info, opts, true, false),
        "csv_gsensor" => csv::write(info, opts, false, true),
        _ => srt::write(info, opts),
    })
}

/// Writes `info` in the format `id` (see [`formats`]).
pub fn write(id: &str, info: &TrackInfo, opts: &WriteOptions, out: &mut impl Write) -> io::Result<()> {
    out.write_all(render(id, info, opts)?.as_bytes())
}

/// Writes `info` to `path` (as given: see [`WriterSpec::with_extension`]) and sets the file
/// times like `WriteFileTime`: created = [`TrackInfo::start`], modified and accessed =
/// [`TrackInfo::finish`], as UTC (local − [`WriteOptions::tz`]; the creation time on Windows
/// only). Times are left alone when the info has none.
pub fn write_file(id: &str, info: &TrackInfo, opts: &WriteOptions, path: &Path) -> io::Result<()> {
    let data = render(id, info, opts)?;
    std::fs::write(path, data)?;
    if let (Some(created), Some(written)) = (system_time(info.start, opts.tz), system_time(info.finish, opts.tz)) {
        set_file_times(path, created, written)?;
    }
    Ok(())
}

/// A local TDateTime as a system time; `None` for no time (0) or one out of range.
fn system_time(local: f64, tz: f64) -> Option<std::time::SystemTime> {
    if local <= 0.0 {
        return None;
    }
    let secs = (local - tz - 25_569.0) * 86_400.0;
    let d = std::time::Duration::try_from_secs_f64(secs.abs()).ok()?;
    if secs >= 0.0 { std::time::UNIX_EPOCH.checked_add(d) } else { std::time::UNIX_EPOCH.checked_sub(d) }
}

/// `WriteFileTime(fn, created, written)`.
fn set_file_times(path: &Path, created: std::time::SystemTime, written: std::time::SystemTime) -> io::Result<()> {
    let file = std::fs::File::options().write(true).open(path)?;
    let times = std::fs::FileTimes::new().set_accessed(written).set_modified(written);
    #[cfg(windows)]
    let times = {
        use std::os::windows::fs::FileTimesExt;
        times.set_created(created)
    };
    #[cfg(not(windows))]
    let _ = created;
    file.set_times(times)
}

/// `IGpsInfoFormat.SaveFile` for formats without fractional seconds (GPX, DatDOD): a fix is
/// dropped when its second of the day is not after the previous kept fix's, unless it is more
/// than 60 s before it (a new day). G-sensor records are kept.
///
/// The second is taken from the time rounded to the millisecond, as the writers print it; the
/// original truncated the raw value, so float noise just below a whole second could drop a fix
/// that prints a later second.
fn one_fix_per_second(info: &TrackInfo) -> TrackInfo {
    let mut prev: i64 = -1;
    let mut points = Vec::with_capacity(info.points.len());
    for p in &info.points {
        let t = if p.item.pt == PT_GPS {
            let d = text::decode(p.item.time);
            i64::from(d.h * 3600 + d.mi * 60 + d.s)
        } else {
            prev
        };
        if prev < t || t < prev - 60 || p.item.pt != PT_GPS {
            prev = t;
            points.push(*p);
        }
    }
    TrackInfo { points, ..info.clone() }
}

/// Escapes `&`, `<` and `>` for XML text. The original inserted names and descriptions as they
/// were; its generated ones have none of these characters, so the output is the same for them.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The good fixes (`pt = PT_GPS`, `warn = 0`) with their index into `info.points`.
fn good_fixes(info: &TrackInfo) -> impl Iterator<Item = (usize, &InfoPoint)> {
    info.points.iter().enumerate().filter(|(_, p)| p.item.pt == PT_GPS && p.item.warn == 0)
}

#[cfg(test)]
mod tests;
