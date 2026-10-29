// The map (TWebBrowser "WebBrowser" in the original, which hosted a Leaflet page): a native
// MapLibre map with vector and raster base layers. The route, its markers, the car and the
// distance tool are map layers; the controls the page had (zoom, layers, ruler, scale,
// attribution) are widgets over it.
// Spec: graphs-map-layout.md §3.11 (GM-150…GM-176), decisions.md Q-10 and Q-27.
#pragma once

#include <QByteArrayList>
#include <QHash>
#include <QPointF>
#include <QSize>
#include <QString>
#include <QVector>
#include <QWidget>

#include <optional>

class QLabel;
class QToolButton;
class QVariantAnimation;
namespace QMapLibre {
class Map;
}

struct GeoPoint {
    double lat = 0, lon = 0;
};

class MapView : public QWidget {
    Q_OBJECT
public:
    struct Layer {
        QString name;   // shown in the layer menu and saved (gm_L)
        bool vector;    // a MapLibre style URL, or a raster tile URL template
        QString url;          // raster: several layers separated by '|', {s} for subdomains
        QString attribution;  // raster layers; vector styles carry their own
        int maxZoom = 19;     // raster tiles exist up to this zoom
    };
    // Q-27: the base layers; the first is the default.
    static const QVector<Layer> &layers();

    // A line of the route or of the route preview (AddPath).
    struct Path {
        enum Kind { Route, Gap, Selected };
        QVector<GeoPoint> points;
        Kind kind = Route;
    };
    // A marker image anchored at its bottom centre (AddPathMarker, SetAB): 's', 'f', 'p', 'a', 'b'.
    struct Marker {
        char icon = 'p';
        GeoPoint at;
    };
    // What the map shows (GM-152 "bounds"); the zoom is in the Leaflet scale the settings keep.
    struct View {
        double north = 0, south = 0, west = 0, east = 0;
        GeoPoint centre;
        double zoom = 0;
        QSize size;
        double metresPerPixel = 0;
        bool valid = false;
    };

    explicit MapView(QWidget *parent = nullptr);
    ~MapView() override;

    // GM-152 "loaded" → initialize: the first view and base layer.
    void start(GeoPoint centre, double zoom, const QString &layer);
    // The map exists and its first style is loaded (the page's "inited").
    bool isReady() const { return ready_; }
    QString layer() const { return layer_; }
    void setLayer(const QString &name);

    // GM-153.
    void setPaths(const QVector<Path> &paths);                                   // ClearPath, AddPath
    void setPathMarkers(const QVector<Marker> &marks);                           // AddPathMarker
    void setAB(const std::optional<Marker> &a, const std::optional<Marker> &b);  // SetAB, ClearMarkersAB
    void setCarPolygon(const QVector<GeoPoint> &polygon);                        // PlaceMarker
    void setCarCircle(GeoPoint centre, double radiusMetres);                     // PlaceMarkerD
    void clearCar();                                                             // ClearMarkers
    void clearAll();
    void panTo(GeoPoint p);  // animated
    void fitBounds(GeoPoint southWest, GeoPoint northEast);
    void zoomBy(int steps, QPointF anchor);  // 2^steps, the anchor staying in place
    // GM-172: the lines and points of a KML or GPX file over the map, fitted; false if none.
    bool showOverlay(const QString &path);

    View view() const;
    QPointF pixelFor(GeoPoint p) const;
    GeoPoint coordinateFor(QPointF pixel) const;
    // The engine, for tests; null until the widget was first shown.
    QMapLibre::Map *engine() const;
    // The distance tool's total (metres) and text, for tests.
    double rulerMetres() const;

signals:
    void loaded();                    // the engine exists; start() is expected
    void inited();                    // the first style finished loading
    void viewChanged(bool finished);  // while moving, and once it stops
    void layerChanged(const QString &name);
    void clicked(GeoPoint at, QPointF pixel);  // a left click without a drag (routeClick)

protected:
    void resizeEvent(QResizeEvent *e) override;
    bool eventFilter(QObject *watched, QEvent *ev) override;

private:
    class Canvas;
    class ScaleBar;
    void onCreated();
    void onStyleLoaded();
    void applyStyle();
    void addOverlayLayers();
    void setData(const QString &source, const QByteArray &geojson);
    void placeControls();
    void setRulerActive(bool on);
    void updateRuler();
    void canvasClick(QPointF pixel);

    Canvas *canvas = nullptr;
    QWidget *zoomBox = nullptr, *rulerBox = nullptr;
    QToolButton *rulerButton = nullptr, *rulerClear = nullptr, *layerButton = nullptr;
    QLabel *rulerText = nullptr, *attribution = nullptr;
    ScaleBar *scale = nullptr;
    QVariantAnimation *pan = nullptr;

    QString layer_;
    bool ready_ = false;
    QHash<QString, QByteArray> data;  // GeoJSON of each overlay source, kept across style changes
    QByteArrayList endpoints, abMarks;  // S and F, A and B
    QVector<GeoPoint> ruler;
    bool rulerOn = false;
};
