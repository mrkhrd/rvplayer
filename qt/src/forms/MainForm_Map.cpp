// The map of TFormMain. Spec: graphs-map-layout.md §3.11, decisions.md Q-10/Q-11/Q-27.

#include "MainForm.h"

#include "core/Playback.h"
#include "core/Settings.h"
#include "core/Trace.h"
#include "filelist/FileListView.h"
#include "i18n/Lang.h"
#include "map/MapView.h"

#include <QAction>
#include <QFileDialog>

#include <algorithm>
#include <cmath>

QWidget *MainForm::createControl(const vcl::DfmObject &def, QWidget *parent) {
    if (def.className == "TListView" && def.name == "ListView") return new FileListView(parent);
    if (def.className == "TWebBrowser" && def.name == "WebBrowser") {
        auto *map = new MapView(parent);
        connect(map, &MapView::loaded, this, &MainForm::mapLoaded);
        connect(map, &MapView::inited, this, [this] { setRoute(); });  // a group opened earlier is fitted now
        connect(map, &MapView::viewChanged, this, &MainForm::mapViewChanged);
        connect(map, &MapView::layerChanged, this, [](const QString &name) { Settings::instance().writeString("gm_L", name); });
        connect(map, &MapView::clicked, this, [this](GeoPoint at) { mapClicked(at.lat, at.lon); });
        connect(map, &MapView::apiKeyChanged, this, [](const QString &service, const QString &key) {
            Settings::instance().writeString("mapKey_" + service, key);
        });
        return map;
    }
    return nullptr;
}

MapView *MainForm::mapView() const {
    return find<MapView>("WebBrowser");
}

// Q-10/Q-27: the embedded Google, Yandex, Navitel and Baidu pages are gone; "OpenStreetMap" is the
// map (MapLibre, base layers in its own layer menu) and "None" hides it. There is no page to run
// a script in after loading, so that item is hidden too.
void MainForm::setupMapMenu() {
    for (const char *name : {"N_View_OpenStreetMaps", "N_View_GoogleMaps", "N_View_YandexMaps", "N_View_YandexMaps2",
                             "N_View_NavitelMaps", "N_View_BaiduMaps"}) {
        if (QAction *a = action(name)) {
            a->setVisible(false);
            if (isChecked(a)) setChecked("N_View_OpenStreetMaps2", true);
        }
    }
    if (QAction *a = action("N_View_Maps_PostAction")) a->setVisible(false);
}

// GM-150.
void MainForm::N_View_GoogleMapsClick(vcl::Event *e) {
    auto *src = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (src) menuItemClick(src);
    N_View_FileListClick(nullptr);
    FormResize(nullptr);
    persistSplitters();
}

// GM-152 "loaded": the last view and base layer, or the defaults of the language file.
void MainForm::mapLoaded() {
    MapView *map = mapView();
    if (!map) return;
    Settings &s = Settings::instance();
    const Lang &lang = Lang::instance();
    const double y = s.readString("gm_Y", lang.trDef("gmStartY", "40.713003")).toDouble();
    const double x = s.readString("gm_X", lang.trDef("gmStartX", "-74.005767")).toDouble();
    const int z = s.readInt("gm_Z", lang.trDef("gmStartZoom", "14").toInt());
    MapView::setApiKey("carto", s.readString("mapKey_carto"));  // Q-28
    map->start({y, x}, z, s.readString("gm_L"));
}

// GM-152 "bounds" (while moving) and "view" (once it stops), steps 1–8.
void MainForm::mapViewChanged(bool finished) {
    MapView *map = mapView();
    if (!map || !isVisible()) return;
    const MapView::View v = map->view();
    if (!v.valid) return;
    viewport.top = v.north;
    viewport.bottom = v.south;
    viewport.left = v.west;
    viewport.right = v.east;
    viewport.x = v.centre.lon;
    viewport.y = v.centre.lat;
    viewport.w = v.size.width();
    viewport.h = v.size.height();
    viewport.zoom = int(std::lround(v.zoom));
    viewport.metresPerPixel = v.metresPerPixel;
    viewport.valid = true;
    if (lastCar.valid) mapFollow = carInView(lastCar.lat, lastCar.lon, false);
    if (!finished) return;
    Settings &s = Settings::instance();
    s.writeString("gm_X", QString::number(v.centre.lon, 'f', 6));
    s.writeString("gm_Y", QString::number(v.centre.lat, 'f', 6));
    s.writeInt("gm_Z", std::clamp(viewport.zoom, 10, 17));
}

// GM-152 "routeClick" (L3352–L3371): seek to the nearest track point, if it is within 10 map
// pixels of the click.
void MainForm::mapClicked(double lat, double lon) {
    const Group &g = playback->group();
    if (g.isEmpty() || g.duration <= 0 || !viewport.valid) return;
    double minD = 1e6;
    const TDateTime t = moving.nearestPlace(lat, lon, minD);
    const double pixel = (viewport.right - viewport.left) / std::max(1, viewport.w);  // degrees
    if (t >= 0 && std::sqrt(minD) < 10 * pixel) seekTo(std::clamp(g.progressAt(t), 0.0, 1.0), true);
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

GeoPoint at(const GpsSample &s) {
    return {s.lat, s.lon};
}

// gmGetCirclePoint: the point `d` metres from (lat, lon) along `bearing` degrees.
GeoPoint dest(double lat, double lon, double bearing, double d) {
    const double r = bearing * M_PI / 180;
    const double dlat = d * std::cos(r) / 111320.0;
    const double dlon = d * std::sin(r) / (111320.0 * std::max(0.01, std::cos(lat * M_PI / 180)));
    return {lat + dlat, lon + dlon};
}

}  // namespace

// GM-160: a new route; the map fits it.
void MainForm::setRoute() {
    MapView *map = mapView();
    mapFollow = true;
    mapRecentre = false;
    if (!map) return;
    const auto segments = moving.routeSegments();
    if (segments.isEmpty()) {
        map->clearAll();
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
    map->fitBounds({s, w}, {n, e});
}

// GM-161 (whole route): the runs with gap lines between them, a P marker at the first point of
// every time bucket, S and F. The engine simplifies the lines for each zoom itself.
void MainForm::redrawRoute() {
    RV_TRACE_SCOPE("MainForm::redrawRoute");
    MapView *map = mapView();
    if (!map) return;
    const auto segments = moving.routeSegments();
    if (segments.isEmpty()) return;
    QVector<MapView::Path> paths;
    const GpsSample *prevEnd = nullptr;
    for (const auto &seg : segments) {
        if (prevEnd) paths.append({{at(*prevEnd), at(seg.first())}, MapView::Path::Gap});
        MapView::Path route;
        for (const GpsSample &p : seg) route.points.append(at(p));
        paths.append(route);
        prevEnd = &seg.last();
    }
    // Step 6: a dot at the first point of every time bucket (one at the start even with "Never").
    const double bucket = isChecked("N_View_TimePoints_1m")    ? 1.0 / 1440
                          : isChecked("N_View_TimePoints_5m")  ? 5.0 / 1440
                          : isChecked("N_View_TimePoints_20m") ? 20.0 / 1440
                                                               : 100.0;
    QVector<MapView::Marker> marks;
    qint64 lastBucket = -1;
    for (const auto &seg : segments) {
        for (const GpsSample &p : seg) {
            const qint64 b = qint64(std::floor(p.t / bucket));
            if (b == lastBucket) continue;
            lastBucket = b;
            marks.append({'p', at(p)});
        }
    }
    marks.append({'s', at(segments.first().first())});
    marks.append({'f', at(segments.last().last())});
    map->setPaths(paths);
    map->setPathMarkers(marks);
}

// GM-165 and GM-167: the car arrow at the position, and keeping it in view.
void MainForm::updateMapMarker(const MovingValue &v, bool timerTick) {
    MapView *map = mapView();
    if (!map || !map->isReady()) return;
    lastCar = {v.gps, v.lat, v.lon};
    if (!v.gps) {
        map->clearCar();
        return;
    }
    const double base = 20 * (viewport.valid ? viewport.metresPerPixel : 1);
    if (!v.gpsGood) {
        map->setCarCircle({v.lat, v.lon}, std::lround(1.8 * base));
    } else {
        const double size = base * (isChecked("N_View_Arrow_Small") ? 0.5 : isChecked("N_View_Arrow_Normal") ? 1 / 1.6 : 1.0);
        QVector<GeoPoint> pts;
        if (v.course < 0) {
            for (int a = 30; a < 360; a += 60) pts.append(dest(v.lat, v.lon, a, size / 2));
        } else if (isChecked("N_View_Arrow_Small")) {
            pts << dest(v.lat, v.lon, v.course, size) << dest(v.lat, v.lon, v.course + 120, size / 2) << GeoPoint{v.lat, v.lon}
                << dest(v.lat, v.lon, v.course + 240, size / 2);
        } else {
            const double a = std::max(20.0, 60.0 - v.speed / 3);
            const double tip = size * (1 + v.speed / 300);
            pts << dest(v.lat, v.lon, v.course, tip) << dest(v.lat, v.lon, v.course + 180 - a, size) << GeoPoint{v.lat, v.lon}
                << dest(v.lat, v.lon, v.course + 180 + a, size);
        }
        map->setCarPolygon(pts);
    }
    // GM-167: recentre after a seek, or when a followed car leaves the safe area.
    if (mapRecentre || (timerTick && mapFollow && !carInView(v.lat, v.lon, true))) {
        mapRecentre = false;
        mapFollow = true;
        map->panTo({v.lat, v.lon});
    }
}

// GM-168: map markers at the edges of the timeline zoom window (not the A-B selection).
void MainForm::drawMarkersAB() {
    MapView *map = mapView();
    if (!map || !playback || playback->group().isEmpty()) return;
    const Group &g = playback->group();
    const auto segments = moving.routeSegments();
    const GpsSample *first = segments.isEmpty() ? nullptr : &segments.first().first();
    const GpsSample *last = segments.isEmpty() ? nullptr : &segments.last().last();
    auto marker = [&](double p, bool want, char icon) -> std::optional<MapView::Marker> {
        if (!want) return std::nullopt;
        const MovingValue v = moving.at(g.timeAt(p));
        if (!v.gps) return std::nullopt;
        // A point equal to the route's start or finish is dropped (Q-12: both ends compared).
        for (const GpsSample *end : {first, last}) {
            if (end && end->lat == v.lat && end->lon == v.lon) return std::nullopt;
        }
        return MapView::Marker{icon, {v.lat, v.lon}};
    };
    std::optional<MapView::Marker> a = marker(zoomL, zoomL > 0, 'a'), b = marker(zoomR, zoomR < 1, 'b');
    // SetAB always places both: a missing one takes the other's place.
    if (a && !b) b = MapView::Marker{'b', a->at};
    if (b && !a) a = MapView::Marker{'a', b->at};
    map->setAB(a, b);
}

// GM-172: a KML or GPX file over the map.
void MainForm::N_View_Maps_OpenKMLClick(vcl::Event *) {
    const QString path = QFileDialog::getOpenFileName(
        this, action("N_View_Maps_OpenKML")->text(), QString(),
        QStringLiteral("All supported formats (*.kml *.gpx);;KML (*.kml);;GPX (*.gpx);;All files (*.*)"));
    if (path.isEmpty()) return;
    if (!isChecked("N_View_OpenStreetMaps2")) {
        setChecked("N_View_OpenStreetMaps2", true);
        N_View_GoogleMapsClick(nullptr);
    }
    if (MapView *map = mapView()) map->showOverlay(path);
}

// GM-171 (Q-27): hidden; the map is not a web page any more.
void MainForm::N_View_Maps_PostActionClick(vcl::Event *) {}

// GM-170: zoom around the followed car, or around the point under the mouse.
bool MainForm::mapWheel(int delta, QPoint global) {
    MapView *map = mapView();
    if (!map || !map->isReady()) return false;
    const int d = delta / 120;
    if (d == 0) return true;
    QPointF anchor(map->width() / 2.0, map->height() / 2.0);
    if (mapFollow && lastCar.valid) {
        anchor = map->pixelFor({lastCar.lat, lastCar.lon});
    } else if (const QPoint local = map->mapFromGlobal(global); map->rect().contains(local)) {
        anchor = local;
    }
    map->zoomBy(d, anchor);
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
    if (!map || !map->isReady()) return;
    map->clearAll();
    QVector<MapView::Path> paths;
    double s = 90, w = 180, n = -90, e = -180;
    for (int k = 0; k < keys.size(); ++k) {
        for (const auto &seg : previewCache.value(keys[k])) {
            MapView::Path path;
            path.kind = selected.value(k) ? MapView::Path::Selected : MapView::Path::Route;
            for (const GpsSample &p : seg) {
                path.points.append(at(p));
                s = std::min(s, p.lat);
                n = std::max(n, p.lat);
                w = std::min(w, p.lon);
                e = std::max(e, p.lon);
            }
            paths.append(path);
        }
    }
    map->setPaths(paths);
    if (!paths.isEmpty()) map->fitBounds({s, w}, {n, e});
}
