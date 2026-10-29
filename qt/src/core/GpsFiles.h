// GPS files through the Rust core: the open-dialog filter of the known log formats, and
// writing tracks in the export formats (the original's `gpsSavers`) with the user's regional
// settings, units, time shift and subtitle template.
// Spec: file-list.md FL-070…FL-073, graphs-map-layout.md GM-190…GM-192, playback-video.md PV-092.
#pragma once

#include "Group.h"

#include <QString>
#include <QStringList>
#include <QVector>

namespace gpsfiles {

// One export format: "kml", "Google Earth (.kml)", "kml".
struct Writer {
    QString id, caption, ext;
};

// The export formats in the original's order: KML, PLT, GPX, DatDOD, three CSV, SRT.
QVector<Writer> writers();
// Save-dialog filters of `list`, in the same order.
QStringList saveFilters(const QVector<Writer> &list);

// GetOpenerFilterString: all GPS formats together, then each extension, then any file. The
// dialog's default is the combined entry (FilterIndex = 2 in the original).
QStringList openFilters();

// A GPS log or video read whole, or only the part inside [from, to] (wall clock). A video's
// duration lets its subtitles run to its end.
struct Source {
    QString path;
    TDateTime from = 0, to = 0;
    double videoSeconds = 0;
};

// The display settings a written track depends on.
struct Settings {
    double tzShift = 0;    // user clock shift (days), in the track name
    bool british = false;  // miles, mph and feet in subtitles
};

struct Result {
    bool ok = false;
    QString error;
    bool hasPoints = false;
    QString name;      // "Track 2016.05.16 13-06-26"
    QString fileName;  // the name made safe for a file
};

// The sources as one track: only described when `writer` is empty, else written to `output`
// with the track's start and end as the file's times (WriteFileTime).
Result write(const QVector<Source> &sources, const QString &writer, const QString &output, const Settings &settings);

// The subtitle template window: the cue text of `templ` for one sample fix, and the default
// template.
QString srtPreview(const QString &templ, const Settings &settings, TDateTime time, double lat, double lng, double alt,
                   double speed, double bearing, double trackMetres);
QString srtDefaultTemplate(const Settings &settings);

}  // namespace gpsfiles
