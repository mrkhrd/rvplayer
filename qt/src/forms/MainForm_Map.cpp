// The map of TFormMain. Spec: graphs-map-layout.md §3.11, decisions.md Q-10/Q-11.

#include "MainForm.h"

#include "core/Settings.h"
#include "filelist/FileListView.h"
#include "i18n/Lang.h"
#include "map/MapView.h"

#include <QAction>

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

