#include "MapView.h"

#include <QApplication>
#include <QBuffer>
#include <QDesktopServices>
#include <QFile>
#include <QUrlQuery>
#include <QWebEnginePage>
#include <QWebEngineProfile>
#include <QWebEngineSettings>
#include <QWebEngineUrlRequestJob>
#include <QWebEngineUrlScheme>
#include <QWebEngineUrlSchemeHandler>

#include <functional>

namespace {

const QByteArray kScheme = "rvmap";

// Q-10: of the original's base layers only OpenStreetMap-based ones that still work without a
// key remain. "Open Steet Map" [sic] is the page's fallback layer name and is kept as the key.
const char *const kLayers = R"JS(layers = {
    "Open Steet Map": L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {maxZoom: 19, attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'}),
    "OpenTopoMap": L.tileLayer('https://{s}.tile.opentopomap.org/{z}/{x}/{y}.png', {subdomains: 'abc', maxZoom: 17, attribution: '&copy; OpenStreetMap contributors, SRTM | &copy; <a href="https://opentopomap.org">OpenTopoMap</a> (CC-BY-SA)'}),
    "CyclOSM": L.tileLayer('https://{s}.tile-cyclosm.openstreetmap.fr/cyclosm/{z}/{x}/{y}.png', {subdomains: 'abc', maxZoom: 20, attribution: '&copy; OpenStreetMap contributors, <a href="https://www.cyclosm.org">CyclOSM</a>'})
  };)JS";

// The original page with its dead endpoints replaced: https for the libraries, the working
// layers, and no link to the defunct help site.
QByteArray adaptPage(QByteArray html) {
    QString s = QString::fromUtf8(html);
    s.replace("http://cdnjs.cloudflare.com/", "https://cdnjs.cloudflare.com/");
    s.replace("http://leaflet.github.io/Leaflet.draw/leaflet.draw.css",
              "https://cdnjs.cloudflare.com/ajax/libs/leaflet.draw/0.2.3/leaflet.draw.css");
    s.replace("http://leaflet.github.io/Leaflet.draw/leaflet.draw.js",
              "https://cdnjs.cloudflare.com/ajax/libs/leaflet.draw/0.2.3/leaflet.draw.js");
    const qsizetype from = s.indexOf("layers = {");
    const qsizetype to = s.indexOf("uplayers = {");
    if (from >= 0 && to > from) s.replace(from, to - from, QString::fromUtf8(kLayers) + "\n  ");
    // The distance tool needs Leaflet.draw; without it the map must still start.
    s.replace("map.addControl(new L.Control.Distance());",
              "try { map.addControl(new L.Control.Distance()); } catch (e) { }");
    const qsizetype help = s.indexOf("'I have the Internet,");
    if (help >= 0) s.remove(help, s.indexOf('\n', help) - help);
    return s.toUtf8();
}

QByteArray mimeFor(const QString &name) {
    if (name.endsWith(".HTML")) return "text/html";
    if (name.endsWith(".JS")) return "text/javascript";
    if (name.endsWith(".CSS")) return "text/css";
    if (name.endsWith(".PNG")) return "image/png";
    if (name.endsWith(".CUR")) return "image/x-icon";
    return "application/octet-stream";
}

// rvmap://local/<file>: the original map files, stored upper-case in the resources.
class MapSchemeHandler : public QWebEngineUrlSchemeHandler {
public:
    using QWebEngineUrlSchemeHandler::QWebEngineUrlSchemeHandler;
    void requestStarted(QWebEngineUrlRequestJob *job) override {
        const QString name = job->requestUrl().path().mid(1).toUpper();
        QFile f(":/original/html/" + name);
        if (name.isEmpty() || name.contains('/') || !f.open(QIODevice::ReadOnly)) {
            job->fail(QWebEngineUrlRequestJob::UrlNotFound);
            return;
        }
        QByteArray data = f.readAll();
        if (name == "OSM2.HTML") data = adaptPage(data);
        auto *buffer = new QBuffer(job);
        buffer->setData(data);
        buffer->open(QIODevice::ReadOnly);
        job->reply(mimeFor(name), buffer);
    }
};

QWebEngineProfile *mapProfile() {
    static QWebEngineProfile *profile = [] {
        auto *p = new QWebEngineProfile(QStringLiteral("map"), qApp);
        p->setHttpCacheType(QWebEngineProfile::DiskHttpCache);  // tiles are reused, not re-fetched
        // OpenStreetMap tile usage policy: the user agent identifies the application.
        p->setHttpUserAgent(p->httpUserAgent() + QStringLiteral(" RVPlayer/0.1"));
        p->installUrlSchemeHandler(kScheme, new MapSchemeHandler(p));
        return p;
    }();
    return profile;
}

}  // namespace

class MapPage : public QWebEnginePage {
public:
    MapPage(QWebEngineProfile *profile, MapView *view, std::function<void(const QUrl &)> onCallback)
        : QWebEnginePage(profile, view), onCallback(std::move(onCallback)) {}

protected:
    bool acceptNavigationRequest(const QUrl &url, NavigationType type, bool isMainFrame) override {
        if (url.scheme() == "http" && url.host() == "callback") {
            onCallback(url);
            return false;  // GM-152: callbacks are cancelled navigations
        }
        if (type == NavigationTypeLinkClicked && url.scheme().startsWith("http")) {
            QDesktopServices::openUrl(url);  // attribution links open in the system browser
            return false;
        }
        return QWebEnginePage::acceptNavigationRequest(url, type, isMainFrame);
    }

    QWebEnginePage *createWindow(WebWindowType) override {
        return nullptr;
    }

private:
    std::function<void(const QUrl &)> onCallback;
};

void MapView::registerScheme() {
    QWebEngineUrlScheme scheme(kScheme);
    scheme.setSyntax(QWebEngineUrlScheme::Syntax::Host);
    scheme.setFlags(QWebEngineUrlScheme::SecureScheme | QWebEngineUrlScheme::CorsEnabled);
    QWebEngineUrlScheme::registerScheme(scheme);
}

MapView::MapView(QWidget *parent) : QWebEngineView(parent) {
    setPage(new MapPage(mapProfile(), this, [this](const QUrl &url) { onCallback(url); }));
    setContextMenuPolicy(Qt::NoContextMenu);
    connect(this, &QWebEngineView::loadFinished, this, [this](bool) {
        loaded_ = true;
        emit documentComplete();
    });
}

void MapView::navigate(int provider) {
    if (provider == provider_) return;
    provider_ = provider;
    inited_ = false;
    loaded_ = false;
    setUrl(provider == 0 ? QUrl("about:blank") : QUrl("rvmap://local/osm2.html"));
}

bool MapView::exec(const QString &script, bool force) {
    if ((!inited_ && !force) || !isVisible() || !loaded_) return false;
    page()->runJavaScript(script);
    return true;
}

void MapView::onCallback(const QUrl &url) {
    const QString cmd = url.path().mid(1);
    QHash<QString, QString> params;
    for (const QString &pair : url.query(QUrl::FullyEncoded).split('&', Qt::SkipEmptyParts)) {
        const qsizetype eq = pair.indexOf('=');
        if (eq < 0) continue;
        QString value = pair.mid(eq + 1);
        if (pair.left(eq) == "mt") value.replace("%20", " ");
        params.insert(pair.left(eq), value);
    }
    if (cmd == "loaded") loaded_ = true;  // body onload: the document is complete
    if (cmd == "inited") inited_ = true;
    emit callback(cmd, params);
}
