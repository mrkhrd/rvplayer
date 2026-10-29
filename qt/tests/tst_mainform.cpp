// UI tests of the main window: they drive the real form (loaded from the original DFM) with mouse
// clicks, menu actions and wheel events, the way a user would, and check what the spec says
// should happen. They also measure how long the UI thread is busy for each action, so stalls
// show up as failures.
//
// Data: three short recordings are copied once from the sample share into
// %TEMP%\rvplayer-tests\samples (the share is only read). Set RV_TEST_DIR to run on another
// folder instead, for example the share itself to include network delays. Without data the
// tests are skipped. Settings go to a separate INI file under %TEMP%\rvplayer-tests.

#include "app/Startup.h"
#include "core/GpsFiles.h"
#include "core/Settings.h"
#include "core/Exif.h"
#include "core/Playback.h"
#include "filelist/FileListView.h"
#include "forms/FormAbout.h"
#include "forms/FormCopy.h"
#include "forms/FormCropping.h"
#include "forms/FormDetails.h"
#include "forms/FormWhatsNew.h"
#include "forms/FormFileProperty.h"
#include "forms/FormGPSLog.h"
#include "forms/FormGroup.h"
#include "forms/FormRepair.h"
#include "forms/FormSetupSubtitleTemplate.h"
#include "forms/MainForm.h"
#include "i18n/Lang.h"
#include "map/MapView.h"
#include "rv_ffi.h"
#include "video/VideoView.h"

#include <QAbstractButton>
#include <QAction>
#include <QCheckBox>
#include <QApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QHeaderView>
#include <QInputDialog>
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMouseEvent>
#include <QOpenGLWidget>
#include <QToolButton>
#include <QMenu>
#include <QPlainTextEdit>
#include <QMessageBox>
#include <QProgressBar>
#include <QSettings>
#include <QTableWidget>
#include <QTimeZone>
#include <QTimer>
#include <QTreeWidget>
#include <QTest>
#include <QWheelEvent>

#include <QMapLibre/Map>

#include <functional>

namespace {

const char *const kShare = R"(\\192.168.6.47\p\Video\reg)";
const char *const kSamples[] = {
    "Fragment of 05161259_0066 (8) (13-06-26 - 13-06-31).MOV",
    "Fragment of 163641AA (2) (16-38-43 - 16-38-48).MP4",
    "Fragment of 132015AA (9) (13-57-23 - 13-57-35).MP4",
};

QString testRoot() {
    return QDir::temp().filePath("rvplayer-tests");
}

// The folder the tests open: RV_TEST_DIR, or local copies of the samples.
QString prepareData() {
    if (qEnvironmentVariableIsSet("RV_TEST_DIR")) return qEnvironmentVariable("RV_TEST_DIR");
    const QString dir = QDir(testRoot()).filePath("samples");
    QDir().mkpath(dir);
    for (const char *name : kSamples) {
        const QString to = QDir(dir).filePath(name);
        if (QFile::exists(to)) continue;
        if (!QFile::copy(QDir(kShare).filePath(name), to)) return {};
    }
    return dir;
}

// RV_TEST_SHOTS=<folder>: windows are saved as PNG at checkpoints, for looking at the result.
void shot(QWidget *w, const char *name) {
    if (!qEnvironmentVariableIsSet("RV_TEST_SHOTS")) return;
    QApplication::processEvents();
    w->grab().save(QDir(qEnvironmentVariable("RV_TEST_SHOTS")).filePath(QString::fromLatin1(name) + ".png"));
}

qint64 timed(const std::function<void()> &f) {
    QElapsedTimer t;
    t.start();
    f();
    return t.elapsed();
}

// The UI thread may be busy at most this long for one user action.
constexpr qint64 kMaxBlockMs = 300;

}  // namespace

class MainFormTest : public QObject {
    Q_OBJECT
public:
    explicit MainFormTest(QString data) : data(std::move(data)) {}

private slots:
    void initTestCase();
    void startupPlaysFirstRow();
    void clickRowOpensPaused();
    void seekBarClickIsResponsive();
    void doubleClickPlays();
    void headerClickSorts();
    void fullScreenToggles();
    void videoZoomInAndBack();
    void wheelOverGraphsZoomsTimeline();
    void languageSwitchRetranslates();
    void propertyWindowShowsTheRow();
    void hoverOverGraphShowsDetails();
    void aboutShowsCredits();
    void whatsNewShowsNotes();
    void snapshotJpegCarriesExif();
    void coordinateTemplateFillsValues();
    void flipXNegatesLongitude();
    void routeClickSeeksToThePoint();
    void mapShowsRouteOnAnyLayer();
    void mapRulerMeasures();
    void groupWithJoinsTickedRows();
    void copyCopiesFilesWithTimes();
    void cuttingListsTheSelection();
    void cuttingExportsTheSelection();
    void repairPicksReference();
    void repairRebuildsLostHeader();
    void gpsConverterShowsStatistics();
    void exportTrackOfTheSelection();
    void subtitleTemplateIsEditable();
    void moveVideoShiftsTheRow();
    void attachTrackReplacesGps();
    void cleanupTestCase();

private:
    FileListView *list() const { return form->listView(); }
    QPoint rowCenter(int row) const {
        const QModelIndex i = list()->listModel()->index(row, 0);
        return list()->visualRect(i).center();
    }
    void clickRow(int row) { QTest::mouseClick(list()->viewport(), Qt::LeftButton, {}, rowCenter(row)); }
    void stop() { form->ButtonStopClick(nullptr); }
    // The map, shown and with its style loaded.
    MapView *mapReady() {
        if (!form->isChecked("N_View_OpenStreetMaps2")) form->action("N_View_OpenStreetMaps2")->trigger();
        MapView *map = form->mapView();
        if (!map || !QTest::qWaitFor([&] { return map->isVisible() && map->isReady(); }, 10000)) return nullptr;
        return map;
    }

    QString data;
    MainForm *form = nullptr;
};

void MainFormTest::initTestCase() {
    if (data.isEmpty()) QSKIP("no sample recordings (share not reachable and RV_TEST_DIR not set)");
    // A raster base layer: its style is local, so the map is ready without the network.
    Settings::instance().writeString("gm_L", "OpenStreetMap");
    form = new MainForm;
    QVERIFY2(form->load(":/original/forms/TFORMMAIN.dfm"), qPrintable(form->loadError()));
    form->showMaximized();
    QVERIFY(QTest::qWaitForWindowExposed(form));
    form->activateWindow();
    // FL-001: the folder on the command line is listed and its first row plays.
    // A folder of one's own (RV_TEST_DIR) may hold a single group; the samples are three rows.
    const int rows = qEnvironmentVariableIsSet("RV_TEST_DIR") ? 1 : 3;
    QTRY_VERIFY_WITH_TIMEOUT(list()->listModel()->rowCount() >= rows && !list()->listModel()->hasStatus(), 20000);
}

void MainFormTest::startupPlaysFirstRow() {
    QTRY_VERIFY_WITH_TIMEOUT(form->playback->state() == Playback::State::Running, 5000);
    QTRY_VERIFY_WITH_TIMEOUT(!form->video->frameSize().isEmpty(), 5000);
    QCOMPARE(list()->selectedRows(), QList<int>{0});
}

// PV-010/PV-011: selecting a row loads it; the click opens it paused with its first frame.
void MainFormTest::clickRowOpensPaused() {
    stop();
    QVERIFY(form->playback->group().isEmpty());
    const qint64 ms = timed([&] { clickRow(1); });
    qInfo("click on a row: UI busy %lld ms", ms);
    QVERIFY2(ms < kMaxBlockMs, "the UI thread was blocked by a row click");
    QCOMPARE(list()->selectedRows(), QList<int>{1});
    QVERIFY(!form->playback->group().isEmpty());
    QTRY_VERIFY_WITH_TIMEOUT(form->playback->active(), 2000);
    QCOMPARE(form->playback->state(), Playback::State::Paused);
    QTRY_VERIFY_WITH_TIMEOUT(form->playback->displayedTime() >= 0, 2000);
}

// PV-034: a press on the seek bar away from the pin seeks there at once.
void MainFormTest::seekBarClickIsResponsive() {
    auto *seekImage = form->find<QWidget>("ImageSeek");
    QVERIFY(seekImage);
    const int border = form->seek.border();
    const int x = border + (seekImage->width() - 2 * border) * 3 / 4;
    const QPoint at(x, seekImage->height() / 2);
    QElapsedTimer total;
    total.start();
    const int seeksBefore = form->playback->seekRequests();
    const qint64 ms = timed([&] { QTest::mouseClick(seekImage, Qt::LeftButton, {}, at); });
    qInfo("click on the seek bar: UI busy %lld ms", ms);
    QVERIFY2(ms < kMaxBlockMs, "the UI thread was blocked by a seek");
    // VCL delivers a click to one control: the panels under the seek bar must not seek again.
    QCOMPARE(form->playback->seekRequests() - seeksBefore, 1);
    const double target = form->seek.progressAt(x);
    QTRY_VERIFY_WITH_TIMEOUT(std::abs(form->playback->position() - target) < 0.05, 2000);
    qInfo("seek: frame at the new position after %lld ms", total.elapsed());
}

// PV-012: a double click plays the row.
void MainFormTest::doubleClickPlays() {
    auto *vp = list()->viewport();
    const QPoint at = rowCenter(2);
    auto state = [&](const char *step) {
        qInfo("%s: active %d state %d rows %s", step, form->playback->active(), int(form->playback->state()),
              qPrintable(QString::number(list()->selectedRows().value(0, -1))));
    };
    const qint64 ms = timed([&] {
        QTest::mousePress(vp, Qt::LeftButton, {}, at);
        state("press");
        QTest::mouseRelease(vp, Qt::LeftButton, {}, at);
        state("release");
        QTest::mouseDClick(vp, Qt::LeftButton, {}, at);
        state("double click");
    });
    qInfo("double click on a row: UI busy %lld ms", ms);
    QTest::qWait(500);
    auto *play = form->find<QAbstractButton>("ButtonPlay");
    qInfo("after double click: active %d state %d play enabled %d checked %d rows %s", form->playback->active(),
          int(form->playback->state()), play->isEnabled(), play->isChecked(),
          qPrintable(QString::number(list()->selectedRows().value(0, -1))));
    QTRY_VERIFY_WITH_TIMEOUT(form->playback->state() == Playback::State::Running, 3000);
    QCOMPARE(list()->selectedRows(), QList<int>{2});
}

// FL-025: a header click sorts by that column; a second click reverses it.
void MainFormTest::headerClickSorts() {
    QHeaderView *h = list()->header();
    auto names = [&] {
        QStringList n;
        for (const FileRow &r : list()->listModel()->fileRows().rows) n.append(r.name.toUpper());
        return n;
    };
    auto clickHeader = [&](int column) {
        const int x = h->sectionViewportPosition(column) + h->sectionSize(column) / 2;
        QTest::mouseClick(h->viewport(), Qt::LeftButton, {}, QPoint(x, h->height() / 2));
    };
    clickHeader(0);
    QStringList sorted = names();
    std::sort(sorted.begin(), sorted.end());
    QTRY_COMPARE_WITH_TIMEOUT(names(), sorted, 2000);
    clickHeader(0);
    std::reverse(sorted.begin(), sorted.end());
    QTRY_COMPARE_WITH_TIMEOUT(names(), sorted, 2000);
    clickHeader(2);  // back to the default: date, ascending
}

// PV-070: full screen moves the video into its own window and back.
void MainFormTest::fullScreenToggles() {
    QWidget *panel = form->find<QWidget>("PanelVideo");
    QWidget *home = panel->parentWidget();
    form->action("N_FullScreen")->trigger();
    QVERIFY(form->fullScreen);
    QTRY_VERIFY(form->fullScreen->isVisible());
    QCOMPARE(panel->parentWidget(), static_cast<QWidget *>(form->fullScreen));
    QTRY_VERIFY(panel->width() > home->width());
    form->action("N_FullScreen")->trigger();
    QTRY_VERIFY(!form->fullScreen->isVisible());
    QCOMPARE(panel->parentWidget(), home);
}

// PV-061: zoom in switches to Custom with a larger picture; Letterbox fits it again.
void MainFormTest::videoZoomInAndBack() {
    qInfo("before zoom: group %d active %d state %d", !form->playback->group().isEmpty(), form->playback->active(),
          int(form->playback->state()));
    QTRY_VERIFY(!form->video->frameSize().isEmpty());
    const QRectF fit = form->video->pictureRect();
    form->action("N_VideoZoom_ZoomIn")->trigger();
    QVERIFY(form->isChecked("N_VideoZoom_Custom"));
    QVERIFY(form->video->pictureRect().width() > fit.width());
    form->action("N_VideoZoom_Letterbox")->trigger();
    QVERIFY(form->isChecked("N_VideoZoom_Letterbox"));
    QCOMPARE(form->video->pictureRect(), fit);
}

// PV-075: the wheel over the graphs zooms the timeline around the position.
void MainFormTest::wheelOverGraphsZoomsTimeline() {
    auto *seekImage = form->find<QWidget>("ImageSeek");
    form->zoomL = 0;
    form->zoomR = 1;
    const QPoint local(seekImage->width() / 2, seekImage->height() / 2);
    const QPoint global = seekImage->mapToGlobal(local);
    QCursor::setPos(global);
    // The wheel goes to the widget under the pointer: nothing else may be on top there.
    form->raise();
    form->activateWindow();
    QTRY_COMPARE(QApplication::widgetAt(global), seekImage);
    QWheelEvent wheel(QPointF(local), QPointF(global), QPoint(), QPoint(0, 120), Qt::NoButton, Qt::NoModifier,
                      Qt::NoScrollPhase, false);
    QApplication::sendEvent(seekImage, &wheel);
    QVERIFY2(form->zoomR - form->zoomL < 0.99, "the timeline did not zoom in");
}

// TR-010: choosing a language re-translates the menus at once.
void MainFormTest::languageSwitchRetranslates() {
    QAction *file = form->action("N_File");
    QAction *en = form->action("N_View_Language_en");
    QAction *ru = form->action("N_View_Language_ru");
    QVERIFY(en && ru);
    const bool wasRu = form->isChecked(ru);
    en->trigger();
    QCOMPARE(file->text(), QStringLiteral("File"));
    ru->trigger();
    QCOMPARE(file->text(), QStringLiteral("Файл"));
    (wasRu ? ru : en)->trigger();
}

// FL-060: Property opens a window with the row's name, duration, date and file block.
void MainFormTest::propertyWindowShowsTheRow() {
    clickRow(0);
    const FileRow row = list()->listModel()->fileRows().rows.value(0);
    form->action("N_FileList_Property")->trigger();
    FormFileProperty *w = nullptr;
    QTRY_VERIFY((w = [] {
        for (QWidget *t : QApplication::topLevelWidgets()) {
            if (auto *p = qobject_cast<FormFileProperty *>(t); p && p->isVisible()) return p;
        }
        return static_cast<FormFileProperty *>(nullptr);
    }()) != nullptr);
    auto *g = w->find<QTableWidget>("StringGrid");
    QVERIFY(g && g->rowCount() >= 6);
    QCOMPARE(g->item(0, 1)->text(), row.name);
    QVERIFY(w->windowTitle().endsWith(row.name));
    bool hasFile = false;
    for (int r = 0; r < g->rowCount(); ++r) hasFile |= g->item(r, 1) && g->item(r, 1)->text().endsWith(".MOV", Qt::CaseInsensitive) | g->item(r, 1)->text().endsWith(".MP4", Qt::CaseInsensitive);
    QVERIFY2(hasFile, "no File row");
    // SF-FILEPROPERTY-002: the values take the rest of the width as soon as the window opens.
    QTRY_VERIFY2(g->columnWidth(0) + g->columnWidth(1) >= g->viewport()->width() - 1,
                 qPrintable(QStringLiteral("%1 + %2 < %3").arg(g->columnWidth(0)).arg(g->columnWidth(1)).arg(g->viewport()->width())));
    QVERIFY(g->columnWidth(1) > g->columnWidth(0));
    shot(w, "property-default");
    w->resize(560, 520);
    shot(w, "property");
    shot(form, "main");
    w->close();
}

// GM-120…GM-122: moving over a graph shows the values there, with a mini-graph; leaving hides it.
void MainFormTest::hoverOverGraphShowsDetails() {
    QVERIFY(!form->graphs.empty());
    QWidget *graph = form->graphs.front().image;
    const QPoint at(graph->width() / 2, graph->height() / 2);
    // Sent straight to the graph: a synthetic move through the window system depends on where
    // the real cursor and other windows are.
    QMouseEvent move(QEvent::MouseMove, QPointF(at), graph->mapToGlobal(QPointF(at)), Qt::NoButton, Qt::NoButton,
                     Qt::NoModifier);
    QApplication::sendEvent(graph, &move);
    QTRY_VERIFY(form->details && form->details->isVisible());
    {
        const Group &g = form->playback->group();
        const double p = form->detailsCenter;
        const MovingValue v = form->moving.at(g.timeAt(p));
        qInfo("hover p=%.3f t=%.6f gps=%d g=%d hasGps=%d hasG=%d files=%lld start=%.6f finish=%.6f", p, g.timeAt(p), v.gps,
              v.gsensor, form->moving.hasGps(), form->moving.hasGSensor(), static_cast<long long>(g.files.size()),
              g.files.value(0).start(), g.files.value(0).finish);
    }
    QVERIFY(!form->details->find<QLabel>("LabelSystemTime")->text().isEmpty());
    // The mini-graph needs about 20 timeline pixels to span at least half a second (GM-120).
    const double minZoom = 0.5 / (1 + form->playback->group().duration * 86400.0);
    QCOMPARE(form->details->find<QWidget>("Image")->isVisible(), form->detailsWidth >= minZoom);
    shot(form->details, "details");
    QEvent leave(QEvent::Leave);
    QApplication::sendEvent(graph, &leave);
    QVERIFY(!form->details->isVisible());
}

// SF-ABOUT-*: the box opens with the product name and the author, without the brand logo.
void MainFormTest::aboutShowsCredits() {
    form->action("N_Help_About")->trigger();
    QVERIFY(form->aboutForm && form->aboutForm->isVisible());
    QCOMPARE(form->aboutForm->find<QLabel>("LabelTitle")->text(), QStringLiteral("Registrator Viewer"));
    QVERIFY(!form->aboutForm->find<QWidget>("ImageLogo")->isVisible());
    shot(form->aboutForm, "about");
    form->aboutForm->close();
}

// SF-WHATSNEW-*: the release notes of the original, in the current language.
void MainFormTest::whatsNewShowsNotes() {
    form->action("N_Help_WhatsNew")->trigger();
    QVERIFY(form->whatsNewForm && form->whatsNewForm->isVisible());
    const QString text = form->whatsNewForm->find<QPlainTextEdit>("Memo")->toPlainText();
    QVERIFY(text.startsWith("Registrator Viewer 6.0.0.22"));
    QVERIFY(text.size() > 1000);
    form->whatsNewForm->close();
}

// PV-086: the snapshot JPEG decodes and carries the date and GPS position in EXIF.
void MainFormTest::snapshotJpegCarriesExif() {
    QImage img(64, 48, QImage::Format_RGB32);
    img.fill(Qt::darkCyan);
    exif::Info info;
    info.description = "Video frame of test";
    info.original = QDateTime(QDate(2016, 2, 13), QTime(18, 39, 24, 250));
    info.gps = exif::Info::Gps{60.2528, 30.3371, 12};
    const QByteArray jpg = exif::jpeg(img, 96, info);
    QVERIFY(!QImage::fromData(jpg, "JPG").isNull());
    QVERIFY(jpg.contains(QByteArray("Exif\0\0", 6)));
    QVERIFY(jpg.contains("2016:02:13 18:39:24"));
    QVERIFY(jpg.contains("Video frame of test"));
    QVERIFY(jpg.contains(QByteArray("N\0", 2)) && jpg.contains(QByteArray("E\0", 2)));
}

// GM-182: placeholders are replaced case-insensitively by the point's values.
void MainFormTest::coordinateTemplateFillsValues() {
    MovingValue v;
    v.gps = true;
    v.lat = 60.5;
    v.lon = 30.25;
    v.speed = 50.4;
    v.course = 90;
    v.track = 1234;
    v.alt = 12;
    QCOMPARE(form->fillCoordinatesTemplate("{y}, {X} {s} {a} {p} {h}", v), QStringLiteral("60.500000, 30.250000 50 90 1234 12"));
    // A typed "\n" (backslash, n) becomes CRLF.
    QCOMPARE(form->fillCoordinatesTemplate(QStringLiteral("a\\nb"), v), QStringLiteral("a\r\nb"));
}

// GM-101/GM-180: Flip X reloads the item with negated longitudes.
void MainFormTest::flipXNegatesLongitude() {
    clickRow(0);
    QTRY_VERIFY(form->moving.hasGps());
    const Group &g = form->playback->group();
    const double t = g.timeAt(0.5);
    const double lon = form->moving.at(t).lon;
    QVERIFY(lon != 0);
    form->action("N_View_Coord_FlipX")->trigger();
    QTRY_COMPARE(form->moving.at(form->playback->group().timeAt(0.5)).lon, -lon);
    form->action("N_View_Coord_FlipX")->trigger();
    QTRY_COMPARE(form->moving.at(form->playback->group().timeAt(0.5)).lon, lon);
}

// GM-152 routeClick: a click on the route seeks to the nearest point; one far from it does not.
void MainFormTest::routeClickSeeksToThePoint() {
    clickRow(0);
    QTRY_VERIFY(form->moving.hasGps());
    MapView *map = mapReady();
    QVERIFY(map);
    const auto segments = form->moving.routeSegments();
    QVERIFY(!segments.isEmpty() && segments.first().size() >= 3);
    const GpsSample p = segments.first()[segments.first().size() / 2];
    // About 2 m per pixel: 10 pixels are about 20 m.
    map->engine()->setCoordinateZoom({p.lat, p.lon}, 16);
    QTRY_VERIFY(form->viewport.valid && std::abs(form->viewport.y - p.lat) < 1e-5);
    QWidget *canvas = map->findChild<QOpenGLWidget *>();
    const Group &g = form->playback->group();
    form->seekTo(0, true);
    QTest::mouseClick(canvas, Qt::LeftButton, {}, (map->pixelFor({p.lat, p.lon}) + QPointF(3, 2)).toPoint());
    QVERIFY(form->lastPosition > 0);
    const double seconds = std::abs(form->lastPosition - g.progressAt(p.t)) * g.duration * 86400;
    QVERIFY2(seconds < 1.2, qPrintable(QString::number(seconds)));  // the point or a neighbour
    form->seekTo(0, true);
    QTest::mouseClick(canvas, Qt::LeftButton, {}, QPoint(4, canvas->height() - 4));
    QCOMPARE(form->lastPosition, 0.0);
}

// GM-161: the opened row's route is on the map, fitted; another base layer keeps it.
void MainFormTest::mapShowsRouteOnAnyLayer() {
    clickRow(0);
    QTRY_VERIFY(form->moving.hasGps());
    MapView *map = mapReady();
    QVERIFY(map);
    form->setRoute();
    const auto segments = form->moving.routeSegments();
    // Every point inside the view the map reports once it has moved (edge points within 1e-6°).
    const auto fitted = [&] {
        const auto &v = form->viewport;
        for (const auto &seg : segments) {
            for (const GpsSample &p : seg) {
                if (p.lat > v.top + 1e-6 || p.lat < v.bottom - 1e-6 || p.lon < v.left - 1e-6 || p.lon > v.right + 1e-6) return false;
            }
        }
        return v.valid;
    };
    QTRY_VERIFY(fitted());
    QVERIFY(map->engine()->layerExists("rv-route"));
    if (qEnvironmentVariableIsSet("RV_TEST_SHOTS")) {
        QTest::qWaitFor([&] { return map->engine()->isFullyLoaded(); }, 5000);  // a grab mid-update is torn
        shot(map, "map");
    }
    auto *layers = map->findChild<QToolButton *>("layerButton");
    QVERIFY(layers && layers->menu());
    QAction *satellite = nullptr;
    for (QAction *a : layers->menu()->actions()) {
        if (a->text() == "Satellite (Esri)") satellite = a;
    }
    QVERIFY(satellite);
    satellite->trigger();
    QCOMPARE(map->layer(), QString("Satellite (Esri)"));
    QCOMPARE(Settings::instance().readString("gm_L"), QString("Satellite (Esri)"));
    QTRY_VERIFY(map->engine()->layerExists("rv-route") && map->engine()->layerExists("base0"));
    map->setLayer("OpenStreetMap");
    Settings::instance().writeString("gm_L", "OpenStreetMap");
    QTRY_VERIFY(map->engine()->layerExists("rv-route"));
}

// GM-153 distance tool: the ruler adds a vertex per click and sums the legs; the cross clears.
void MainFormTest::mapRulerMeasures() {
    MapView *map = mapReady();
    QVERIFY(map);
    QWidget *canvas = map->findChild<QOpenGLWidget *>();
    auto *ruler = map->findChild<QToolButton *>("rulerButton");
    auto *text = map->findChild<QLabel *>("rulerText");
    QVERIFY(ruler && text);
    ruler->click();
    QVERIFY(text->isVisible());
    const QPoint a(canvas->width() / 3, canvas->height() / 2), b(2 * canvas->width() / 3, canvas->height() / 2);
    const qreal before = form->lastPosition;
    QTest::mouseClick(canvas, Qt::LeftButton, {}, a);
    QTest::mouseClick(canvas, Qt::LeftButton, {}, b);
    QCOMPARE(form->lastPosition, before);  // measuring does not seek
    const double expected = form->viewport.metresPerPixel * (b.x() - a.x());
    QVERIFY2(std::abs(map->rulerMetres() - expected) < expected * 0.02,
             qPrintable(QStringLiteral("%1 vs %2").arg(map->rulerMetres()).arg(expected)));
    QVERIFY(text->text().endsWith(" m") || text->text().endsWith(" km"));
    map->findChild<QToolButton *>("rulerClear")->click();
    QCOMPARE(map->rulerMetres(), 0.0);
    ruler->click();
    QVERIFY(!text->isVisible());
}

// FL-055: ticking two rows in the Grouping dialog and pressing Group makes them one item.
void MainFormTest::groupWithJoinsTickedRows() {
    const int before = list()->listModel()->rowCount();
    QVERIFY(before >= 2);
    clickRow(0);
    bool seen = false;
    QTimer::singleShot(300, this, [&] {
        for (QWidget *t : QApplication::topLevelWidgets()) {
            auto *g = qobject_cast<FormGroup *>(t);
            if (!g || !g->isVisible()) continue;
            seen = true;
            auto *tree = g->find<QTreeWidget>("ListView");
            tree->topLevelItem(0)->setCheckState(0, Qt::Checked);
            tree->topLevelItem(1)->setCheckState(0, Qt::Checked);
            shot(g, "group");
            QTest::mouseClick(g->find<QWidget>("ButtonGroup"), Qt::LeftButton);
        }
    });
    form->action("N_FileList_Group_With")->trigger();
    QVERIFY(seen);
    QTRY_COMPARE(list()->listModel()->rowCount(), before - 1);
    const auto grouped = std::find_if(list()->listModel()->fileRows().rows.begin(), list()->listModel()->fileRows().rows.end(),
                                      [](const FileRow &r) { return r.files == 2; });
    QVERIFY(grouped != list()->listModel()->fileRows().rows.end());
    // Ungroup it again for the remaining tests.
    list()->selectRow(int(grouped - list()->listModel()->fileRows().rows.begin()));
    form->action("N_FileList_Ungroup")->trigger();
    QTRY_COMPARE(list()->listModel()->rowCount(), before);
}

// SF-COPY-005…008: every file arrives with its size and modification time.
void MainFormTest::copyCopiesFilesWithTimes() {
    QStringList files;
    for (const QFileInfo &f : QDir(data).entryInfoList({"*.MOV", "*.MP4"}, QDir::Files)) files.append(f.absoluteFilePath());
    QVERIFY(!files.isEmpty());
    const QString to = QDir(testRoot()).filePath("copy");
    QDir(to).removeRecursively();
    auto *w = new FormCopy;
    QVERIFY(w->load(":/original/forms/TFORMCOPY.dfm"));
    QVERIFY(w->popup(files, "test", to));
    QTRY_VERIFY_WITH_TIMEOUT(w->isFinished(), 30000);
    shot(w, "copy");
    for (const QString &f : files) {
        const QFileInfo src(f), dst(QDir(to).filePath(src.fileName()));
        QVERIFY2(dst.exists(), qPrintable(dst.filePath()));
        QCOMPARE(dst.size(), src.size());
        QCOMPARE(dst.lastModified(QTimeZone::UTC), src.lastModified(QTimeZone::UTC));
    }
    w->close();
}

// SF-CROPPING-035, SF-CROPPINGITEM-*: the A-B selection becomes a fragment; its speed can be edited.
void MainFormTest::cuttingListsTheSelection() {
    clickRow(0);
    QTRY_VERIFY(!form->playback->group().isEmpty());
    form->selectL = 0.25;
    form->selectR = 0.75;
    form->action("N_File_Save_Video")->trigger();
    FormCropping *f = form->croppingForm;
    QVERIFY(f && f->isVisible());
    QCOMPARE(f->count(), 1);
    QVERIFY(std::abs(f->cropItems()[0].p1 - 0.25) < 1e-6 && std::abs(f->cropItems()[0].p2 - 0.75) < 1e-6);
    QVERIFY(f->find<QLineEdit>("EditFileName")->text().contains("Fragment of "));
    QVERIFY(f->find<QLabel>("LabelTimePos")->text().startsWith("(1) "));
    shot(f, "cutting");
    // The editor: speed 4 switches the method to key frames only.
    QTimer::singleShot(300, this, [] {
        for (QWidget *t : QApplication::topLevelWidgets()) {
            if (t->objectName() != "FormCroppingItem" || !t->isVisible()) continue;
            auto *ed = qobject_cast<vcl::Form *>(t);
            ed->find<QLineEdit>("EditSpeed")->setText("4");
            shot(t, "cutting-item");
            QTest::mouseClick(ed->find<QWidget>("ButtonOk"), Qt::LeftButton);
        }
    });
    f->N_EditClick(nullptr);
    QCOMPARE(f->cropItems()[0].speed, 4.0);
    QVERIFY(!f->cropItems()[0].speedUpFps);
    f->N_FileList_ClearClick(nullptr);
    f->close();
    form->selectL = form->selectR = 0;
}

// SF-CROPPING-023…030: the middle half of a recording is exported with its GPS, gets the
// recording's date and joins the main list.
void MainFormTest::cuttingExportsTheSelection() {
    clickRow(0);
    QTRY_VERIFY(!form->playback->group().isEmpty());
    const GroupFile src = form->playback->group().files.first();
    form->selectL = 0.25;
    form->selectR = 0.75;
    form->action("N_File_Save_Video")->trigger();
    FormCropping *f = form->croppingForm;
    QVERIFY(f && f->count() == 1);
    const QString out = QDir(testRoot()).filePath("cut/fragment." + QFileInfo(src.path).suffix());
    QDir(QFileInfo(out).absolutePath()).removeRecursively();
    f->find<QLineEdit>("EditFileName")->setText(QDir::toNativeSeparators(out));
    f->find<QCheckBox>("CheckBoxSRTSubtitles")->setChecked(true);
    QString finished;
    connect(f, &FormCropping::exportFinished, this, [&](const QString &o) { finished = o; });
    QStringList messages;
    QTimer answer;
    connect(&answer, &QTimer::timeout, this, [&] {
        if (auto *m = qobject_cast<QMessageBox *>(QApplication::activeModalWidget())) {
            messages.append(m->text());
            m->accept();
        }
    });
    answer.start(50);
    QTest::mouseClick(f->find<QWidget>("ButtonStart"), Qt::LeftButton);
    QTRY_VERIFY_WITH_TIMEOUT(!finished.isEmpty(), 60000);
    answer.stop();
    QVERIFY2(messages.isEmpty(), qPrintable(messages.join(" | ")));
    QCOMPARE(QFileInfo(finished), QFileInfo(out));
    QVERIFY(form->isVisible());
    char *raw = rv_file_info_json(QDir::toNativeSeparators(out).toUtf8().constData());
    const QJsonObject info = QJsonDocument::fromJson(raw).object();
    rv_string_free(raw);
    const double half = src.durationMs / 1000.0 / 2;
    const double duration = info.value("duration").toDouble();
    // The start snaps back to a key frame (at most one GOP, about a second, earlier).
    QVERIFY2(duration > half - 0.2 && duration < half + 1.5, qPrintable(QString::number(duration)));
    QVERIFY(!info.value("gps").toArray().isEmpty());
    QVERIFY(QFileInfo(out).lastModified().date().year() < 2020);
    // SF-CROPPING-030: subtitles beside it, a cue per GPS fix.
    QFile srt(out.left(out.lastIndexOf('.') + 1) + "srt");
    QVERIFY2(srt.open(QIODevice::ReadOnly), qPrintable(srt.fileName()));
    QVERIFY(srt.readAll().count("-->") >= 2);
    f->find<QCheckBox>("CheckBoxSRTSubtitles")->setChecked(false);
    QTRY_VERIFY_WITH_TIMEOUT(std::any_of(form->fileManager->rows().rows.begin(), form->fileManager->rows().rows.end(),
                                         [&](const FileRow &r) { return QFileInfo(r.path) == QFileInfo(out); }),
                             10000);
    shot(f, "cutting-done");
    f->N_FileList_ClearClick(nullptr);
    f->close();
    form->selectL = form->selectR = 0;
}

// FL-063, SF-REPAIR-003/009: the current row is the broken file, another file of the list the
// reference; the same file twice is refused.
void MainFormTest::repairPicksReference() {
    clickRow(0);
    const Group g = form->fileManager->item(0);
    form->action("N_FileList_Repair")->trigger();
    FormRepair *f = form->repairForm;
    QVERIFY(f && f->isVisible());
    const QString broken = f->find<QLineEdit>("EditBroken")->text(), ref = f->find<QLineEdit>("EditRef")->text();
    QCOMPARE(QFileInfo(broken), QFileInfo(g.files.first().path));
    QVERIFY(!ref.isEmpty() && QFileInfo(ref) != QFileInfo(broken));
    QVERIFY(!f->find<QProgressBar>("ProgressBar")->isVisible());
    QVERIFY(!f->find<QWidget>("EditResult")->isVisible());
    // The button sits under the method box, which holds the whole description.
    auto *box = f->find<QWidget>("GroupBoxMethod");
    auto *fast = f->find<QLabel>("LabelCaptionFast");
    QVERIFY(f->find<QWidget>("ButtonRepair")->geometry().top() > box->geometry().bottom());
    QVERIFY(fast->geometry().bottom() < box->height());
    QVERIFY(f->find<QWidget>("ButtonRepair")->geometry().bottom() < f->height());
    shot(f, "repair");
    QString message;
    QTimer::singleShot(300, this, [&] {
        if (auto *m = qobject_cast<QMessageBox *>(QApplication::activeModalWidget())) {
            message = m->text();
            m->accept();
        }
    });
    f->find<QLineEdit>("EditRef")->setText(broken);
    QTest::mouseClick(f->find<QWidget>("ButtonRepair"), Qt::LeftButton);
    QVERIFY2(message.contains("="), qPrintable(message));
    f->close();
    form->action("N_File_Repair_Video")->trigger();
    QVERIFY(f->find<QLineEdit>("EditBroken")->text().isEmpty() && f->find<QLineEdit>("EditRef")->text().isEmpty());
    f->close();
}

// SF-REPAIR-013…015: a recording whose header was lost (moov turned into free space) is
// rebuilt from another file of the same camera and plays again.
void MainFormTest::repairRebuildsLostHeader() {
    QString source, reference;
    for (const QFileInfo &f : QDir(data).entryInfoList({"*AA*.MP4"}, QDir::Files, QDir::Name)) {
        (f.fileName().contains("163641AA") ? source : reference) = f.absoluteFilePath();
    }
    if (source.isEmpty() || reference.isEmpty()) QSKIP("needs the two AA camera samples");
    const QString dir = QDir(testRoot()).filePath("repair");
    QDir(dir).removeRecursively();
    QDir().mkpath(dir);
    const QString broken = QDir(dir).filePath("broken.MP4");
    QVERIFY(QFile::copy(source, broken));
    QFile::setPermissions(broken, QFile::permissions(broken) | QFileDevice::WriteOwner);
    {
        QFile f(broken);
        QVERIFY(f.open(QIODevice::ReadWrite));
        QByteArray all = f.readAll();
        const qsizetype at = all.indexOf("moov");
        QVERIFY(at > 4);
        f.seek(at);
        f.write("free");
    }
    auto infoDuration = [](const QString &path) {
        char *raw = rv_file_info_json(QDir::toNativeSeparators(path).toUtf8().constData());
        const double d = QJsonDocument::fromJson(raw).object().value("duration").toDouble();
        rv_string_free(raw);
        return d;
    };
    QCOMPARE(infoDuration(broken), 0.0);

    form->action("N_File_Repair_Video")->trigger();
    FormRepair *f = form->repairForm;
    QVERIFY(f && f->isVisible());
    f->find<QLineEdit>("EditBroken")->setText(QDir::toNativeSeparators(broken));
    f->find<QLineEdit>("EditRef")->setText(QDir::toNativeSeparators(reference));
    QString message;
    QTimer answer;
    connect(&answer, &QTimer::timeout, this, [&] {
        if (auto *m = qobject_cast<QMessageBox *>(QApplication::activeModalWidget())) {
            message = m->text();
            m->accept();
        }
    });
    answer.start(50);
    QTest::mouseClick(f->find<QWidget>("ButtonRepair"), Qt::LeftButton);
    QTRY_VERIFY_WITH_TIMEOUT(!message.isEmpty(), 20000);
    answer.stop();
    QVERIFY2(message.startsWith(Lang::instance().tr("Repair_Ok")), qPrintable(message));
    QVERIFY(infoDuration(broken) > 4);
    QVERIFY(f->find<QWidget>("ButtonRepair")->isEnabled());
    f->close();
}

// GM-190: a file in the converter gets its points, duration and length.
void MainFormTest::gpsConverterShowsStatistics() {
    const QString source = form->fileManager->item(0).files.first().path;
    const QString dir = QDir(testRoot()).filePath("convert");
    QDir(dir).removeRecursively();
    QDir().mkpath(dir);
    const QString file = QDir(dir).filePath(QFileInfo(source).fileName());
    QVERIFY(QFile::copy(source, file));
    form->action("N_File_Convert_GPS")->trigger();
    FormGPSLog *f = form->gpsLogForm;
    QVERIFY(f && f->isVisible());
    auto *tree = f->find<QTreeWidget>("ListViewFiles");
    tree->clear();
    f->popup({file}, form, false, 0);
    QCOMPARE(tree->topLevelItemCount(), 1);
    QTRY_VERIFY_WITH_TIMEOUT(!tree->topLevelItem(0)->text(1).isEmpty(), 5000);
    QVERIFY2(tree->topLevelItem(0)->text(1).toInt() > 0, qPrintable(tree->topLevelItem(0)->text(1)));
    QVERIFY(tree->topLevelItem(0)->text(2).contains(':'));
    QVERIFY(tree->headerItem()->text(3).endsWith(Lang::instance().tr("Kmeters")));
    shot(f, "gpslog");
    // Nothing ticked: a message instead of a conversion.
    tree->topLevelItem(0)->setCheckState(0, Qt::Unchecked);
    QString message;
    QTimer::singleShot(300, this, [&] {
        if (auto *m = qobject_cast<QMessageBox *>(QApplication::activeModalWidget())) {
            message = m->text();
            m->accept();
        }
    });
    QTest::mouseClick(f->find<QWidget>("ButtonConvert"), Qt::LeftButton);
    QVERIFY(!message.isEmpty());
    // GM-192: the eight formats are listed, KML ticked; ticking GPX too writes both.
    auto *formats = f->find<QListWidget>("CheckListBoxConverters");
    QCOMPARE(formats->count(), 8);
    QCOMPARE(formats->item(0)->checkState(), Qt::Checked);
    formats->item(2)->setCheckState(Qt::Checked);
    tree->topLevelItem(0)->setCheckState(0, Qt::Checked);
    QTest::mouseClick(f->find<QWidget>("ButtonConvert"), Qt::LeftButton);
    const QString base = file.left(file.lastIndexOf('.') + 1);
    for (const char *ext : {"kml", "gpx"}) {
        QFile out(base + ext);
        QVERIFY2(out.open(QIODevice::ReadOnly), qPrintable(out.fileName()));
        QVERIFY(out.readAll().contains(ext[0] == 'k' ? "<kml" : "<gpx"));
    }
    QCOMPARE(tree->topLevelItem(0)->checkState(0), Qt::Unchecked);
    formats->item(2)->setCheckState(Qt::Unchecked);
    f->close();
}

// FL-070/071: the selected row's track as GPX, named after its start.
void MainFormTest::exportTrackOfTheSelection() {
    list()->clearSelection();
    clickRow(0);
    QTRY_VERIFY(!form->playback->group().isEmpty());
    const QVector<gpsfiles::Source> sources = form->selectionGps();
    QVERIFY(!sources.isEmpty());
    const gpsfiles::Result info = gpsfiles::write(sources, QString(), QString(), {});
    QVERIFY(info.hasPoints);
    QVERIFY2(info.fileName.startsWith("Track ") && !info.fileName.contains(':'), qPrintable(info.fileName));
    const QString out = QDir(testRoot()).filePath("track.gpx");
    QFile::remove(out);
    const gpsfiles::Result r = gpsfiles::write(sources, "gpx", out, {});
    QVERIFY2(r.ok, qPrintable(r.error));
    QFile f(out);
    QVERIFY(f.open(QIODevice::ReadOnly));
    QVERIFY(f.readAll().count("<trkpt") >= 2);
}

// SF-CROPPING-021, PV-092: the template window shows an example; a template of one's own is kept.
void MainFormTest::subtitleTemplateIsEditable() {
    form->action("N_File_Save_Video")->trigger();
    FormCropping *c = form->croppingForm;
    QVERIFY(c);
    QTest::mouseClick(c->find<QWidget>("ButtonSubtitle"), Qt::LeftButton);
    auto *t = c->findChild<FormSetupSubtitleTemplate *>();
    QVERIFY(t && t->isVisible());
    auto *def = t->find<QCheckBox>("CheckBoxDefault");
    auto *memo = t->find<QPlainTextEdit>("MemoTemplate");
    auto *preview = t->find<QLabel>("LabelPreview");
    QVERIFY(def->isChecked());
    QVERIFY(memo->toPlainText().contains("%lat%"));
    QVERIFY(!preview->text().contains('%') && !preview->text().isEmpty());
    shot(t, "subtitle-template");
    def->click();
    QVERIFY(memo->isEnabled());
    memo->setPlainText("Speed %speed%");
    QTest::keyClick(memo, Qt::Key_End);
    QVERIFY2(preview->text().startsWith("Speed ") && !preview->text().contains('%'), qPrintable(preview->text()));
    QTest::mouseClick(t->find<QWidget>("ButtonOk"), Qt::LeftButton);
    QCOMPARE(Settings::instance().readString("subtitleTemplate"), QString("Speed %speed%"));
    Settings::instance().remove("subtitleTemplate");
    c->close();
}

// FL-074: "0:00:10" moves the row's videos 10 s later; "-0:00:10" moves them back.
void MainFormTest::moveVideoShiftsTheRow() {
    clickRow(0);
    const TDateTime before = form->fileManager->item(0).finish;
    for (const char *text : {"0:00:10", "-0:00:10"}) {
        QTimer::singleShot(300, this, [text] {
            if (auto *d = qobject_cast<QInputDialog *>(QApplication::activeModalWidget())) {
                d->setTextValue(text);
                d->accept();
            }
        });
        form->action("N_FileList_Track_Move")->trigger();
        const double expected = before + (text[0] == '-' ? 0 : 10 * kSecond);
        QVERIFY2(std::abs(form->fileManager->item(0).finish - expected) < 1e-3 * kSecond, text);
    }
}

// FL-073: another file's track becomes the GPS of every file of the row.
void MainFormTest::attachTrackReplacesGps() {
    QVERIFY(form->fileManager->rows().rows.size() >= 2);
    const QString source = form->fileManager->item(0).files.first().path;
    QVERIFY(form->fileManager->setTrack(1, source));
    for (const GroupFile &f : form->fileManager->item(1).files) QCOMPARE(QFileInfo(f.infoPath), QFileInfo(source));
}

void MainFormTest::cleanupTestCase() {
    delete form;
}

int main(int argc, char *argv[]) {
    const QString data = prepareData();
    QDir(QDir(testRoot()).filePath("settings")).removeRecursively();
    QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, QDir(testRoot()).filePath("settings"));
    app::beforeApplication();
    // The data folder is the application's command line (FL-001), as when it is passed to rvplayer.exe.
    QByteArray folder = QDir::toNativeSeparators(data).toLocal8Bit();
    char *appArgv[] = {argv[0], folder.data(), nullptr};
    int appArgc = data.isEmpty() ? 1 : 2;
    QApplication a(appArgc, appArgv);
    app::afterApplication(a);
    MainFormTest test(data);
    return QTest::qExec(&test, argc, argv);
}

#include "tst_mainform.moc"
