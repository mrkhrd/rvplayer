// GPS and G-sensor data of the current group on the wall-clock timeline (TMovingInfo).
// Spec: graphs-map-layout.md GM-034 (graph columns), GM-103 (G-sensor values), GM-104 (track
// distance), GM-105 (value at an instant). The tracks come from the Rust core per file.
#pragma once

#include "Group.h"
#include "draw/Graphics.h"

#include <QVector>

struct GpsSample {
    TDateTime t = 0;
    double lat = 0, lon = 0;
    double alt = 0;     // m, 0 = unknown
    double speed = 0;   // km/h
    double course = -1; // degrees clockwise from north, -1 = unknown
    double track = 0;   // cumulative metres
    bool warn = false;
};

struct GSample {
    TDateTime t = 0;
    double x = 0, y = 0, z = 0;  // dynamic acceleration, g (median removed)
    double v = 0;                // total vector
    double g = 0;                // horizontal change to the next sample
};

// Values at one instant; `gps`/`gsensor` false when there is no data there.
struct MovingValue {
    bool gps = false;
    bool gpsGood = false;
    int gpsIndex = 0, gIndex = 0;  // index of the sample at or before the instant, over the group
    double lat = 0, lon = 0, alt = 0, speed = 0, course = -1, track = 0;
    bool gsensor = false;
    double x = 0, y = 0, z = 0;
};

class MovingInfo {
public:
    // GM-101 and GM-104: corrections applied while loading.
    struct Options {
        bool flipX = false, flipY = false;  // negate longitude / latitude
        double shiftX = 0, shiftY = 0;      // degrees added afterwards
        bool speedFromCoordinates = false;  // ignore the recorded speed and bearing
    };

    // Loads the tracks of every file of the group (rv_file_info_json).
    void load(const Group &group, const Options &options = {});
    // The whole track of one GPS log or video, uncropped (the GPS converter's TMovingInfo).
    void loadSource(const QString &path);
    void clear();
    bool hasGps() const;
    bool hasGSensor() const;

    // GM-105.
    MovingValue at(TDateTime t) const;
    // GM-034: N columns over [t1, t2] for a graph kind; British units convert S and H.
    QVector<draw::Bin> bins(draw::Graphic::Kind kind, TDateTime t1, TDateTime t2, int n, bool british) const;

    // Track statistics of the whole group (GM-106).
    double trackLength() const;
    int gpsCount() const;
    // Wall-clock span of the GPS and G-sensor samples, in days (GetMaxTime - GetMinTime).
    double span() const;
    // GM-160 step 1 / GM-175: the route as runs of connected points, split at GPS breaks and
    // warned points.
    QVector<QVector<GpsSample>> routeSegments() const;
    // GetNearestPlace (GM-152 routeClick): the time of the GPS point nearest to (lat, lon) if it is
    // nearer than minD (squared degrees, then lowered to it), moved 0.5 s inside its run at the ends
    // and at gaps; -1 when no point is nearer.
    TDateTime nearestPlace(double lat, double lon, double &minD) const;

private:
    static void deriveSpeedAndBearing(QVector<GpsSample> &gps);
    struct FileTrack {
        QVector<GpsSample> gps;
        QVector<GSample> g;
    };
    QVector<FileTrack> files;
};
