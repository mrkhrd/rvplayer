// The map browser (TWebBrowser "WebBrowser" in the original): hosts the original Leaflet page
// OSM2.HTML and speaks its protocol. The page calls the application by navigating to
// http://callback/<cmd>?<params>; the application calls the page's JS API.
// Spec: graphs-map-layout.md §3.11 (GM-150…GM-176), narrowed by decisions.md Q-10.
#pragma once

#include <QHash>
#include <QWebEngineView>

class MapView : public QWebEngineView {
    Q_OBJECT
public:
    // Must run before the QApplication is created: registers the rvmap: scheme that serves the
    // original map pages from the resources.
    static void registerScheme();

    explicit MapView(QWidget *parent = nullptr);

    // GM-150: 0 = none (blank page), 5 = OpenStreetMap (Leaflet). Reloads only on a change.
    void navigate(int provider);
    int provider() const { return provider_; }
    bool inited() const { return inited_; }

    // GM-151: runs a script only when the map is initialised (or `force`), visible and loaded.
    bool exec(const QString &script, bool force = false);

signals:
    // A callback the page sent: cmd is "loaded", "inited", "view", "bounds" or "routeClick".
    void callback(const QString &cmd, const QHash<QString, QString> &params);
    void documentComplete();

private:
    void onCallback(const QUrl &url);

    int provider_ = -1;
    bool inited_ = false;
    bool loaded_ = false;
};
