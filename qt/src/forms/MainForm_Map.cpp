// The map of TFormMain. Spec: graphs-map-layout.md §3.11, decisions.md Q-10/Q-11.

#include "MainForm.h"

#include "core/Playback.h"
#include "core/Settings.h"
#include "core/Trace.h"
#include "filelist/FileListView.h"
#include "i18n/Lang.h"
#include "map/MapView.h"

#include <QAction>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFileDialog>
#include <QPlainTextEdit>
#include <QVBoxLayout>

#include <algorithm>
#include <cmath>

QWidget *MainForm::createControl(const vcl::DfmObject &def, QWidget *parent) {
    if (def.className == "TListView" && def.name == "ListView") return new FileListView(parent);
    if (def.className == "TWebBrowser" && def.name == "WebBrowser") {
        auto *map = new MapView(parent);
        connect(map, &MapView::callback, this, &MainForm::mapCallback);
        return map;
    }
    return nullptr;
}

MapView *MainForm::mapView() const {
    return find<MapView>("WebBrowser");
}

// Q-10: the embedded Google, Yandex, Navitel and Baidu pages (and the OpenLayers variant of
// OSM) no longer work; only the Leaflet OpenStreetMap page and "None" remain.
void MainForm::setupMapMenu() {
    for (const char *name : {"N_View_OpenStreetMaps", "N_View_GoogleMaps", "N_View_YandexMaps", "N_View_YandexMaps2",
                             "N_View_NavitelMaps", "N_View_BaiduMaps"}) {
        if (QAction *a = action(name)) {
            a->setVisible(false);
            if (isChecked(a)) setChecked("N_View_OpenStreetMaps2", true);
        }
    }
}

int MainForm::mapProvider() const {
    if (!mapVisible()) return 0;
    return isChecked("N_View_OpenStreetMaps2") ? 5 : 0;
}

// GM-150.
void MainForm::N_View_GoogleMapsClick(vcl::Event *e) {
    auto *src = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (src) menuItemClick(src);
    N_View_FileListClick(nullptr);
    FormResize(nullptr);
    persistSplitters();
}

// GM-152.
void MainForm::mapCallback(const QString &cmd, const QHash<QString, QString> &params) {
    RV_TRACE_SCOPE("MainForm::mapCallback");
    MapView *map = mapView();
    if (!map || !isVisible()) return;
    Settings &s = Settings::instance();
    const Lang &lang = Lang::instance();
    if (cmd == "loaded") {
        const QString y = s.readString("gm_Y", lang.trDef("gmStartY", "40.713003"));
        const QString x = s.readString("gm_X", lang.trDef("gmStartX", "-74.005767"));
        const int z = s.readInt("gm_Z", lang.trDef("gmStartZoom", "14").toInt());
        const QString layer = s.readString("gm_L").replace('\'', ' ');
        map->exec(QStringLiteral("initialize(%1, %2, %3, '%4')").arg(y, x).arg(z).arg(layer), true);
        // GM-171: the user's script after the map is set up.
        if (const QString post = s.readString("mapPostAction"); !post.isEmpty()) map->exec(post, true);
        return;
    }
    if (cmd == "inited") {
        setRoute();  // a group opened before the map was ready is fitted now
        return;
    }
    if (cmd == "bounds" || cmd == "view") {
        bool okX = false, okY = false, okZ = false;
        const double x = params.value("x").toDouble(&okX);
        const double y = params.value("y").toDouble(&okY);
        const int z = params.value("z").toInt(&okZ);
        if (okX && okY) {
            s.writeString("gm_X", QString::number(x, 'f', 6));
            s.writeString("gm_Y", QString::number(y, 'f', 6));
        }
        if (okZ) s.writeInt("gm_Z", std::clamp(z, 10, 17));
        if (params.contains("mt")) s.writeString("gm_L", params.value("mt"));

        // Steps 1–2 and 8: the viewport, its scale, and whether the car is inside it.
        MapViewport v;
        v.top = params.value("t").toDouble();
        v.left = params.value("l").toDouble();
        v.bottom = params.value("b").toDouble();
        v.right = params.value("r").toDouble();
        v.x = x;
        v.y = y;
        v.w = params.value("w").toInt();
        v.h = params.value("h").toInt();
        if (v.w <= 0 || v.h <= 0) {
            v.w = map->width();
            v.h = map->height();
        }
        v.zoom = z;
        v.metresPerPixel = distanceMetres(v.y, v.left, v.y, v.right) / std::max(1, v.w);
        v.valid = okX && okY && v.right > v.left;
        const bool zoomChanged = v.zoom != viewport.zoom;
        viewport = v;
        if (zoomChanged) redrawRoute();
        if (lastCar.valid) mapFollow = carInView(lastCar.lat, lastCar.lon, false);
    }
}

double MainForm::distanceMetres(double lat1, double lon1, double lat2, double lon2) {
    const double r = M_PI / 180;
    const double c = std::sin(lat1 * r) * std::sin(lat2 * r) + std::cos(lat1 * r) * std::cos(lat2 * r) * std::cos((lon2 - lon1) * r);
    return 6371000.0 * std::acos(std::clamp(c, -1.0, 1.0));
}

// GM-167: inside the view, optionally shrunk by the safe border (max(30 px, 15 %) per axis).
bool MainForm::carInView(double lat, double lon, bool safeBorder) const {
    if (!viewport.valid) return false;
    double bx = 0, by = 0;
    if (safeBorder) {
        bx = (viewport.right - viewport.left) * std::max(30.0 / std::max(1, viewport.w), 0.15);
        by = (viewport.top - viewport.bottom) * std::max(30.0 / std::max(1, viewport.h), 0.15);
    }
    return lon > viewport.left + bx && lon < viewport.right - bx && lat > viewport.bottom + by && lat < viewport.top - by;
}

namespace {

// Point literal for the Leaflet page: [lat, lon], 6 decimals (gmGetPositionStr).
QString pt(double lat, double lon) {
    return QStringLiteral("[%1,%2]").arg(lat, 0, 'f', 6).arg(lon, 0, 'f', 6);
}

// gmGetCirclePoint: the point `d` metres from (lat, lon) along `bearing` degrees.
QString dest(double lat, double lon, double bearing, double d) {
    const double r = bearing * M_PI / 180;
    const double dlat = d * std::cos(r) / 111320.0;
    const double dlon = d * std::sin(r) / (111320.0 * std::max(0.01, std::cos(lat * M_PI / 180)));
    return pt(lat + dlat, lon + dlon);
}

}  // namespace

// GM-160: a new route; the map fits it.
void MainForm::setRoute() {
    MapView *map = mapView();
    lastRouteScript.clear();
    mapFollow = true;
    mapRecentre = false;
    if (!map) return;
    const auto segments = moving.routeSegments();
    if (segments.isEmpty()) {
        map->exec("ClearPath(); ClearMarkers(); ClearMarkersAB();");
        return;
    }
    double s = 90, w = 180, n = -90, e = -180;
    for (const auto &seg : segments) {
        for (const GpsSample &p : seg) {
            s = std::min(s, p.lat);
            n = std::max(n, p.lat);
            w = std::min(w, p.lon);
            e = std::max(e, p.lon);
        }
    }
    // Enlarged while the diagonal is under 400 m.
    for (int i = 0; i < 20 && distanceMetres(s, w, n, e) < 400; ++i) {
        const double dx = std::max(0.0001, (e - w) / 3), dy = std::max(0.0001, (n - s) / 3);
        w -= dx;
        e += dx;
        s -= dy;
        n += dy;
    }
    redrawRoute();
    map->exec(QStringLiteral("SetMapBounds(%1, %2);").arg(pt(s, w), pt(n, e)));
}

// GM-161 (whole route): polylines simplified to the map scale, S and F markers.
void MainForm::redrawRoute() {
    RV_TRACE_SCOPE("MainForm::redrawRoute");
    MapView *map = mapView();
    if (!map || !map->inited()) return;
    const auto segments = moving.routeSegments();
    if (segments.isEmpty()) return;
    double tolerance = 1;
    const double px4 = viewport.valid ? 4 * viewport.metresPerPixel : 4;
    while (tolerance < px4) tolerance *= 2;
    QString script = QStringLiteral("ClearPath();");
    const GpsSample *prevEnd = nullptr;
    for (const auto &seg : segments) {
        QStringList pts{pt(seg.first().lat, seg.first().lon)};
        const GpsSample *kept = &seg.first();
        for (int i = 1; i < seg.size(); ++i) {
            const bool last = i == seg.size() - 1;
            if (last || distanceMetres(kept->lat, kept->lon, seg[i].lat, seg[i].lon) >= tolerance) {
                pts.append(pt(seg[i].lat, seg[i].lon));
                kept = &seg[i];
            }
        }
        // Gap lines join the runs.
        if (prevEnd) {
            script += QStringLiteral("AddPath([%1,%2], '#444444', 0.4);")
                          .arg(pt(prevEnd->lat, prevEnd->lon), pt(seg.first().lat, seg.first().lon));
        }
        script += QStringLiteral("AddPath([%1], '#FF0000', 0.4);").arg(pts.join(','));
        prevEnd = &seg.last();
    }
    // Step 6: a dot at the first point of every time bucket (one at the start even with "Never").
    const double bucket = isChecked("N_View_TimePoints_1m")    ? 1.0 / 1440
                          : isChecked("N_View_TimePoints_5m")  ? 5.0 / 1440
                          : isChecked("N_View_TimePoints_20m") ? 20.0 / 1440
                                                               : 100.0;
    qint64 lastBucket = -1;
    for (const auto &seg : segments) {
        for (const GpsSample &p : seg) {
            const qint64 b = qint64(std::floor(p.t / bucket));
            if (b == lastBucket) continue;
            lastBucket = b;
            script += QStringLiteral("AddPathMarker('gm_markerp.png', %1);").arg(pt(p.lat, p.lon));
        }
    }
    const GpsSample &a = segments.first().first();
    const GpsSample &b = segments.last().last();
    script += QStringLiteral("AddPathMarker('gm_markers.png', %1);AddPathMarker('gm_markerf.png', %2);")
                  .arg(pt(a.lat, a.lon), pt(b.lat, b.lon));
    if (script == lastRouteScript) return;
    if (map->exec(script)) lastRouteScript = script;
}

// GM-165 and GM-167: the car arrow at the position, and keeping it in view.
void MainForm::updateMapMarker(const MovingValue &v, bool timerTick) {
    MapView *map = mapView();
    if (!map || !map->inited()) return;
    lastCar = {v.gps, v.lat, v.lon};
    if (!v.gps) {
        map->exec("ClearMarkers();");
        return;
    }
    const double base = 20 * (viewport.valid ? viewport.metresPerPixel : 1);
    if (!v.gpsGood) {
        map->exec(QStringLiteral("PlaceMarkerD(%1, %2, 36);").arg(pt(v.lat, v.lon)).arg(std::lround(1.8 * base)));
    } else {
        const double size = base * (isChecked("N_View_Arrow_Small") ? 0.5 : isChecked("N_View_Arrow_Normal") ? 1 / 1.6 : 1.0);
        QStringList pts;
        if (v.course < 0) {
            for (int a = 30; a < 360; a += 60) pts.append(dest(v.lat, v.lon, a, size / 2));
        } else if (isChecked("N_View_Arrow_Small")) {
            pts << dest(v.lat, v.lon, v.course, size) << dest(v.lat, v.lon, v.course + 120, size / 2) << pt(v.lat, v.lon)
                << dest(v.lat, v.lon, v.course + 240, size / 2);
        } else {
            const double a = std::max(20.0, 60.0 - v.speed / 3);
            const double tip = size * (1 + v.speed / 300);
            pts << dest(v.lat, v.lon, v.course, tip) << dest(v.lat, v.lon, v.course + 180 - a, size) << pt(v.lat, v.lon)
                << dest(v.lat, v.lon, v.course + 180 + a, size);
        }
        map->exec(QStringLiteral("PlaceMarker([%1]);").arg(pts.join(',')));
    }
    // GM-167: recentre after a seek, or when a followed car leaves the safe area.
    if (mapRecentre || (timerTick && mapFollow && !carInView(v.lat, v.lon, true))) {
        mapRecentre = false;
        mapFollow = true;
        map->exec(QStringLiteral("PanTo(%1);").arg(pt(v.lat, v.lon)));
    }
}


// GM-168: map markers at the edges of the timeline zoom window (not the A-B selection).
void MainForm::drawMarkersAB() {
    MapView *map = mapView();
    if (!map || !map->inited() || !playback || playback->group().isEmpty()) return;
    const Group &g = playback->group();
    const auto segments = moving.routeSegments();
    const GpsSample *first = segments.isEmpty() ? nullptr : &segments.first().first();
    const GpsSample *last = segments.isEmpty() ? nullptr : &segments.last().last();
    auto at = [&](double p, bool want) -> std::optional<MovingValue> {
        if (!want) return std::nullopt;
        const MovingValue v = moving.at(g.timeAt(p));
        if (!v.gps) return std::nullopt;
        // A point equal to the route's start or finish is dropped (Q-12: both ends compared).
        for (const GpsSample *end : {first, last}) {
            if (end && end->lat == v.lat && end->lon == v.lon) return std::nullopt;
        }
        return v;
    };
    std::optional<MovingValue> a = at(zoomL, zoomL > 0), b = at(zoomR, zoomR < 1);
    if (!a && !b) {
        map->exec("ClearMarkersAB();");
        return;
    }
    if (!a) a = b;
    if (!b) b = a;
    map->exec(QStringLiteral("SetAB('gm_markera.png', %1, 'gm_markerb.png', %2);").arg(pt(a->lat, a->lon), pt(b->lat, b->lon)));
}

// GM-172: a KML or GPX overlay on the OpenStreetMap page.
void MainForm::N_View_Maps_OpenKMLClick(vcl::Event *) {
    const QString path = QFileDialog::getOpenFileName(
        this, action("N_View_Maps_OpenKML")->text(), QString(),
        QStringLiteral("All supported formats (*.kml *.gpx);;KML (*.kml);;GPX (*.gpx);;All files (*.*)"));
    if (path.isEmpty()) return;
    if (!isChecked("N_View_OpenStreetMaps2")) {
        setChecked("N_View_OpenStreetMaps2", true);
        N_View_GoogleMapsClick(nullptr);
    }
    MapView *map = mapView();
    if (!map) return;
    const QString url = MapView::serveOverlay(path);
    const QString call = path.endsWith(".gpx", Qt::CaseInsensitive) ? "AddGPXLayer" : "AddKMLLayer";
    map->exec(QStringLiteral("%1('%2');").arg(call, url));
}

// GM-171: a script run after the map page is loaded.
void MainForm::N_View_Maps_PostActionClick(vcl::Event *) {
    QDialog dialog(this, Qt::Tool);
    dialog.setWindowTitle(action("N_View_Maps_PostAction")->text());
    dialog.resize(400, 300);
    dialog.setSizeGripEnabled(true);
    auto *memo = new QPlainTextEdit(&dialog);
    memo->setFont(QFont("Lucida Console", 9));
    memo->setLineWrapMode(QPlainTextEdit::NoWrap);
    memo->setPlainText(Settings::instance().readString("mapPostAction"));
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Ok, &dialog);
    connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
    auto *layout = new QVBoxLayout(&dialog);
    layout->addWidget(memo);
    layout->addWidget(buttons);
    if (dialog.exec() != QDialog::Accepted) return;
    Settings::instance().writeString("mapPostAction", memo->toPlainText());
    if (MapView *map = mapView()) map->reloadPage();
}

// GM-170: zoom around the followed car, or around the point under the mouse.
bool MainForm::mapWheel(int delta, QPoint global) {
    MapView *map = mapView();
    if (!map || !map->inited() || !viewport.valid) return false;
    const int d = delta / 120;
    if (d == 0) return true;
    std::optional<std::pair<double, double>> anchor;  // lat, lon
    if (mapFollow && lastCar.valid) {
        anchor = {lastCar.lat, lastCar.lon};
    } else {
        const QPoint local = map->mapFromGlobal(global);
        if (map->rect().contains(local)) {
            const double lon = viewport.left + double(local.x()) / std::max(1, map->width()) * (viewport.right - viewport.left);
            const double lat = viewport.top - double(local.y()) / std::max(1, map->height()) * (viewport.top - viewport.bottom);
            anchor = {lat, lon};
        }
    }
    const QString a = anchor ? pt(anchor->first, anchor->second) : QStringLiteral("null");
    const double f = std::pow(2.0, -d);
    const double cLat = anchor ? anchor->first + (viewport.y - anchor->first) * f : viewport.y;
    const double cLon = anchor ? anchor->second + (viewport.x - anchor->second) * f : viewport.x;
    map->exec(QStringLiteral("SetMapZoom(%1, %2, %3);").arg(viewport.zoom + d).arg(a, pt(cLat, cLon)));
    return true;
}

void MainForm::previewRoutes(const QList<int> &rows, const QSet<int> &highlighted) {
    if (!fileManager) return;
    QStringList keys;
    QList<bool> selected;
    for (int r : rows) {
        const Group g = fileManager->item(r);
        QStringList paths;
        for (const GroupFile &f : g.files) paths.append(f.path);
        const QString key = paths.join('|');
        if (!previewCache.contains(key)) {
            MovingInfo info;
            info.load(g, trackOptions());
            previewCache.insert(key, info.routeSegments());
        }
        keys.append(key);
        selected.append(highlighted.contains(r));
    }
    drawPreview(keys, selected);
}

// GM-191: the GPS converter's files, each read whole.
void MainForm::previewTrackFiles(const QStringList &paths, const QList<bool> &selected) {
    QStringList keys;
    for (const QString &p : paths) {
        const QString key = "log|" + p;
        if (!previewCache.contains(key)) {
            MovingInfo info;
            info.loadSource(p);
            previewCache.insert(key, info.routeSegments());
        }
        keys.append(key);
    }
    drawPreview(keys, selected);
}

// The cached routes of `keys` in red, selected ones in magenta, the map fitted around them.
void MainForm::drawPreview(const QStringList &keys, const QList<bool> &selected) {
    MapView *map = mapView();
    if (!map || !map->inited()) return;
    QString script = QStringLiteral("ClearPath();ClearMarkers();ClearMarkersAB();");
    double s = 90, w = 180, n = -90, e = -180;
    bool any = false;
    for (int k = 0; k < keys.size(); ++k) {
        const QString colour = selected.value(k) ? QStringLiteral("#FF00FF") : QStringLiteral("#FF0000");
        for (const auto &seg : previewCache.value(keys[k])) {
            QStringList pts;
            // Every few metres is enough for an overview.
            const GpsSample *kept = nullptr;
            for (const GpsSample &p : seg) {
                if (kept && distanceMetres(kept->lat, kept->lon, p.lat, p.lon) < 10 && &p != &seg.last()) continue;
                pts.append(pt(p.lat, p.lon));
                kept = &p;
                s = std::min(s, p.lat);
                n = std::max(n, p.lat);
                w = std::min(w, p.lon);
                e = std::max(e, p.lon);
                any = true;
            }
            script += QStringLiteral("AddPath([%1], '%2', 0.4);").arg(pts.join(','), colour);
        }
    }
    if (any) script += QStringLiteral("SetMapBounds(%1, %2);").arg(pt(s, w), pt(n, e));
    lastRouteScript.clear();
    map->exec(script);
}
