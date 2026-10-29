#include "MovingInfo.h"

#include "core/Format.h"
#include "core/Trace.h"
#include "rv_ffi.h"

#include <QFileInfo>
#include <QHash>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

#include <algorithm>
#include <cmath>

namespace {

constexpr double kEarthRadius = 6371000.0;
constexpr double kOutside = 5.5 * kSecond;  // GM-105: the end sample covers 5.5 s past the track

QJsonObject fileInfo(const QString &path) {
    char *raw = rv_file_info_json(path.toUtf8().constData());
    const QJsonObject o = QJsonDocument::fromJson(raw).object();
    rv_string_free(raw);
    return o;
}

// A GPS log (or any file the core can read a track from).
QJsonObject gpsTrack(const QString &path) {
    char *raw = rv_gps_track_json(path.toUtf8().constData());
    const QJsonObject o = QJsonDocument::fromJson(raw).object();
    rv_string_free(raw);
    return o;
}

// GM-176: spherical law of cosines.
double distance(double lat1, double lon1, double lat2, double lon2) {
    if (std::abs(lat1 - lat2) <= 8.993e-8 && std::abs(lon1 - lon2) <= 8.993e-8) return 0;
    const double r = M_PI / 180;
    const double c = std::sin(lat1 * r) * std::sin(lat2 * r) + std::cos(lat1 * r) * std::cos(lat2 * r) * std::cos((lon2 - lon1) * r);
    return kEarthRadius * std::acos(std::clamp(c, 0.0, 1.0));
}

double median(QVector<double> v) {
    if (v.isEmpty()) return 0;
    std::nth_element(v.begin(), v.begin() + v.size() / 2, v.end());
    return v[v.size() / 2];
}

double lerp(double a, double b, double f) {
    return a + (b - a) * f;
}

// Bearing interpolated on the shorter arc; an unknown side takes the other.
double lerpCourse(double a, double b, double f) {
    if (a < 0) return b;
    if (b < 0) return a;
    double d = b - a;
    if (d > 180) d -= 360;
    if (d < -180) d += 360;
    return std::fmod(a + d * f + 360, 360);
}

template <class T>
int lowerIndex(const QVector<T> &v, TDateTime t) {  // last sample with time <= t, or -1
    auto it = std::upper_bound(v.begin(), v.end(), t, [](TDateTime x, const T &s) { return x < s.t; });
    return int(it - v.begin()) - 1;
}

}  // namespace

void MovingInfo::clear() {
    files.clear();
}

void MovingInfo::load(const Group &group, const Options &options) {
    RV_TRACE_SCOPE("MovingInfo::load");
    files.clear();
    double trackOffset = 0;  // the trip counter continues over the files of a group
    QHash<QString, QJsonObject> logs;  // a log shared by several files is read once (FL-031)
    for (const GroupFile &f : group.files) {
        FileTrack ft;
        // FL-031: the file's GPS info (inside the video or a same-name sidecar), else its GPS log.
        const QString source = !f.infoPath.isEmpty() ? f.infoPath : f.gpsLogPath;
        const bool own = source.isEmpty() || QFileInfo(source) == QFileInfo(f.path);
        QJsonObject info;
        TDateTime t0 = f.start();
        if (own) {
            info = fileInfo(f.path);  // times from the start of the video
        } else {
            if (!logs.contains(source)) logs.insert(source, gpsTrack(source));
            info = logs.value(source);
            // A log has wall-clock times; it is cropped to this file's time span.
            t0 = fmt::fromQDateTime(QDateTime::fromString(info.value("start").toString(), Qt::ISODateWithMs));
        }
        auto inside = [&](TDateTime t) { return own || (t >= f.start() && t <= f.finish); };
        for (const QJsonValue &v : info.value("gps").toArray()) {
            const QJsonArray a = v.toArray();
            GpsSample s;
            s.t = t0 + a[0].toDouble() * kSecond;
            if (!inside(s.t)) continue;
            s.lat = a[1].toDouble();
            s.lon = a[2].toDouble();
            s.alt = a[3].toDouble();
            s.speed = a[4].toDouble();
            s.course = a[5].toDouble();
            s.warn = a[6].toInt() != 0;
            // GM-101: flips first, then the shift.
            if (options.flipX) s.lon = -s.lon;
            if (options.flipY) s.lat = -s.lat;
            s.lon += options.shiftX;
            s.lat += options.shiftY;
            if (options.speedFromCoordinates) {
                s.speed = -1;
                s.course = -1;
                s.warn = false;
            }
            // GM-104: points on whole degrees (including zeroed ones) are not positions.
            if (s.lat == std::floor(s.lat) || s.lon == std::floor(s.lon)) continue;
            ft.gps.append(s);
        }
        // GM-104: cumulative distance; implausible steps and warned points add nothing.
        for (int i = 1; i < ft.gps.size(); ++i) {
            GpsSample &b = ft.gps[i];
            const GpsSample &a = ft.gps[i - 1];
            const double d = distance(a.lat, a.lon, b.lat, b.lon);
            const double dt = (b.t - a.t) * 86400.0;
            const bool ok = !b.warn && dt > 0 && dt < 20 && d < 1000 && d / dt < 500;
            b.track = a.track + (ok ? d : 0);
        }
        if (options.speedFromCoordinates) deriveSpeedAndBearing(ft.gps);
        for (GpsSample &s : ft.gps) s.track += trackOffset;
        if (!ft.gps.isEmpty()) trackOffset = ft.gps.last().track;
        for (const QJsonValue &v : info.value("gsensor").toArray()) {
            const QJsonArray a = v.toArray();
            const GSample s{t0 + a[0].toDouble() * kSecond, a[1].toDouble(), a[2].toDouble(), a[3].toDouble()};
            if (!inside(s.t)) continue;
            // Some cameras store each reading twice (5 Hz data at 10 Hz). The repeat would make
            // every other G value 0 and the graph a sawtooth, so it is dropped (original bug, fixed).
            if (!ft.g.isEmpty()) {
                const GSample &p = ft.g.last();
                if (s.x == p.x && s.y == p.y && s.z == p.z && s.t - p.t <= 0.25 * kSecond) continue;
            }
            ft.g.append(s);
        }
        // GM-103 steps 5–8: remove each axis' median, then the total vector and the horizontal change.
        QVector<double> xs, ys, zs;
        for (const GSample &s : ft.g) {
            xs.append(s.x);
            ys.append(s.y);
            zs.append(s.z);
        }
        const double mx = median(xs), my = median(ys), mz = median(zs);
        for (GSample &s : ft.g) {
            s.x -= mx;
            s.y -= my;
            s.z -= mz;
            s.v = std::sqrt(s.x * s.x + s.y * s.y + s.z * s.z);
        }
        for (int i = 0; i + 1 < ft.g.size(); ++i) {
            const double dx = ft.g[i].x - ft.g[i + 1].x, dy = ft.g[i].y - ft.g[i + 1].y;
            ft.g[i].g = std::sqrt(dx * dx + dy * dy);
        }
        if (!ft.g.isEmpty()) ft.g.last().v = ft.g.last().g = 0;
        files.append(ft);
    }
}

void MovingInfo::loadSource(const QString &path) {
    // One file whose GPS info is `path` and whose window holds any time.
    GroupFile f;
    f.infoPath = path;
    f.finish = 1e7;
    f.duration = 2e7;
    Group g;
    g.files = {f};
    load(g);
}

int MovingInfo::gpsCount() const {
    int n = 0;
    for (const FileTrack &f : files) n += int(f.gps.size());
    return n;
}

double MovingInfo::span() const {
    double lo = 1e9, hi = -1e9;
    for (const FileTrack &f : files) {
        for (const auto *t : {f.gps.isEmpty() ? nullptr : &f.gps.first().t, f.gps.isEmpty() ? nullptr : &f.gps.last().t,
                              f.g.isEmpty() ? nullptr : &f.g.first().t, f.g.isEmpty() ? nullptr : &f.g.last().t}) {
            if (!t) continue;
            lo = std::min(lo, *t);
            hi = std::max(hi, *t);
        }
    }
    return hi > lo ? hi - lo : 0;
}

bool MovingInfo::hasGps() const {
    return std::any_of(files.begin(), files.end(), [](const FileTrack &f) { return f.gps.size() >= 2; });
}

bool MovingInfo::hasGSensor() const {
    return std::any_of(files.begin(), files.end(), [](const FileTrack &f) { return f.g.size() >= 2; });
}

double MovingInfo::trackLength() const {
    for (auto it = files.rbegin(); it != files.rend(); ++it) {
        if (!it->gps.isEmpty()) return it->gps.last().track;
    }
    return 0;
}

MovingValue MovingInfo::at(TDateTime t) const {
    MovingValue r;
    int gpsBase = 0, gBase = 0;
    for (const FileTrack &f : files) {
        if (!r.gps && !f.gps.isEmpty() && t >= f.gps.first().t - kOutside && t <= f.gps.last().t + kOutside) {
            const int i = lowerIndex(f.gps, t);
            const GpsSample &a = f.gps[std::max(i, 0)];
            const GpsSample &b = f.gps[std::min(i + 1, int(f.gps.size()) - 1)];
            const double span = b.t - a.t;
            const double k = (i < 0 || span <= 0) ? 0 : std::clamp((t - a.t) / span, 0.0, 1.0);
            r.gps = true;
            r.gpsIndex = gpsBase + std::max(i, 0);
            r.lat = lerp(a.lat, b.lat, k);
            r.lon = lerp(a.lon, b.lon, k);
            r.speed = lerp(a.speed, b.speed, k);
            r.track = lerp(a.track, b.track, k);
            r.alt = a.alt == 0 ? b.alt : b.alt == 0 ? a.alt : lerp(a.alt, b.alt, k);
            r.course = lerpCourse(a.course, b.course, k);
            r.gpsGood = !a.warn && !b.warn && (span < 11 * kSecond || std::min(t - a.t, b.t - t) < 2.75 * kSecond);
        }
        if (!r.gsensor && !f.g.isEmpty() && t >= f.g.first().t - kOutside && t <= f.g.last().t + kOutside) {
            const int i = lowerIndex(f.g, t);
            const GSample &a = f.g[std::max(i, 0)];
            const GSample &b = f.g[std::min(i + 1, int(f.g.size()) - 1)];
            const double span = b.t - a.t;
            const double k = (i < 0 || span <= 0) ? 0 : std::clamp((t - a.t) / span, 0.0, 1.0);
            r.gsensor = true;
            r.gIndex = gBase + std::max(i, 0);
            r.x = lerp(a.x, b.x, k);
            r.y = lerp(a.y, b.y, k);
            r.z = lerp(a.z, b.z, k);
        }
        gpsBase += int(f.gps.size());
        gBase += int(f.g.size());
    }
    return r;
}

QVector<draw::Bin> MovingInfo::bins(draw::Graphic::Kind kind, TDateTime t1, TDateTime t2, int n, bool british) const {
    using Kind = draw::Graphic::Kind;
    QVector<draw::Bin> out(std::max(n, 0));
    if (n <= 0 || t2 <= t1) return out;
    const double k = n > 1 ? (t2 - t1) / (n - 1) : 4 * (t2 - t1);
    auto add = [&](int i, double v, TDateTime t) {
        if (i < 0 || i >= n) return;
        draw::Bin &b = out[i];
        if (b.count == 0) {
            b.min = b.max = v;
            b.time = t;
        } else {
            b.min = std::min(b.min, v);
            b.max = std::max(b.max, v);
        }
        ++b.count;
    };
    // One series: (time, value) pairs in time order.
    auto series = [&](int count, auto timeOf, auto valueOf) {
        for (int i = 0; i < count; ++i) {
            const TDateTime t = timeOf(i);
            if (t < t1) continue;
            if (t > t2) {
                // The value at t2 closes the last column, if it is still empty.
                if (i > 0 && out[n - 1].count == 0) {
                    const double f = (t2 - timeOf(i - 1)) / (t - timeOf(i - 1));
                    add(n - 1, lerp(valueOf(i - 1), valueOf(i), f), t2);
                }
                break;
            }
            if (i > 0 && timeOf(i - 1) < t1) {  // the value at t1 opens column 0
                const double f = (t1 - timeOf(i - 1)) / (t - timeOf(i - 1));
                add(0, lerp(valueOf(i - 1), valueOf(i), f), t1);
            }
            add(int(std::lround((t - t1) / k)), valueOf(i), t);
        }
    };
    for (const FileTrack &f : files) {
        if (kind == Kind::S || kind == Kind::H) {
            const double scale = british ? (kind == Kind::S ? 0.621371192 : 3.280839895) : 1.0;
            series(int(f.gps.size()), [&](int i) { return f.gps[i].t; },
                   [&](int i) { return (kind == Kind::S ? f.gps[i].speed : f.gps[i].alt) * scale; });
        } else {
            series(int(f.g.size()), [&](int i) { return f.g[i].t; }, [&](int i) {
                const GSample &s = f.g[i];
                switch (kind) {
                case Kind::G: return s.g;
                case Kind::V: return s.v;
                case Kind::X: return s.x;
                case Kind::Y: return s.y;
                default: return s.z;
                }
            });
        }
    }
    return out;
}

namespace {

// GM-175: consecutive points are connected unless the jump is too far for the time between them.
bool connected(const GpsSample &a, const GpsSample &b) {
    const double dist = std::abs(b.lon - a.lon) + std::abs(b.lat - a.lat);
    const double dt = (b.t - a.t) * 86400.0;
    return (dist < 0.005 && dt <= 11) || (dist <= 0.001 && dt <= 15 * 60) || (dist <= 0.0002 && dt <= 4 * 3600) ||
           (dt <= 11 && dist < 0.003 * dt);
}

}  // namespace

QVector<QVector<GpsSample>> MovingInfo::routeSegments() const {
    QVector<QVector<GpsSample>> out;
    QVector<GpsSample> run;
    for (const FileTrack &f : files) {
        for (const GpsSample &s : f.gps) {
            if (!run.isEmpty() && (s.warn || !connected(run.last(), s))) {
                if (run.size() >= 2) out.append(run);
                run.clear();
            }
            if (!s.warn) run.append(s);
        }
    }
    if (run.size() >= 2) out.append(run);
    return out;
}

// UnitMovingInfo L1019–L1033: a gap is more than 11 s between points.
TDateTime MovingInfo::nearestPlace(double lat, double lon, double &minD) const {
    constexpr double kGap = 11 * kSecond, kInside = 0.5 * kSecond;
    TDateTime r = -1;
    for (const FileTrack &f : files) {
        const QVector<GpsSample> &g = f.gps;
        for (int i = 0; i < g.size(); ++i) {
            const double d = (g[i].lon - lon) * (g[i].lon - lon) + (g[i].lat - lat) * (g[i].lat - lat);
            if (d >= minD) continue;
            minD = d;
            r = g[i].t;
            if (i == 0 || g[i].t - g[i - 1].t > kGap) r += kInside;
            else if (i == g.size() - 1 || g[i + 1].t - g[i].t > kGap) r -= kInside;
        }
    }
    return r;
}

// GM-104 (coordinate-only mode): speed from the distance over a window around each point that
// spans a time step, bearing between points i-2 and i+4.
void MovingInfo::deriveSpeedAndBearing(QVector<GpsSample> &gps) {
    const int n = int(gps.size());
    for (int i = 0; i < n; ++i) {
        for (int w = 1; w <= 10; ++w) {
            const int a = std::max(0, i - w), b = std::min(n - 1, i + w);
            const double dt = (gps[b].t - gps[a].t) * 86400.0;
            if (dt <= 0) continue;
            gps[i].speed = (gps[b].track - gps[a].track) / dt * 3.6;
            break;
        }
        if (gps[i].speed < 0) gps[i].speed = 0;
        const GpsSample &p = gps[std::max(0, i - 2)];
        const GpsSample &q = gps[std::min(n - 1, i + 4)];
        const double dx = (q.lon - p.lon) * std::cos(gps[i].lat * M_PI / 180);
        const double dy = q.lat - p.lat;
        if (dx != 0 || dy != 0) gps[i].course = std::fmod(std::atan2(dx, dy) * 180 / M_PI + 360, 360);
        else if (i > 0) gps[i].course = gps[i - 1].course;
    }
}
