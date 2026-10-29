// The item being played: one file or a group of consecutive files, positioned on one wall-clock
// timeline. Spec: playback-video.md §3.1 (PV-001…PV-004), file-list.md §4.10.
#pragma once

#include <QString>
#include <QVector>

#include <algorithm>
#include <cmath>

// Delphi TDateTime: days since 1899-12-30; values <= 2.0 mean "no date".
using TDateTime = double;
constexpr double kSecond = 1.0 / 86400.0;

struct GroupFile {
    QString path;
    QString face;
    TDateTime finish = 0;  // wall-clock end of the video
    double duration = 0;   // days
    double pStart = 0, pFinish = 1;
    int durationState = 3;  // 0 unknown, 1 estimated, 2 parser failed, 3 exact
    bool locked = false;
    int width = 0, height = 0;
    int gop = 0;
    double fps = 0;
    qint64 size = 0;         // bytes
    qint64 durationMs = 0;
    TDateTime fsCreation = 0, fsWrite = 0;
    QString infoPath;        // GPS found inside the video or in a same-name sidecar
    int gpsState = 0;        // 0 none, 1 points without a fix, 2 valid
    QString gpsLogPath;      // a trip log assigned by time
    TDateTime start() const { return finish - duration; }
};

struct Group {
    QString name;
    TDateTime finish = 0;
    double duration = 0;  // days, gaps included
    QVector<GroupFile> files;

    bool isEmpty() const { return files.isEmpty(); }
    TDateTime start() const { return finish - duration; }

    // PV-001: wall-clock time at progress p.
    TDateTime timeAt(double p) const { return files.isEmpty() ? 0 : finish + (p - 1) * duration; }
    // Progress of a wall-clock time.
    double progressAt(TDateTime t) const { return duration > 0 ? 1 + (t - finish) / duration : 0; }

    // PV-003: the file that owns p; a position in a gap belongs to the following file.
    int fileAt(double p) const {
        for (int i = 0; i < files.size(); ++i) {
            const bool last = i == files.size() - 1;
            if (p < files[i].pFinish || last) {
                if (files[i].pStart <= p || i == 0 || files[i - 1].pFinish <= p) return i;
            }
        }
        return 0;
    }
    // PV-002: local progress inside file i, and back.
    double localAt(int i, double p) const {
        const GroupFile &f = files[i];
        const double span = f.pFinish - f.pStart;
        return span > 0 ? (p - f.pStart) / span : 0;
    }
    double globalAt(int i, double local) const {
        const GroupFile &f = files[i];
        return f.pStart + local * (f.pFinish - f.pStart);
    }
    // PV-004: elapsed seconds from the group start.
    double elapsedSeconds(double p) const { return p * duration * 86400.0; }
};

// §4.7 step 5: progress fractions of each file inside the group span.
inline void computeProgress(Group &g) {
    if (g.files.isEmpty()) return;
    TDateTime s = g.files[0].start(), f = g.files[0].finish;
    for (const GroupFile &x : g.files) {
        s = std::min(s, x.start());
        f = std::max(f, x.finish);
    }
    g.finish = f;
    g.duration = f - s;
    const double G = g.duration > 0 ? g.duration : 1.0;
    auto clamp01 = [](double v) { return std::clamp(v, 0.0, 1.0); };
    for (GroupFile &x : g.files) {
        x.pFinish = 1 - clamp01((g.finish - x.finish) / G);
        x.pStart = x.pFinish - clamp01(x.duration / G);
    }
}
