#include "MapView.h"

#include "i18n/Lang.h"

#include <QMapLibre/Map>
#include <QMapLibre/Settings>
#include <QMapLibreWidgets/GLWidget>

#include <QActionGroup>
#include <QDir>
#include <QFile>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLineF>
#include <QLabel>
#include <QMenu>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QToolButton>
#include <QToolTip>
#include <QUrl>
#include <QUrlQuery>
#include <QVBoxLayout>
#include <QVariantAnimation>
#include <QXmlStreamReader>

#include <cmath>
#include <mutex>

namespace {

constexpr double kEarthRadius = 6371000.0;

double distance(GeoPoint a, GeoPoint b) {
    const double r = M_PI / 180;
    const double c = std::sin(a.lat * r) * std::sin(b.lat * r) + std::cos(a.lat * r) * std::cos(b.lat * r) * std::cos((b.lon - a.lon) * r);
    return kEarthRadius * std::acos(std::clamp(c, -1.0, 1.0));
}

// --- GeoJSON of the overlay sources ---

void appendCoords(QByteArray &out, const QVector<GeoPoint> &pts) {
    out += '[';
    for (int i = 0; i < pts.size(); ++i) {
        if (i) out += ',';
        out += '[' + QByteArray::number(pts[i].lon, 'f', 7) + ',' + QByteArray::number(pts[i].lat, 'f', 7) + ']';
    }
    out += ']';
}

QByteArray collection(const QByteArrayList &features) {
    return R"({"type":"FeatureCollection","features":[)" + features.join(',') + "]}";
}

QByteArray lineFeature(const QVector<GeoPoint> &pts) {
    QByteArray f = R"({"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":)";
    appendCoords(f, pts);
    return f + "}}";
}

QByteArray pointFeature(GeoPoint p, char icon = 0) {
    QByteArray props = icon ? R"({"icon":"rv-)" + QByteArray(1, icon) + R"("})" : QByteArray("{}");
    return R"({"type":"Feature","properties":)" + props + R"(,"geometry":{"type":"Point","coordinates":[)" +
           QByteArray::number(p.lon, 'f', 7) + ',' + QByteArray::number(p.lat, 'f', 7) + "]}}";
}

QByteArray polygonFeature(QVector<GeoPoint> ring) {
    if (!ring.isEmpty()) ring.append(ring.first());
    QByteArray f = R"({"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[)";
    appendCoords(f, ring);
    return f + "]}}";
}

// gmGetCirclePoint: the point `d` metres from p along `bearing` degrees.
GeoPoint destination(GeoPoint p, double bearing, double d) {
    const double r = bearing * M_PI / 180;
    return {p.lat + d * std::cos(r) / 111320.0, p.lon + d * std::sin(r) / (111320.0 * std::max(0.01, std::cos(p.lat * M_PI / 180)))};
}

// The overlay sources, bottom to top.
const char *const kSources[] = {"rv-overlay-lines", "rv-overlay-points", "rv-gaps", "rv-route", "rv-selected",
                                "rv-pmarks",        "rv-marks",          "rv-circle", "rv-car", "rv-ruler", "rv-ruler-points"};

// A raster base layer as a style of its own.
QString rasterStyle(const MapView::Layer &l) {
    QJsonObject sources;
    QJsonArray layers;
    const QStringList parts = l.url.split('|');
    for (int i = 0; i < parts.size(); ++i) {
        QJsonArray tiles;
        if (parts[i].contains("{s}")) {
            for (const char *s : {"a", "b", "c"}) tiles.append(QString(parts[i]).replace("{s}", s));
        } else {
            tiles.append(parts[i]);
        }
        QJsonObject src{{"type", "raster"}, {"tiles", tiles}, {"tileSize", 256}, {"maxzoom", l.maxZoom}};
        if (i == 0) src["attribution"] = l.attribution;
        const QString id = QStringLiteral("base%1").arg(i);
        sources[id] = src;
        layers.append(QJsonObject{{"id", id}, {"type", "raster"}, {"source", id}});
    }
    return QString::fromUtf8(QJsonDocument(QJsonObject{{"version", 8}, {"sources", sources}, {"layers", layers}}).toJson(QJsonDocument::Compact));
}

// GM-172: the lines (tracks, routes, line strings) and points (waypoints, placemarks) of a GPX
// or KML file.
void readOverlay(QIODevice &file, QVector<QVector<GeoPoint>> &lines, QVector<GeoPoint> &points) {
    QXmlStreamReader x(&file);
    QVector<GeoPoint> line;
    auto finish = [&] {
        if (line.size() >= 2) lines.append(line);
        line.clear();
    };
    // KML "lon,lat[,alt]" tuples separated by white space.
    auto kmlCoords = [](const QString &text) {
        QVector<GeoPoint> pts;
        for (const QString &t : text.split(QRegularExpression("\\s+"), Qt::SkipEmptyParts)) {
            const QStringList v = t.split(',');
            if (v.size() >= 2) pts.append({v[1].toDouble(), v[0].toDouble()});
        }
        return pts;
    };
    QString geometry;  // the KML geometry being read
    while (!x.atEnd()) {
        x.readNext();
        const QString name = x.name().toString().toLower();
        if (x.isStartElement()) {
            if (name == "trkseg" || name == "rte" || name == "track") {
                finish();
            } else if (name == "trkpt" || name == "rtept" || name == "wpt") {
                const GeoPoint p{x.attributes().value("lat").toDouble(), x.attributes().value("lon").toDouble()};
                if (name == "wpt") points.append(p);
                else line.append(p);
            } else if (name == "linestring" || name == "linearring" || name == "point") {
                geometry = name;
            } else if (name == "coordinates") {
                const QVector<GeoPoint> pts = kmlCoords(x.readElementText());
                if (geometry == "point") points += pts;
                else if (pts.size() >= 2) lines.append(pts);
            } else if (name == "coord") {  // gx:Track: "lon lat alt"
                const QStringList v = x.readElementText().split(' ', Qt::SkipEmptyParts);
                if (v.size() >= 2) line.append({v[1].toDouble(), v[0].toDouble()});
            }
        } else if (x.isEndElement() && (name == "trkseg" || name == "rte" || name == "track")) {
            finish();
        }
    }
    finish();
}

// Leaflet's scale control: the longest round distance that fits in 100 pixels.
double roundDown(double v) {
    const double p = std::pow(10.0, std::floor(std::log10(v)));
    const double d = v / p;
    return p * (d >= 10 ? 10 : d >= 5 ? 5 : d >= 3 ? 3 : d >= 2 ? 2 : 1);
}

QIcon layersIcon() {
    QPixmap pm(QSize(26, 26) * 2);
    pm.setDevicePixelRatio(2);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing);
    for (int i = 2; i >= 0; --i) {
        QPainterPath d;
        const double y = 7 + i * 4.5;
        d.moveTo(13, y - 5);
        d.lineTo(23, y);
        d.lineTo(13, y + 5);
        d.lineTo(3, y);
        d.closeSubpath();
        p.setPen(QPen(QColor(60, 60, 60), 1.2));
        p.setBrush(i == 0 ? QColor(140, 190, 120) : QColor(235, 235, 235));
        p.drawPath(d);
    }
    return QIcon(pm);
}

// Q-28: the providers' API keys, read by the engine's request threads, and their servers.
std::mutex keyLock;
QHash<QString, QString> keys;
const QHash<QString, QString> keyHosts{{"carto", "basemaps.cartocdn.com"}};

// The engine's resource transform.
std::string withKey(const std::string &url) {
    return MapView::withApiKey(QString::fromStdString(url)).toStdString();
}

}  // namespace

// A request to a provider's server carries its key (?key=).
QString MapView::withApiKey(const QString &url) {
    const QString host = QUrl(url).host();
    for (auto it = keyHosts.begin(); it != keyHosts.end(); ++it) {
        if (host != it.value() && !host.endsWith('.' + it.value())) continue;
        const QString key = apiKey(it.key());
        if (key.isEmpty() || QUrlQuery(QUrl(url)).hasQueryItem("key")) return url;
        return url + (url.contains('?') ? '&' : '?') + "key=" + QString::fromLatin1(QUrl::toPercentEncoding(key));
    }
    return url;
}

void MapView::setApiKey(const QString &service, const QString &key) {
    const std::lock_guard lock(keyLock);
    keys.insert(service, key.trimmed());
}

QString MapView::apiKey(const QString &service) {
    const std::lock_guard lock(keyLock);
    return keys.value(service);
}

const QVector<MapView::Layer> &MapView::layers() {
    static const QString osm = QStringLiteral("&copy; <a href=\"https://www.openstreetmap.org/copyright\">OpenStreetMap</a> contributors");
    static const QString esri = QStringLiteral("Tiles &copy; Esri &mdash; Source: Esri, Maxar, Earthstar Geographics, and the GIS User Community");
    static const QString esriImagery = QStringLiteral("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}");
    static const QString freeMap = QStringLiteral("https://tiles.openfreemap.org/styles/");
    static const QVector<Layer> list = {
        {"OpenFreeMap Liberty", true, freeMap + "liberty", {}},
        {"OpenFreeMap Bright", true, freeMap + "bright", {}},
        {"OpenFreeMap Positron", true, freeMap + "positron", {}},
        {"OpenFreeMap Dark", true, freeMap + "dark", {}},
        {"OpenFreeMap Fiord", true, freeMap + "fiord", {}},
        // CARTO's styles and every tile, sprite and font of theirs need a (free) key.
        {"CARTO Voyager", true, "https://basemaps.cartocdn.com/gl/voyager-gl-style/style.json", {}, 19, "carto"},
        {"CARTO Positron", true, "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json", {}, 19, "carto"},
        {"CARTO Dark Matter", true, "https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json", {}, 19, "carto"},
        {"OpenStreetMap", false, "https://tile.openstreetmap.org/{z}/{x}/{y}.png", osm},
        {"OpenStreetMap Humanitarian", false, "https://{s}.tile.openstreetmap.fr/hot/{z}/{x}/{y}.png",
         osm + ", Tiles style by <a href=\"https://www.hotosm.org/\">HOT</a>"},
        {"Satellite (Esri)", false, esriImagery, esri},
        {"Hybrid (Esri)", false,
         esriImagery + "|https://server.arcgisonline.com/ArcGIS/rest/services/Reference/World_Boundaries_and_Places/MapServer/tile/{z}/{y}/{x}",
         esri},
        {"Esri Streets", false, "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}", "Tiles &copy; Esri"},
        {"Esri Topographic", false, "https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{z}/{y}/{x}", "Tiles &copy; Esri"},
        {"OpenTopoMap", false, "https://{s}.tile.opentopomap.org/{z}/{x}/{y}.png",
         osm + ", SRTM | &copy; <a href=\"https://opentopomap.org\">OpenTopoMap</a> (CC-BY-SA)", 17},
        {"CyclOSM", false, "https://{s}.tile-cyclosm.openstreetmap.fr/cyclosm/{z}/{x}/{y}.png",
         osm + ", <a href=\"https://www.cyclosm.org\">CyclOSM</a>", 20},
    };
    return list;
}

// The engine's widget. A left drag pans (no rotation or tilt: the original map is north-up), a
// left click without a drag is reported, a double click zooms in.
class MapView::Canvas : public QMapLibre::GLWidget {
public:
    Canvas(const QMapLibre::Settings &s, MapView *owner) : GLWidget(s), owner(owner) {
        setFocusPolicy(Qt::NoFocus);  // GM-152 "view": the map never keeps the keyboard
        setMouseTracking(true);
    }

protected:
    void initializeGL() override {
        GLWidget::initializeGL();
        owner->onCreated();
    }
    void mousePressEvent(QMouseEvent *e) override {
        press = e->position();
        dragged = false;
        if (e->button() == Qt::LeftButton) GLWidget::mousePressEvent(e);
        e->accept();
    }
    void mouseMoveEvent(QMouseEvent *e) override {
        if (e->buttons() == Qt::LeftButton && !(e->modifiers() & Qt::ShiftModifier)) {
            if ((e->position() - press).manhattanLength() > 3) dragged = true;
            if (dragged) GLWidget::mouseMoveEvent(e);
        } else if (e->buttons() == Qt::NoButton) {
            GLWidget::mouseMoveEvent(e);
        }
        e->accept();
    }
    void mouseReleaseEvent(QMouseEvent *e) override {
        GLWidget::mouseReleaseEvent(e);
        if (e->button() == Qt::LeftButton && !dragged) owner->canvasClick(e->position());
    }

private:
    MapView *owner;
    QPointF press;
    bool dragged = false;
};

// Leaflet's scale control: metric over imperial.
class MapView::ScaleBar : public QWidget {
public:
    explicit ScaleBar(MapView *v) : QWidget(v), view(v) {
        setAttribute(Qt::WA_TransparentForMouseEvents);
        resize(104, 38);
    }

protected:
    void paintEvent(QPaintEvent *) override {
        const double mpp = view->view().metresPerPixel;
        if (mpp <= 0) return;
        QPainter p(this);
        QFont f = font();
        f.setPixelSize(11);
        p.setFont(f);
        auto bar = [&](int y, double px, const QString &text, bool top) {
            const QRect r(0, y, int(std::lround(px)), 16);
            p.fillRect(r, QColor(255, 255, 255, 128));
            p.setPen(QPen(QColor(0x77, 0x77, 0x77), 2));
            if (top) p.drawPolyline(QPolygon{r.bottomLeft(), r.topLeft(), r.topRight(), r.bottomRight()});
            else p.drawPolyline(QPolygon{r.topLeft(), r.bottomLeft(), r.bottomRight(), r.topRight()});
            p.setPen(QColor(0x33, 0x33, 0x33));
            p.drawText(r.adjusted(5, 0, 0, 0), Qt::AlignLeft | Qt::AlignVCenter, text);
        };
        const double metres = roundDown(mpp * 100);
        bar(2, metres / mpp, metres >= 1000 ? QStringLiteral("%1 km").arg(metres / 1000) : QStringLiteral("%1 m").arg(metres), false);
        const double feet = mpp * 100 * 3.2808399;
        if (feet > 5280) {
            const double miles = roundDown(feet / 5280);
            bar(20, miles * 1609.344 / mpp, QStringLiteral("%1 mi").arg(miles), true);
        } else {
            const double ft = roundDown(feet);
            bar(20, ft / 3.2808399 / mpp, QStringLiteral("%1 ft").arg(ft), true);
        }
    }

private:
    MapView *view;
};

MapView::MapView(QWidget *parent) : QWidget(parent) {
    QMapLibre::Settings s;
    // OpenFreeMap and the OSM tile policy: a User-Agent of the application's own.
    s.setClientName("RVPlayer");
    s.setClientVersion("0.1");
    const QString cache = QStandardPaths::writableLocation(QStandardPaths::CacheLocation);
    QDir().mkpath(cache);
    s.setCacheDatabasePath(QDir(cache).filePath("map-tiles.db"));
    s.setCacheDatabaseMaximumSize(256u * 1024 * 1024);
    s.setResourceTransform(withKey);
    canvas = new Canvas(s, this);
    canvas->installEventFilter(this);
    auto *l = new QVBoxLayout(this);
    l->setContentsMargins(0, 0, 0, 0);
    l->addWidget(canvas);

    const QString button = QStringLiteral(
        "QToolButton { background: white; border: none; color: black; font: bold 18px 'Lucida Console'; }"
        "QToolButton:hover { background: #f4f4f4; }");
    // Zoom: + over -, top left.
    zoomBox = new QWidget(this);
    zoomBox->setObjectName("zoomBox");
    zoomBox->setStyleSheet("#zoomBox { border: 2px solid rgba(0,0,0,0.2); border-radius: 4px; background: white; }" + button);
    auto *zl = new QVBoxLayout(zoomBox);
    zl->setContentsMargins(0, 0, 0, 0);
    zl->setSpacing(0);
    for (int step : {1, -1}) {
        auto *b = new QToolButton(zoomBox);
        b->setText(step > 0 ? "+" : QStringLiteral("−"));
        b->setFixedSize(26, 26);
        b->setFocusPolicy(Qt::NoFocus);
        connect(b, &QToolButton::clicked, this, [this, step] { zoomBy(step, QPointF(canvas->width() / 2.0, canvas->height() / 2.0)); });
        zl->addWidget(b);
    }
    zoomBox->adjustSize();

    // The distance tool (OSM2_DISTANCE.JS): the ruler toggles it, the cross clears it, the total
    // is shown under them.
    rulerBox = new QWidget(this);
    rulerBox->setObjectName("rulerBox");
    rulerBox->setStyleSheet(
        "#rulerBox { background: rgba(0,0,0,0.25); border-radius: 7px; }"
        "QToolButton { background: rgba(255,255,255,0.75); border: none; border-radius: 4px; padding: 2px; }"
        "QToolButton:checked { background: rgba(255,255,255,1); }"
        "QLabel { background: rgba(255,255,255,0.75); border-radius: 4px; padding: 0 3px; color: #333; font: 11px; }");
    auto *rl = new QGridLayout(rulerBox);
    rl->setContentsMargins(5, 5, 5, 5);
    rl->setSpacing(4);
    rulerButton = new QToolButton(rulerBox);
    rulerButton->setObjectName("rulerButton");
    rulerButton->setIcon(QIcon(":/original/html/OSM2_DISTANCE_MEASURE.PNG"));
    rulerButton->setCheckable(true);
    rulerButton->setToolTip("Edit");
    rulerClear = new QToolButton(rulerBox);
    rulerClear->setObjectName("rulerClear");
    rulerClear->setIcon(QIcon(":/original/html/OSM2_DISTANCE_CROSS.PNG"));
    rulerClear->setToolTip("Delete");
    rulerText = new QLabel(rulerBox);
    rulerText->setObjectName("rulerText");
    for (QToolButton *b : {rulerButton, rulerClear}) {
        b->setFixedSize(20, 20);
        b->setFocusPolicy(Qt::NoFocus);
    }
    rl->addWidget(rulerButton, 0, 0);
    rl->addWidget(rulerClear, 0, 1);
    rl->addWidget(rulerText, 1, 0, 1, 3);
    rl->setColumnStretch(2, 1);
    connect(rulerButton, &QToolButton::toggled, this, &MapView::setRulerActive);
    connect(rulerClear, &QToolButton::clicked, this, [this] {
        ruler.clear();
        updateRuler();
    });
    setRulerActive(false);

    // Base layers, top right.
    layerButton = new QToolButton(this);
    layerButton->setObjectName("layerButton");
    layerButton->setIcon(layersIcon());
    layerButton->setIconSize(QSize(26, 26));
    layerButton->setFixedSize(36, 36);
    layerButton->setFocusPolicy(Qt::NoFocus);
    layerButton->setPopupMode(QToolButton::InstantPopup);
    layerButton->setStyleSheet(
        "QToolButton { background: white; border: 2px solid rgba(0,0,0,0.2); border-radius: 5px; }"
        "QToolButton::menu-indicator { image: none; }");
    auto *menu = new QMenu(layerButton);
    auto *group = new QActionGroup(menu);
    bool vector = true;
    for (const Layer &l : layers()) {
        if (vector && !l.vector) menu->addSeparator();
        vector = l.vector;
        QAction *a = menu->addAction(l.name);
        a->setCheckable(true);
        group->addAction(a);
        connect(a, &QAction::triggered, this, [this, name = l.name] { chooseLayer(name); });
    }
    menu->addSeparator();
    QAction *cartoKey = menu->addAction(QStringLiteral("CARTO API key..."));
    cartoKey->setObjectName("cartoKey");
    connect(cartoKey, &QAction::triggered, this, [this] { askApiKey("carto"); });
    connect(menu, &QMenu::aboutToShow, this, [this, menu, cartoKey] {
        for (QAction *a : menu->actions()) {
            if (a->isCheckable()) a->setChecked(a->text() == layer_);
        }
        cartoKey->setText(Lang::instance().current() == "ru" ? QStringLiteral("Ключ API CARTO...") : QStringLiteral("CARTO API key..."));
    });
    layerButton->setMenu(menu);

    scale = new ScaleBar(this);
    attribution = new QLabel(this);
    attribution->setTextFormat(Qt::RichText);
    attribution->setOpenExternalLinks(true);
    attribution->setStyleSheet("QLabel { background: rgba(255,255,255,0.7); color: #333; padding: 0 5px; font: 11px; }");

    pan = new QVariantAnimation(this);
    pan->setDuration(250);
    pan->setEasingCurve(QEasingCurve::OutCubic);
    connect(pan, &QVariantAnimation::valueChanged, this, [this](const QVariant &v) {
        const QPointF p = v.toPointF();
        if (QMapLibre::Map *m = engine()) m->setCoordinate({p.x(), p.y()});
    });

    for (const char *s : kSources) data.insert(QString::fromLatin1(s), collection({}));
    for (QWidget *w : {zoomBox, rulerBox, static_cast<QWidget *>(layerButton), static_cast<QWidget *>(scale), static_cast<QWidget *>(attribution)}) {
        w->raise();
    }
}

MapView::~MapView() = default;

QMapLibre::Map *MapView::engine() const {
    return canvas ? canvas->map() : nullptr;
}

// The engine exists (first show): it reports its changes from now on.
void MapView::onCreated() {
    QMapLibre::Map *m = engine();
    connect(m, &QMapLibre::Map::mapChanged, this, [this](QMapLibre::Map::MapChange c) {
        switch (c) {
        case QMapLibre::Map::MapChangeDidFinishLoadingStyle:
            onStyleLoaded();
            break;
        case QMapLibre::Map::MapChangeRegionIsChanging:
            scale->update();
            emit viewChanged(false);
            break;
        case QMapLibre::Map::MapChangeRegionDidChange:
        case QMapLibre::Map::MapChangeRegionDidChangeAnimated:
            scale->update();
            emit viewChanged(true);
            break;
        default:
            break;
        }
    });
    connect(m, &QMapLibre::Map::copyrightsChanged, this, [this](const QString &html) {
        attribution->setText("<a href=\"https://maplibre.org\">MapLibre</a> | " + html);
        placeControls();
    });
    emit loaded();
}

void MapView::start(GeoPoint centre, double zoom, const QString &name) {
    QMapLibre::Map *m = engine();
    if (!m) return;
    m->setCoordinateZoom({centre.lat, centre.lon}, zoom - 1);  // MapLibre counts 512-pixel tiles
    setLayer(name);
}

void MapView::setLayer(const QString &name) {
    layer_ = layers().first().name;
    for (const Layer &l : layers()) {
        if (l.name == name) layer_ = name;
    }
    applyStyle();
}

void MapView::applyStyle() {
    QMapLibre::Map *m = engine();
    if (!m) return;
    for (const Layer &l : layers()) {
        if (l.name != layer_) continue;
        attribution->setText(l.vector ? QString() : "<a href=\"https://maplibre.org\">MapLibre</a> | " + l.attribution);
        placeControls();
        if (l.vector) m->setStyleUrl(l.url);
        else m->setStyleJson(rasterStyle(l));
    }
}

// A new style has none of ours: the marker images, the overlay sources and their layers again.
void MapView::onStyleLoaded() {
    QMapLibre::Map *m = engine();
    for (char c : {'s', 'f', 'p', 'a', 'b'}) {
        m->addImage(QStringLiteral("rv-%1").arg(c), QImage(QStringLiteral(":/original/html/GM_MARKER%1.PNG").arg(QChar(c).toUpper())));
    }
    for (const char *s : kSources) m->addSource(QString::fromLatin1(s), {{"type", "geojson"}, {"data", data.value(QString::fromLatin1(s))}});
    addOverlayLayers();
    if (!ready_) {
        ready_ = true;
        emit inited();
    }
    emit viewChanged(true);
}

void MapView::addOverlayLayers() {
    QMapLibre::Map *m = engine();
    const QVariantMap round{{"line-join", "round"}, {"line-cap", "round"}};
    auto line = [&](const QString &id, const QString &source, const QString &colour, double width, double opacity) {
        m->addLayer(id, {{"type", "line"}, {"source", source}, {"layout", round},
                         {"paint", QVariantMap{{"line-color", colour}, {"line-width", width}, {"line-opacity", opacity}}}});
    };
    auto fill = [&](const QString &id, const QString &source, const QString &colour, double opacity) {
        m->addLayer(id, {{"type", "fill"}, {"source", source}, {"paint", QVariantMap{{"fill-color", colour}, {"fill-opacity", opacity}}}});
    };
    auto symbols = [&](const QString &id) {
        m->addLayer(id, {{"type", "symbol"},
                         {"source", id},
                         {"layout", QVariantMap{{"icon-image", QVariantList{"get", "icon"}},
                                                {"icon-anchor", "bottom"},
                                                {"icon-allow-overlap", true},
                                                {"icon-ignore-placement", true}}}});
    };
    auto dots = [&](const QString &id, const QString &colour, double radius) {
        m->addLayer(id, {{"type", "circle"},
                         {"source", id},
                         {"paint", QVariantMap{{"circle-radius", radius},
                                               {"circle-color", colour},
                                               {"circle-stroke-color", "#0033FF"},
                                               {"circle-stroke-width", 2}}}});
    };
    line("rv-overlay-lines", "rv-overlay-lines", "#0033FF", 4, 0.7);
    dots("rv-overlay-points", "#0033FF", 4);
    // AddPath: weight 6, opacity 0.4.
    line("rv-gaps", "rv-gaps", "#444444", 6, 0.4);
    line("rv-route", "rv-route", "#FF0000", 6, 0.4);
    line("rv-selected", "rv-selected", "#FF00FF", 6, 0.4);
    symbols("rv-pmarks");
    symbols("rv-marks");
    // PlaceMarkerD: half-transparent, no outline; PlaceMarker: opaque with a black 2 px outline.
    fill("rv-circle", "rv-circle", "#00FF00", 0.5);
    fill("rv-car", "rv-car", "#00FF00", 1);
    line("rv-car-outline", "rv-car", "#000000", 2, 1);
    line("rv-ruler", "rv-ruler", "#0033FF", 4, 0.6);
    dots("rv-ruler-points", "#FFFFFF", 5);
}

void MapView::setData(const QString &source, const QByteArray &geojson) {
    if (data.value(source) == geojson) return;
    data.insert(source, geojson);
    QMapLibre::Map *m = engine();
    if (m && ready_ && m->sourceExists(source)) m->updateSource(source, {{"data", geojson}});
}

void MapView::setPaths(const QVector<Path> &paths) {
    paths_ = paths;
    QByteArrayList route, gaps, selected;
    for (const Path &p : paths) {
        if (p.points.size() < 2) continue;
        (p.kind == Path::Gap ? gaps : p.kind == Path::Selected ? selected : route).append(lineFeature(p.points));
    }
    setData("rv-route", collection(route));
    setData("rv-gaps", collection(gaps));
    setData("rv-selected", collection(selected));
}

// The time points in a layer of their own; S and F share the one above with A and B.
void MapView::setPathMarkers(const QVector<Marker> &marks) {
    QByteArrayList p;
    endpoints.clear();
    for (const Marker &m : marks) (m.icon == 'p' ? p : endpoints).append(pointFeature(m.at, m.icon));
    setData("rv-pmarks", collection(p));
    setData("rv-marks", collection(endpoints + abMarks));
}

void MapView::setAB(const std::optional<Marker> &a, const std::optional<Marker> &b) {
    abMarks.clear();
    if (a) abMarks.append(pointFeature(a->at, a->icon));
    if (b) abMarks.append(pointFeature(b->at, b->icon));
    setData("rv-marks", collection(endpoints + abMarks));
}

void MapView::setCarPolygon(const QVector<GeoPoint> &polygon) {
    setData("rv-circle", collection({}));
    setData("rv-car", collection({polygonFeature(polygon)}));
}

void MapView::setCarCircle(GeoPoint centre, double radiusMetres) {
    QVector<GeoPoint> ring;
    for (int i = 0; i < 36; ++i) ring.append(destination(centre, i * 10.0, radiusMetres));
    setData("rv-car", collection({}));
    setData("rv-circle", collection({polygonFeature(ring)}));
}

void MapView::clearCar() {
    setData("rv-car", collection({}));
    setData("rv-circle", collection({}));
}

void MapView::clearAll() {
    abMarks.clear();
    setPaths({});
    setPathMarkers({});
    clearCar();
}

void MapView::panTo(GeoPoint p) {
    QMapLibre::Map *m = engine();
    if (!m) return;
    pan->stop();
    const QMapLibre::Coordinate c = m->coordinate();
    pan->setStartValue(QPointF(c.first, c.second));
    pan->setEndValue(QPointF(p.lat, p.lon));
    pan->start();
}

void MapView::fitBounds(GeoPoint sw, GeoPoint ne) {
    QMapLibre::Map *m = engine();
    if (!m) return;
    pan->stop();
    m->resize(canvas->size());
    const QMapLibre::CoordinateZoom cz = m->coordinateZoomForBounds({sw.lat, sw.lon}, {ne.lat, ne.lon});
    // Leaflet's fitBounds snaps down to a whole zoom level (in its scale), leaving a margin.
    m->setCoordinateZoom(cz.first, std::floor(cz.second + 1 + 1e-9) - 1);
}

void MapView::zoomBy(int steps, QPointF anchor) {
    QMapLibre::Map *m = engine();
    if (!m || steps == 0) return;
    pan->stop();
    m->scaleBy(std::pow(2.0, steps), anchor);
}

bool MapView::showOverlay(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return false;
    QVector<QVector<GeoPoint>> lines;
    QVector<GeoPoint> points;
    readOverlay(f, lines, points);
    QByteArrayList lf, pf;
    double s = 90, w = 180, n = -90, e = -180;
    auto extend = [&](GeoPoint p) {
        s = std::min(s, p.lat);
        n = std::max(n, p.lat);
        w = std::min(w, p.lon);
        e = std::max(e, p.lon);
    };
    for (const auto &l : lines) {
        lf.append(lineFeature(l));
        for (GeoPoint p : l) extend(p);
    }
    for (GeoPoint p : points) {
        pf.append(pointFeature(p));
        extend(p);
    }
    setData("rv-overlay-lines", collection(lf));
    setData("rv-overlay-points", collection(pf));
    if (lf.isEmpty() && pf.isEmpty()) return false;
    fitBounds({s, w}, {n, e});
    return true;
}

MapView::View MapView::view() const {
    View v;
    QMapLibre::Map *m = engine();
    if (!m || canvas->width() <= 0 || canvas->height() <= 0) return v;
    const double w = canvas->width(), h = canvas->height();
    v.north = -90;
    v.south = 90;
    v.west = 180;
    v.east = -180;
    for (QPointF c : {QPointF(0, 0), QPointF(w, 0), QPointF(0, h), QPointF(w, h)}) {
        const GeoPoint p = coordinateFor(c);
        v.north = std::max(v.north, p.lat);
        v.south = std::min(v.south, p.lat);
        v.west = std::min(v.west, p.lon);
        v.east = std::max(v.east, p.lon);
    }
    const QMapLibre::Coordinate c = m->coordinate();
    v.centre = {c.first, c.second};
    v.zoom = m->zoom() + 1;
    v.size = canvas->size();
    v.metresPerPixel = distance(coordinateFor({0, h / 2}), coordinateFor({w, h / 2})) / w;
    v.valid = v.east > v.west && v.north > v.south;
    return v;
}

QPointF MapView::pixelFor(GeoPoint p) const {
    QMapLibre::Map *m = engine();
    return m ? m->pixelForCoordinate({p.lat, p.lon}) : QPointF();
}

GeoPoint MapView::coordinateFor(QPointF pixel) const {
    QMapLibre::Map *m = engine();
    if (!m) return {};
    const QMapLibre::Coordinate c = m->coordinateForPixel(pixel);
    return {c.first, c.second};
}

void MapView::canvasClick(QPointF pixel) {
    const GeoPoint at = coordinateFor(pixel);
    if (rulerOn) {
        ruler.append(at);
        updateRuler();
        return;
    }
    emit clicked(at, pixel);
}

void MapView::setRulerActive(bool on) {
    rulerOn = on;
    rulerClear->setVisible(on);
    rulerText->setVisible(on);
    canvas->setCursor(on ? Qt::CrossCursor : Qt::ArrowCursor);
    rulerBox->adjustSize();
    updateRuler();
}

double MapView::rulerMetres() const {
    double d = 0;
    for (int i = 1; i < ruler.size(); ++i) d += distance(ruler[i - 1], ruler[i]);
    return d;
}

namespace {
QString distanceText(double d) {
    return d < 2000 ? QStringLiteral("%1 m").arg(std::lround(d)) : QStringLiteral("%1 km").arg(d / 1000, 0, 'f', 1);
}
}  // namespace

void MapView::updateRuler() {
    rulerText->setText(distanceText(rulerMetres()));
    rulerBox->adjustSize();
    QByteArrayList pts;
    for (GeoPoint p : ruler) pts.append(pointFeature(p));
    setData("rv-ruler", collection(ruler.size() >= 2 ? QByteArrayList{lineFeature(ruler)} : QByteArrayList{}));
    setData("rv-ruler-points", collection(pts));
}

// A vertex of the distance line under the mouse shows the distance up to it; over the route the
// cursor is a hand (the page's clickable lines), as the route is clicked to seek.
bool MapView::eventFilter(QObject *watched, QEvent *ev) {
    if (watched == canvas && ev->type() == QEvent::MouseMove) {
        auto *me = static_cast<QMouseEvent *>(ev);
        const QPointF pos = me->position();
        if (!ruler.isEmpty()) {
            double d = 0;
            for (int i = 0; i < ruler.size(); ++i) {
                if (i) d += distance(ruler[i - 1], ruler[i]);
                if (QLineF(pixelFor(ruler[i]), pos).length() <= 6) {
                    QToolTip::showText(me->globalPosition().toPoint(), distanceText(d), canvas);
                    return false;
                }
            }
            QToolTip::hideText();
        }
        if (!rulerOn && me->buttons() == Qt::NoButton) canvas->setCursor(routeAt(pos) ? Qt::PointingHandCursor : Qt::ArrowCursor);
    }
    return QWidget::eventFilter(watched, ev);
}

bool MapView::routeAt(QPointF pixel) const {
    if (paths_.isEmpty() || !engine()) return false;
    const double mpp = view().metresPerPixel;
    if (mpp <= 0) return false;
    // In metres around the point: the distance to each leg.
    const GeoPoint c = coordinateFor(pixel);
    const double kx = 111320.0 * std::cos(c.lat * M_PI / 180), ky = 110540.0, tolerance = 6 * mpp;
    for (const Path &p : paths_) {
        for (int i = 1; i < p.points.size(); ++i) {
            const double ax = (p.points[i - 1].lon - c.lon) * kx, ay = (p.points[i - 1].lat - c.lat) * ky;
            const double dx = (p.points[i].lon - c.lon) * kx - ax, dy = (p.points[i].lat - c.lat) * ky - ay;
            const double len2 = dx * dx + dy * dy;
            const double t = len2 > 0 ? std::clamp(-(ax * dx + ay * dy) / len2, 0.0, 1.0) : 0.0;
            if (std::hypot(ax + t * dx, ay + t * dy) <= tolerance) return true;
        }
    }
    return false;
}

// From the layer menu.
bool MapView::chooseLayer(const QString &name) {
    for (const Layer &l : layers()) {
        if (l.name != name) continue;
        if (!l.keyService.isEmpty() && apiKey(l.keyService).isEmpty() && !askApiKey(l.keyService)) return false;
        setLayer(name);
        emit layerChanged(name);
        return true;
    }
    return false;
}

// Q-28: asks for a provider's key (with where to get one); an empty key removes it.
bool MapView::askApiKey(const QString &service) {
    const bool ru = Lang::instance().current() == "ru";
    QInputDialog dialog(this);
    dialog.setWindowTitle(ru ? QStringLiteral("Ключ API CARTO") : QStringLiteral("CARTO API key"));
    dialog.setLabelText(ru ? QStringLiteral("Карты CARTO работают с бесплатным ключом API:<br>"
                                            "<a href=\"https://carto.com/basemaps/apikey\">carto.com/basemaps/apikey</a><br><br>Ключ:")
                           : QStringLiteral("CARTO maps need a free API key:<br>"
                                            "<a href=\"https://carto.com/basemaps/apikey\">carto.com/basemaps/apikey</a><br><br>Key:"));
    for (QLabel *label : dialog.findChildren<QLabel *>()) {
        label->setTextFormat(Qt::RichText);
        label->setOpenExternalLinks(true);
    }
    dialog.setTextValue(apiKey(service));
    if (dialog.exec() != QDialog::Accepted) return false;
    const QString key = dialog.textValue().trimmed();
    setApiKey(service, key);
    emit apiKeyChanged(service, key);
    // A layer of theirs on show fetched placeholders without the key: its style again.
    for (const Layer &l : layers()) {
        if (l.name == layer_ && l.keyService == service) applyStyle();
    }
    return !key.isEmpty();
}

void MapView::resizeEvent(QResizeEvent *e) {
    QWidget::resizeEvent(e);
    placeControls();
}

// Leaflet's corners: zoom and the ruler top left, layers top right, scale bottom left,
// attribution bottom right.
void MapView::placeControls() {
    zoomBox->move(10, 10);
    rulerBox->move(10, zoomBox->geometry().bottom() + 11);
    layerButton->move(width() - layerButton->width() - 10, 10);
    scale->move(10, height() - scale->height() - 6);
    attribution->adjustSize();
    attribution->setMaximumWidth(std::max(50, width() - scale->width() - 30));
    attribution->adjustSize();
    attribution->move(width() - attribution->width(), height() - attribution->height());
}
