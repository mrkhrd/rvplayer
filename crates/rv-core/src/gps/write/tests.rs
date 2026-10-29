use super::*;
use crate::gps::test_support::{load_log, load_log_at};
use crate::gps::util::SEC;
use crate::track::{GSensorSample, GpsPoint};

fn gp(time: f64, lat: f64, lon: f64, altitude: f64, speed_kmh: f64, course: f64, warning: u8) -> GpsPoint {
    GpsPoint { time, lat, lon, altitude, speed_kmh, course, warning }
}

fn start() -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2016, 5, 16).unwrap().and_hms_opt(13, 6, 26).unwrap()
}

/// Fixes at 0..4 s (the one at 2 s without a good fix, one at 3.4 s, no altitude at 3 s), a
/// fix 10 minutes later, and G-sensor samples between the first fixes.
fn sample_track() -> Track {
    Track {
        start: Some(start()),
        gps: vec![
            gp(0.0, 60.063627, 30.327022, 12.5, 70.0, 112.0, 0),
            gp(1.0, 60.063527, 30.327522, 13.0, 71.0, 112.0, 0),
            gp(2.0, 60.063427, 30.328022, 13.0, 72.4, 113.0, 2),
            gp(3.0, 60.063327, 30.328522, 0.0, 73.5, 114.0, 0),
            gp(3.4, 60.0633, 30.3286, 14.0, 73.0, 115.0, 0),
            gp(4.0, 60.063227, 30.329022, 14.0, 74.0, 116.0, 0),
            gp(600.0, 60.163227, 30.429022, 20.0, 80.0, 30.0, 0),
        ],
        gsensor: vec![
            GSensorSample { time: 0.5, x: 0.01, y: -0.02, z: 0.98 },
            GSensorSample { time: 1.5, x: 0.02, y: -0.01, z: 0.97 },
            GSensorSample { time: 2.5, x: -0.015, y: 0.0, z: 1.02 },
        ],
    }
}

fn options() -> WriteOptions {
    WriteOptions { creator: "DATAKAM PLAYER 6.0.0.22".to_string(), ..WriteOptions::default() }
}

fn out(id: &str, info: &TrackInfo, opts: &WriteOptions) -> String {
    let mut v = Vec::new();
    write(id, info, opts, &mut v).unwrap();
    String::from_utf8(v).unwrap()
}

fn sample(id: &str) -> String {
    out(id, &track_info(&[&sample_track()], &options()), &options())
}

#[track_caller]
fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b} (±{tol})");
}

#[test]
fn formats_in_the_original_order() {
    let ids: Vec<&str> = formats().iter().map(|f| f.id).collect();
    assert_eq!(ids, ["kml", "plt", "gpx", "datdod", "csv", "csv_gps", "csv_gsensor", "srt"]);
    let exts: Vec<&str> = formats().iter().map(|f| f.ext).collect();
    assert_eq!(exts, ["kml", "plt", "gpx", "dat", "csv", "csv", "csv", "srt"]);
    for f in formats() {
        assert_eq!(f.caption, format!("{} (.{})", f.title, f.ext));
    }
    assert_eq!(formats()[3].filter(), "GS600, Q3 (.dat)|*.dat|");
    assert_eq!(formats()[6].caption, "Comma Separated Values: GSensor (.csv)");
    let kml = find("kml").unwrap();
    assert_eq!(kml.with_extension(Path::new("a/Track 1")), Path::new("a/Track 1.kml"));
    assert_eq!(kml.with_extension(Path::new("a/t.KML")), Path::new("a/t.KML"));
    assert!(find("xls").is_none());
    let err = write("xls", &TrackInfo::default(), &options(), &mut Vec::new()).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn track_name_and_file_name() {
    let mut opts = options();
    opts.user_shift = 2.0 / 24.0;
    let mut info = track_info(&[&sample_track()], &opts);
    assert_eq!(info.name, "Track 2016.05.16 15-06-26", "the user clock shift is in the name");
    assert_eq!(info.desc, "Duration: 0:10:00. Length: 120m. AvgSpeed: 0.7km/h.");
    info.name = "a:b*c?".to_string();
    assert_eq!(info.default_file_name(), "a-b_c_");
    assert!(!TrackInfo::default().has_points());
}

#[test]
fn kml_document() {
    let kml = sample("kml");
    let head = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<kml creator=\"DATAKAM PLAYER 6.0.0.22\" xmlns=\"http://www.opengis.net/kml/2.2\" xmlns:gx=\"http://www.google.com/kml/ext/2.2\">\r\n",
        "<Document id=\"RVTrackDoc\">\r\n",
        "  <name>Track 2016.05.16 13-06-26</name>\r\n",
        "  <description>Duration: 0:10:00. Length: 120m. AvgSpeed: 0.7km/h.</description>\r\n",
        "  <open>1</open>\r\n",
        "  <LookAt>\r\n",
        "    <longitude>30.378022</longitude>\r\n",
        "    <latitude>60.113227</latitude>\r\n",
        "    <range>13343</range>\r\n",
        "    <altitude>0</altitude>\r\n",
        "    <tilt>0</tilt>\r\n",
        "    <heading>0</heading>\r\n",
        "    <gx:TimeSpan>\r\n",
        "      <begin>2016-05-16T13:06:26Z</begin>\r\n",
        "      <end>2016-05-16T13:16:26Z</end>\r\n",
        "    </gx:TimeSpan>\r\n",
        "  </LookAt>\r\n",
        "<Style id=\"yellowLine\"><LineStyle><color>7f00ffff</color><width>3</width></LineStyle></Style>\r\n",
    );
    assert!(kml.starts_with(head), "{kml}");
    // the line: good fixes only, a new line string after the 10 minute gap
    let line = "<Placemark><name>Track</name><styleUrl>#yellowLine</styleUrl><MultiGeometry><LineString><coordinates>\r\n\
30.327022,60.063627,12.5\r\n\
30.327522,60.063527,13\r\n\
30.328522,60.063327,0\r\n\
30.328600,60.063300,14\r\n\
30.329022,60.063227,14\r\n\
</coordinates></LineString><LineString><coordinates>\r\n\
30.429022,60.163227,20\r\n\
</coordinates></LineString></MultiGeometry></Placemark>\r\n";
    assert!(kml.contains(line), "{kml}");
    let point = "<Placemark><Point><coordinates>30.328522,60.063327,0</coordinates></Point>\
<description><![CDATA[<nobr><b>( 3 ) Track 2016.05.16 13-06-26</b><br><br><font color=\"#666666\">\
( +00:00:02 = 00:00:03 ) - 16.05.2016 13:06:29</font></nobr><br><br>N60.063327, E30.328522<br>\
<br><b>Speed</b>: 73.5 km/h<br><b>Bearing</b>: 114\u{b0}<br><b>Track Length</b>: 90 m<br>]]></description></Placemark>\r\n";
    assert!(kml.contains(point), "{kml}");
    assert!(kml.contains("( 00:00:03.4 ) - 16.05.2016 13:06:29"));
    assert!(kml.contains("<b>Altitude</b>: 12.5 m<br>"));
    assert_eq!(kml.matches("<Placemark><Point>").count(), 6);
    assert!(kml.contains("<when>2016-05-16T13:16:26Z</when><gx:coord>30.429022 60.163227 20</gx:coord>\r\n"));
    assert_eq!(kml.matches("<when>").count(), 6);
    // the tour: the start, then the fixes far enough away
    let tour = "<gx:Tour><name>Tour</name><gx:Playlist>\r\n\
<gx:AnimatedUpdate><gx:duration>1.00</gx:duration><Update><targetHref></targetHref><Create><Document targetId=\"RVTrackDoc\">\
<Placemark id=\"Car\"><styleUrl>#multiTrack_n</styleUrl><Style><IconStyle><heading>112</heading></IconStyle></Style>\
<Point><coordinates>30.327022,60.063627,12.5</coordinates></Point></Placemark></Document></Create></Update></gx:AnimatedUpdate>\r\n\
<gx:FlyTo><gx:duration>2</gx:duration><gx:flyToMode>bounce</gx:flyToMode><LookAt><longitude>30.327022</longitude>\
<latitude>60.063627</latitude> <altitude>12.5</altitude><altitudeMode>relativeToGround</altitudeMode><heading>112</heading>\
<tilt>65</tilt><range>100</range></LookAt></gx:FlyTo>\r\n\
<gx:AnimatedUpdate><gx:duration>5.00</gx:duration><Update><targetHref></targetHref><Change><Placemark targetId=\"Car\">\
<Style><IconStyle><heading>30</heading></IconStyle></Style><Point><coordinates>30.429022,60.163227,20</coordinates></Point>\
</Placemark></Change></Update></gx:AnimatedUpdate>\r\n\
<gx:FlyTo><gx:duration>5.00</gx:duration><gx:flyToMode>smooth</gx:flyToMode><LookAt><longitude>30.429022</longitude>\
<latitude>60.163227</latitude> <altitude>20</altitude><altitudeMode>relativeToGround</altitudeMode><heading>30</heading>\
<tilt>27</tilt><range>143</range></LookAt></gx:FlyTo>\r\n\
</gx:Playlist></gx:Tour>\r\n</Document>\r\n</kml>\r\n";
    assert!(kml.ends_with(tour), "{kml}");
}

#[test]
fn kml_tour_splits_turns() {
    // two fixes 60 s apart turning by 135°: three 45° steps with interpolated positions
    let track = Track {
        start: Some(start()),
        gps: vec![gp(0.0, 60.0, 30.0, 0.0, 50.0, 10.0, 0), gp(60.0, 60.01, 30.0, 0.0, 50.0, 145.0, 0)],
        gsensor: vec![],
    };
    let kml = out("kml", &track_info(&[&track], &options()), &options());
    let headings: Vec<&str> = kml.match_indices("<IconStyle><heading>").map(|(i, _)| &kml[i + 20..i + 23]).collect();
    assert_eq!(headings, ["10<", "55<", "100", "145"]);
    assert!(kml.contains("<coordinates>30.000000,60.003333,0</coordinates>"));
    // 1112 m at 50 km/h is 80 s; a tenth of it is capped at 5 s and split in three
    assert_eq!(kml.matches("<gx:duration>1.67</gx:duration>").count(), 6);
}

#[test]
fn gpx_document() {
    let expected = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\r\n",
        "<gpx\r\n",
        " version=\"1.0\"\r\n",
        " creator=\"DATAKAM PLAYER 6.0.0.22\"\r\n",
        " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\r\n",
        " xmlns=\"http://www.topografix.com/GPX/1/0\"\r\n",
        " xsi:schemaLocation=\"http://www.topografix.com/GPX/1/0 http://www.topografix.com/GPX/1/0/gpx.xsd\">\r\n",
        "<time>2016-05-16T13:06:26Z</time>\r\n",
        "<bounds minlat=\"60.063227\" minlon=\"30.327022\" maxlat=\"60.163227\" maxlon=\"30.429022\"/>\r\n",
        "<trk>\r\n",
        "  <name>Track 2016.05.16 13-06-26</name>\r\n",
        "  <desc>Duration: 0:10:00. Length: 120m. AvgSpeed: 0.7km/h.</desc>\r\n",
        "  <time>2016-05-16T13:06:26Z</time>\r\n",
        "  <trkseg>\r\n",
        "\t<trkpt lat=\"60.063627\" lon=\"30.327022\"><time>2016-05-16T13:06:26Z</time><course>112</course><speed>19.44</speed><ele>12.5</ele></trkpt>\r\n",
        "\t<trkpt lat=\"60.063527\" lon=\"30.327522\"><time>2016-05-16T13:06:27Z</time><course>112</course><speed>19.72</speed><ele>13</ele></trkpt>\r\n",
        "\t<trkpt lat=\"60.063327\" lon=\"30.328522\"><time>2016-05-16T13:06:29Z</time><course>114</course><speed>20.41</speed></trkpt>\r\n",
        "\t<trkpt lat=\"60.063227\" lon=\"30.329022\"><time>2016-05-16T13:06:30Z</time><course>116</course><speed>20.55</speed><ele>14</ele></trkpt>\r\n",
        "\t<trkpt lat=\"60.163227\" lon=\"30.429022\"><time>2016-05-16T13:16:26Z</time><course>30</course><speed>22.22</speed><ele>20</ele></trkpt>\r\n",
        "  </trkseg>\r\n",
        "</trk>\r\n",
        "</gpx>\r\n",
    );
    assert_eq!(sample("gpx"), expected);
}

#[test]
fn plt_document() {
    let expected = "OziExplorer Track Point File Version 2.1\r\nWGS 84\r\nAltitude is in Feet\r\nReserved 3\r\n\
0,2,8323327,Track 2016.05.16 13-06-26,0,0,2,8421376\r\n\
6\r\n\
60.063627,30.327022,1,41,42506.5461343,16.05.2016,13:06:26,70,\r\n\
60.063527,30.327522,0,43,42506.5461458,16.05.2016,13:06:27,71,\r\n\
60.063327,30.328522,0,-777,42506.5461690,16.05.2016,13:06:29,74,\r\n\
60.063300,30.328600,0,46,42506.5461736,16.05.2016,13:06:29,73,\r\n\
60.063227,30.329022,0,46,42506.5461806,16.05.2016,13:06:30,74,\r\n\
60.163227,30.429022,1,66,42506.5530787,16.05.2016,13:16:26,80,\r\n";
    assert_eq!(sample("plt"), expected);
}

#[test]
fn datdod_document() {
    let expected = "[G]\t2016-05-16 13:06:26\tN60.063627\tE30.327022\t70\t112\r\n\
[S]\t10\t-20\t1980\r\n\
[G]\t2016-05-16 13:06:27\tN60.063527\tE30.327522\t71\t112\r\n\
[S]\t20\t-10\t1970\r\n\
[S]\t-15\t0\t2020\r\n\
[G]\t2016-05-16 13:06:29\tN60.063327\tE30.328522\t74\t114\r\n\
[G]\t2016-05-16 13:06:30\tN60.063227\tE30.329022\t74\t116\r\n\
[G]\t2016-05-16 13:16:26\tN60.163227\tE30.429022\t80\t30\r\n";
    assert_eq!(sample("datdod"), expected);
}

#[test]
fn csv_documents() {
    let both = "Type,Date,Time,ms,X,Y,H,Speed,Bearing,Trip\r\n\
[gps],16.05.2016,13:06:26,000,30.327022,60.063627,12.5,70,112,0,\r\n\
[gsensor],16.05.2016,13:06:26,500,0.010,-0.020,0.980,\r\n\
[gps],16.05.2016,13:06:27,000,30.327522,60.063527,13,71,112,30,\r\n\
[gsensor],16.05.2016,13:06:27,500,0.020,-0.010,0.970,\r\n\
[gps],16.05.2016,13:06:28,000,?30.328022,?60.063427,13,,,,\r\n\
[gsensor],16.05.2016,13:06:28,500,-0.015,0.000,1.020,\r\n\
[gps],16.05.2016,13:06:29,000,30.328522,60.063327,0,74,114,90,\r\n\
[gps],16.05.2016,13:06:29,400,30.328600,60.063300,14,73,115,95,\r\n\
[gps],16.05.2016,13:06:30,000,30.329022,60.063227,14,74,116,120,\r\n\
[gps],16.05.2016,13:16:26,000,30.429022,60.163227,20,80,30,120,\r\n";
    assert_eq!(sample("csv"), both);
    let gps = sample("csv_gps");
    assert!(gps.starts_with("Date,Time,ms,X,Y,H,Speed,Bearing,Trip\r\n16.05.2016,13:06:26,000,30.327022,60.063627,12.5,70,112,0,\r\n"));
    assert_eq!(gps.lines().count(), 8);
    let gs = "Date,Time,ms,X,Y,Z\r\n\
16.05.2016,13:06:26,500,0.010,-0.020,0.980,\r\n\
16.05.2016,13:06:27,500,0.020,-0.010,0.970,\r\n\
16.05.2016,13:06:28,500,-0.015,0.000,1.020,\r\n";
    assert_eq!(sample("csv_gsensor"), gs);
}

#[test]
fn csv_with_decimal_comma_and_other_formats() {
    let mut opts = options();
    opts.locale = Locale {
        decimal_separator: ',',
        time_separator: ':',
        date_format: "M/d/yyyy".to_string(),
        time_format: "h:mm:ss AP".to_string(),
    };
    let info = track_info(&[&sample_track()], &opts);
    assert_eq!(info.desc, "Duration: 0:10:00. Length: 120m. AvgSpeed: 0,7km/h.");
    let csv = out("csv", &info, &opts);
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "Type;Date;Time;ms;X;Y;H;Speed;Bearing;Trip");
    // the altitude keeps its '.', as in the original
    assert_eq!(lines[1], "[gps];5/16/2016;1:06:26 PM;000;30,327022;60,063627;12.5;70;112;0;");
    assert_eq!(lines[2], "[gsensor];5/16/2016;1:06:26 PM;500;0,010;-0,020;0,980;");
}

#[test]
fn time_zone_makes_utc_where_the_format_wants_it() {
    let mut opts = options();
    opts.tz = 3.0 / 24.0;
    let info = track_info(&[&sample_track()], &opts);
    assert!(out("gpx", &info, &opts).contains("<trkpt lat=\"60.063627\" lon=\"30.327022\"><time>2016-05-16T10:06:26Z</time>"));
    assert!(out("plt", &info, &opts).contains("60.063627,30.327022,1,41,42506.4211343,16.05.2016,10:06:26,70,"));
    let kml = out("kml", &info, &opts);
    assert!(kml.contains("<begin>2016-05-16T10:06:26Z</begin>"));
    assert!(kml.contains("<when>2016-05-16T10:06:26Z</when>"));
    assert!(kml.contains(") - 16.05.2016 13:06:26</font>"), "descriptions show local time");
    assert!(out("csv", &info, &opts).contains("[gps],16.05.2016,13:06:26,000,"));
    assert!(out("datdod", &info, &opts).starts_with("[G]\t2016-05-16 13:06:26\t"));
    assert!(out("srt", &info, &opts).contains("2016.05.16  13:06:26 \r\n"));
}

#[test]
fn srt_document() {
    let srt = sample("srt");
    let first = "\u{feff}1\r\n00:00:00,000 --> 00:00:00,999\r\n\
2016.05.16  13:06:26 \r\nN60.063627   E30.327022 \r\nTrack: 0m    Speed: 70km/h\r\n\r\n2\r\n";
    assert!(srt.starts_with(first), "{srt}");
    // a cue per fix (also the one without a good fix), not closer than 0.45 s; a cue lasts
    // until the next one; the last fix ends the data, so its cue has no length
    assert!(srt.contains("3\r\n00:00:02,000 --> 00:00:02,999\r\n2016.05.16  13:06:28 \r\nN60.063427   E30.328022 \r\nTrack: 30m    Speed: 72km/h\r\n"));
    assert!(srt.contains("5\r\n00:00:04,000 --> 00:09:59,999\r\n"));
    assert!(srt.ends_with("6\r\n00:10:00,000 --> 00:09:59,999\r\n2016.05.16  13:16:26 \r\nN60.163227   E30.429022 \r\nTrack: 120m    Speed: 80km/h\r\n\r\n"));
    assert_eq!(srt.matches(" --> ").count(), 6);
}

#[test]
fn srt_templates_and_units() {
    assert_eq!(
        default_srt_template(false, &Words::default()),
        "%y%.%m%.%d%  %h%:%mi%:%s% \r\n%lat%   %lng% \r\nTrack: %track%m    Speed: %speed%km/h"
    );
    assert_eq!(
        default_srt_template(true, &Words::default()),
        "%m%-%d%, %y%   %h%:%mi%:%s% \r\n%lat%   %lng% \r\nTrack: %track%miles    Speed: %speed%mph"
    );
    let ru = Words::from_translation(|id| if id == "Speed" { "Скорость".into() } else { id.into() });
    assert!(default_srt_template(false, &ru).ends_with("Скорость: %speed%KmetersPerH"));

    let mut opts = options();
    opts.imperial = true;
    let info = track_info(&[&sample_track()], &opts);
    let srt = out("srt", &info, &opts);
    assert!(srt.contains("5-16, 2016   13:06:30 \r\nN60.063227   E30.329022 \r\nTrack: 0.074miles    Speed: 46mph\r\n"), "{srt}");

    // a short template is the default one; placeholders are case-insensitive
    opts.imperial = false;
    opts.srt_template = Some("x".to_string());
    assert_eq!(out("srt", &info, &opts), sample("srt"));
    opts.srt_template = Some("%DATE% %Time% %alt%m %bearing%\u{b0} %Lat%/%LNG% %mi%%S% «%y%»".to_string());
    let srt = out("srt", &info, &opts);
    assert!(srt.contains("\r\n16.05.2016 13:06:26 12m 112\u{b0} N60.063627/E30.327022 0626 «2016»\r\n"), "{srt}");
}

#[test]
fn merged_tracks_follow_each_other() {
    let a = sample_track();
    // an earlier recording appended after the later one is still written (a new day for the
    // one-fix-per-second rule)
    let b = Track { start: Some(start() - chrono::TimeDelta::hours(1)), ..sample_track() };
    let info = track_info(&[&a, &b], &options());
    assert_eq!(info.points.len(), 20);
    assert_eq!(info.name, "Track 2016.05.16 12-06-26");
    assert_eq!(round_m(info.track_len), 239);
    let gpx = out("gpx", &info, &options());
    assert_eq!(gpx.matches("<trkpt").count(), 10);
    let plt = out("plt", &info, &options());
    assert!(plt.contains("\r\n12\r\n"));
    assert!(plt.contains("60.063627,30.327022,1,41,42506.5044676,16.05.2016,12:06:26,70,"), "the second track starts a segment");
}

fn round_m(v: f64) -> i64 {
    v.round() as i64
}

#[test]
fn video_duration_spans_the_source() {
    let track = sample_track();
    let mut info = TrackInfo::default();
    info.add(&track, Some(900.0), &options());
    assert_eq!(info.desc, "Duration: 0:15:00. Length: 120m. AvgSpeed: 0.5km/h.");
    // the last cue now lasts 1 s
    assert!(out("srt", &info, &options()).contains("6\r\n00:10:00,000 --> 00:10:00,999\r\n"));
}

#[test]
fn empty_info() {
    let info = TrackInfo::default();
    for f in formats() {
        let s = out(f.id, &info, &options());
        assert!(!s.is_empty() || f.id == "datdod", "{}", f.id);
    }
    assert!(out("gpx", &info, &options()).contains("<bounds minlat=\"1000.000000\" minlon=\"1000.000000\""));
    assert_eq!(out("srt", &info, &options()), "\u{feff}");
}

// In the round trips the loader drops the lone last fix after the 10 minute gap
// (`Fix2LastPoints`), so it is not compared.

#[test]
fn round_trip_gpx() {
    let t = load_log("a.gpx", sample("gpx").as_bytes()).unwrap().track;
    assert_eq!(t.start, Some(start()));
    let src = sample_track();
    let expect = [0, 1, 3, 5];
    assert_eq!(t.gps.len(), expect.len());
    for (p, &k) in t.gps.iter().zip(&expect) {
        let s = &src.gps[k];
        near(p.time, s.time, 1e-6);
        near(p.lat, s.lat, 5e-7);
        near(p.lon, s.lon, 5e-7);
        near(p.speed_kmh, s.speed_kmh, 0.01 * 3.6);
        near(p.course, s.course.round(), 0.0);
        if k != 3 {
            near(p.altitude, s.altitude, 0.05);
        }
    }
}

#[test]
fn round_trip_kml() {
    // the reader takes the first line string and spreads it over the file times
    let t0 = 42_506.5;
    let t = load_log_at("a.kml", sample("kml").as_bytes(), (t0, t0 + 4.0 * SEC)).unwrap().track;
    let src = sample_track();
    let expect = [0, 1, 3, 4, 5];
    assert_eq!(t.gps.len(), expect.len());
    for (p, &k) in t.gps.iter().zip(&expect) {
        near(p.lat, src.gps[k].lat, 5e-7);
        near(p.lon, src.gps[k].lon, 5e-7);
        near(p.altitude, src.gps[k].altitude, 0.05);
    }
}

#[test]
fn round_trip_plt() {
    let t = load_log("a.plt", sample("plt").as_bytes()).unwrap().track;
    // 7 decimals of a day are 8.64 ms
    assert!((t.start.unwrap() - start()).num_milliseconds().abs() <= 5);
    let src = sample_track();
    let expect = [0, 1, 3, 4, 5];
    assert_eq!(t.gps.len(), expect.len());
    for (p, &k) in t.gps.iter().zip(&expect) {
        let s = &src.gps[k];
        near(p.time, s.time, 0.01);
        near(p.lat, s.lat, 5e-7);
        near(p.lon, s.lon, 5e-7);
        if k != 3 {
            near(p.altitude, s.altitude, 0.3048 / 2.0);
        }
    }
}

#[test]
fn round_trip_csv() {
    let src = sample_track();
    for id in ["csv", "csv_gps"] {
        let t = load_log("a.csv", sample(id).as_bytes()).unwrap().track;
        assert_eq!(t.start, Some(start()), "{id}");
        assert_eq!(t.gps.len(), 6, "{id}");
        for (p, s) in t.gps.iter().zip(&src.gps) {
            near(p.time, s.time, 1e-6);
            near(p.lat, s.lat, 5e-7);
            near(p.lon, s.lon, 5e-7);
            near(p.altitude, s.altitude, 0.05);
            assert_eq!(p.warning, s.warning);
            if s.warning == 0 {
                near(p.speed_kmh, s.speed_kmh.round_ties_even(), 0.0);
                near(p.course, s.course, 0.0);
            }
        }
    }
    for id in ["csv", "csv_gsensor"] {
        let t = load_log("a.csv", sample(id).as_bytes()).unwrap().track;
        assert_eq!(t.gsensor.len(), 3, "{id}");
        let t0 = if id == "csv" { 0.0 } else { 0.5 };
        for (g, s) in t.gsensor.iter().zip(&src.gsensor) {
            near(g.time, s.time - t0, 1e-6);
            near(g.x, s.x, 5e-4);
            near(g.y, s.y, 5e-4);
            near(g.z, s.z, 5e-4);
        }
    }
}

#[test]
fn round_trip_datdod() {
    let t = load_log("a.dat", sample("datdod").as_bytes()).unwrap().track;
    assert_eq!(t.start, Some(start()));
    let src = sample_track();
    let expect = [0, 1, 3, 5];
    assert_eq!(t.gps.len(), expect.len());
    for (p, &k) in t.gps.iter().zip(&expect) {
        let s = &src.gps[k];
        near(p.time, s.time.floor(), 1e-6);
        near(p.lat, s.lat, 5e-7);
        near(p.lon, s.lon, 5e-7);
        near(p.speed_kmh, s.speed_kmh.round_ties_even(), 0.0);
        near(p.course, s.course, 0.0);
    }
    assert_eq!(t.gsensor.len(), 3);
    for (g, s) in t.gsensor.iter().zip(&src.gsensor) {
        near(g.x, s.x, 5e-4);
        near(g.y, s.y, 5e-4);
        near(g.z, s.z + 1.0, 5e-4);
    }
}

#[test]
fn round_trip_srt() {
    // the reader drops the cue at 00:00:00 and takes the camera clock from the cue text
    let t = load_log("a.srt", sample("srt").as_bytes()).unwrap().track;
    let src = sample_track();
    let expect = [1, 2, 3, 5];
    assert_eq!(t.gps.len(), expect.len(), "{t:?}");
    assert_eq!(t.start, Some(start()), "cue 00:00:00 is the first fix's time");
    for (p, &k) in t.gps.iter().zip(&expect) {
        let s = &src.gps[k];
        near(p.time, s.time, 1e-6);
        near(p.lat, s.lat, 5e-7);
        near(p.lon, s.lon, 5e-7);
    }
}

#[test]
fn write_file_sets_times() {
    let mut opts = options();
    opts.tz = 3.0 / 24.0;
    let info = track_info(&[&sample_track()], &opts);
    let path = std::env::temp_dir().join(format!("rv-core-write-{}.gpx", std::process::id()));
    write_file("gpx", &info, &opts, &path).unwrap();
    let meta = std::fs::metadata(&path).unwrap();
    let data = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(data, out("gpx", &info, &opts));
    // finish 13:16:26 local = 10:16:26 UTC
    let utc = |h, m, s| chrono::NaiveDate::from_ymd_opt(2016, 5, 16).unwrap().and_hms_opt(h, m, s).unwrap().and_utc();
    let modified = chrono::DateTime::<chrono::Utc>::from(meta.modified().unwrap());
    assert!((modified - utc(10, 16, 26)).num_milliseconds().abs() < 2, "{modified}");
    if cfg!(windows) {
        let created = chrono::DateTime::<chrono::Utc>::from(meta.created().unwrap());
        assert!((created - utc(10, 6, 26)).num_milliseconds().abs() < 2, "{created}");
    }
}
