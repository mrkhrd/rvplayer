// Playback, position and timeline interaction of TFormMain.
// Spec: playback-video.md §3.1–§3.10, graphs-map-layout.md GM-063/GM-072/GM-080.

#include "MainForm.h"

#include "core/Format.h"
#include "core/Playback.h"
#include "forms/FormDetails.h"
#include "core/Trace.h"
#include "filelist/FileListView.h"
#include "i18n/Lang.h"
#include "map/MapView.h"
#include "vcl/AlignLayout.h"
#include "video/VideoView.h"

#include <QAbstractButton>
#include <QAction>
#include <QCheckBox>
#include <QCursor>
#include <QFileInfo>
#include <QLocale>
#include <QScrollBar>
#include <QSignalBlocker>
#include <QTimer>
#include <QToolButton>

#include <algorithm>
#include <cmath>
#include <cstdio>

using vcl::Align;

namespace {

constexpr double kJumpSeconds = 8.312;  // PV-056

void setButton(MainForm *f, const char *name, bool enabled) {
    if (auto *w = f->find<QWidget>(name)) w->setEnabled(enabled);
}

}  // namespace

void MainForm::setupPlayback() {
    playback = new Playback(this);
    if (QWidget *panel = widget("PanelVideo")) {
        video = new VideoView(panel);
        video->setObjectName("VideoView");
        layoutOf(panel)->addControl(video, Align::Client, vcl::Anchors{}, QRect(0, 0, panel->width(), panel->height()));
        // The picture forwards the panel's mouse handlers, as the VMR window copied them.
        for (const char *ev : {"OnMouseDown", "OnMouseMove", "OnMouseUp"}) {
            bindHandler(video, ev, QString(ev).replace("On", "VideoWindowWMR"));
        }
        bindHandler(video, "OnDblClick", "N_FullScreenClick");
        attachPopup(video, "PopupMenuVideoWindow");
    }
    connect(playback, &Playback::fileOpened, this, [this] { layoutVideo(); });
    connect(playback, &Playback::frameReady, this, [this](const QImage &img) {
        if (video) video->setFrame(img);
        if (playback->state() != Playback::State::Running) TimerTimer(nullptr);
    });
    // TimerTimer is the DFM timer's handler; it runs only while playing (PV-013).
    if (QTimer *t = timer("Timer")) t->stop();
    for (const char *name : {"ButtonPlay", "ButtonStop", "ButtonFramePrev", "ButtonFrameNext", "ButtonVideoZoomIn",
                             "ButtonVideoZoomOut"}) {
        setButton(this, name, false);
    }
}

// PV-010: the selected item becomes the current one; nothing is opened yet.
void MainForm::loadGroup(const Group &g) {
    RV_TRACE_SCOPE("MainForm::loadGroup");
    stopPlayback(false);
    playback->setGroup(g);
    moving.load(g, trackOptions());
    setButton(this, "ButtonPlay", !g.isEmpty());
    playSpeedEnabled = !g.isEmpty();
    zoomL = 0;
    zoomR = 1;
    setupZoomBar();
    setRoute();
    FormResize(nullptr);
    TimerTimer(nullptr);
}

// PV-005 with the pin drag (step 1).
double MainForm::currentPosition() {
    if (pinGet) return lastPosition = pinGetValue;
    return lastPosition = playback->position();
}

// PV-006.
void MainForm::seekTo(double p, bool callTimer, bool force) {
    RV_TRACE_SCOPE("MainForm::seekTo");
    pinGet = false;
    mapRecentre = true;
    const double reached = playback->seek(p, force);
    if (reached >= 0) lastPosition = reached;
    if (callTimer) TimerTimer(nullptr);
}

// PV-013.
void MainForm::ButtonPlayClick(vcl::Event *) {
    RV_TRACE_SCOPE("MainForm::ButtonPlayClick");
    auto *play = find<QToolButton>("ButtonPlay");
    if (!play || playback->group().isEmpty()) return;
    if (!playback->active()) {
        updateCaption();
        seekTo(lastPosition, false, true);  // opens paused at the last position
    }
    QTimer *t = timer("Timer");
    if (play->isChecked()) {
        playback->play();
        if (t) t->start();
    } else {
        playback->pause();
        if (t) t->stop();
    }
    for (const char *name : {"ButtonStop", "ButtonVideoZoomIn", "ButtonVideoZoomOut", "ButtonFramePrev", "ButtonFrameNext"}) {
        setButton(this, name, true);
    }
    playSpeedEnabled = true;
    FormResize(nullptr);
}

// PV-015 (Space).
void MainForm::N_Playback_PlayClick(vcl::Event *) {
    auto *play = find<QToolButton>("ButtonPlay");
    if (!play || !play->isEnabled()) return;
    play->setChecked(!play->isChecked());
    ButtonPlayClick(nullptr);
}

// PV-016 (Esc).
void MainForm::N_Playback_StopClick(vcl::Event *) {
    if (isChecked("N_FullScreen")) {
        N_FullScreenClick(nullptr);  // PV-016: Esc leaves full screen first
        return;
    }
    stopPlayback(true);
}

void MainForm::ButtonStopClick(vcl::Event *) {
    stopPlayback(true);
}

// PV-017.
void MainForm::stopPlayback(bool clearSelection) {
    RV_TRACE_SCOPE("MainForm::stopPlayback");
    if (QTimer *t = timer("Timer")) t->stop();
    if (auto *play = find<QToolButton>("ButtonPlay")) {
        play->setChecked(false);
        play->setEnabled(false);
    }
    for (const char *name : {"ButtonStop", "ButtonVideoZoomIn", "ButtonVideoZoomOut", "ButtonFramePrev", "ButtonFrameNext"}) {
        setButton(this, name, false);
    }
    playSpeedEnabled = false;
    if (clearSelection) clearListSelection();
    if (isChecked("N_FullScreen")) N_FullScreenClick(nullptr);
    playback->setGroup(Group());
    moving.clear();
    if (video) video->clear();
    setWindowTitle(QStringLiteral("RV Player"));
    // ClearAll.
    if (auto *loop = find<QCheckBox>("CheckBoxLoop")) loop->setChecked(false);
    setChecked("N_View_Time_LoopAB", false);
    lastPosition = 0;
    pinGet = false;
    zoomL = 0;
    zoomR = 1;
    selectL = selectR = 0;
    position = -1;
    videoZoom = 1;
    videoDx = videoDy = 0.5;
    if (isChecked("N_VideoZoom_Custom")) menuItemSet(action("N_VideoZoom_Letterbox"), true);
    layoutVideo();
    if (MapView *m = mapView()) m->clearAll();
    setupZoomBar();
    clearGauges();
    hideDetails();
    FormResize(nullptr);
}

// PV-014 without the DATAKAM brand.
void MainForm::updateCaption() {
    const Group &g = playback->group();
    if (g.isEmpty()) return;
    const bool british = isChecked("N_View_Measu_British");
    QString c = g.files[0].face + "    " + fmt::dateTime(g.start(), tzShift);
    const double track = moving.trackLength();
    if (const QString t = fmt::trip(track, british); !t.isEmpty()) c += "    " + t;
    const double secs = g.duration * 86400.0;
    if (track > 0 && secs > 0) {
        c += "    " + fmt::speed(track / secs * 3.6, british) +
             Lang::instance().tr(british ? "MilesPerH" : "KmetersPerH");
    }
    setWindowTitle(c + "  -  RV Player");
}

// PV-021…PV-026.
void MainForm::TimerTimer(vcl::Event *e) {
    RV_TRACE_SCOPE("MainForm::TimerTimer");
    if (!isVisible()) return;
    const Group &g = playback->group();
    double p = currentPosition();
    if (g.isEmpty() || p < 0) {
        drawCursors();
        return;
    }
    const bool running = e && playback->active() && playback->state() == Playback::State::Running;
    if (running) {
        auto *loop = find<QCheckBox>("CheckBoxLoop");
        auto *play = find<QToolButton>("ButtonPlay");
        if (loop && loop->isChecked() && selectR > selectL && p >= selectR) {
            seekTo(selectL, false);  // PV-022
        } else if (isChecked("N_Playback_PlayNext") && p >= 1.0 && play && play->isChecked() && listView()) {
            // PV-019: the next row plays, after the last one the first.
            FileListView *list = listView();
            const int rows = list->listModel()->rowCount();
            const int i = list->selectedRows().value(0, -1);
            stopPlayback(true);
            if (rows > 0) {
                list->selectRow(i + 1 < rows ? i + 1 : 0);
                ListViewDblClick(nullptr);
            }
            return;
        } else {
            const double zoom = zoomR - zoomL;  // PV-023
            bool moved = false;
            if (p > zoomR - 0.05 * zoom && zoomR < 1) {
                zoomL = p - 0.1 * zoom;
                zoomR = zoomL + zoom;
                moved = true;
            } else if (p < zoomL) {
                zoomL = p - 0.5 * zoom;
                zoomR = zoomL + zoom;
                moved = true;
            }
            if (moved) {
                setupZoomBar();
                FormResize(nullptr);
            }
        }
    }

    // PV-025: gauges and the map marker at the same position.
    if (profile() != Profile::Video) {
        const MovingValue v = moving.at(g.timeAt(p));
        updateGauges(v);
        updateMapMarker(v, e != nullptr);
    }
    position = p;
    drawCursors();
    // The details mini-graph follows the position (TimerTimer L1359).
    if (details && detailsWidth > 0 && detailsColumns > 1) {
        const double from = detailsCenter - detailsWidth / 2;
        const double f = (p - from) / detailsWidth;
        details->drawCursor(f >= 0 && f <= 1 ? int(std::lround(f * (detailsColumns - 1))) : -1);
    }
}

// GM-080…GM-083 with Q-14 (no reading without data) and Q-15 (needle kept on the dial).
void MainForm::updateGauges(const MovingValue &v) {
    QWidget *pc = widget("PanelControl");
    if (!pc || pc->isHidden()) return;
    const qreal dpr = devicePixelRatioF();
    const QColor bg = Lang::instance().color("colorMainBG");
    const bool british = isChecked("N_View_Measu_British");
    if (auto *speed = find<vcl::Image>("ImageSpeed")) {
        if (!v.gps) {
            speed->setImage(speedBitmap.draw(dpr, -1, QString(), 0));
        } else {
            const double kmh = british ? v.speed / 1.609344 : v.speed;
            QString track;
            if (british) {
                track = QString::number(v.track / 1000 / 1.609344, 'f', 3);
            } else {
                track = QString::number(qint64(std::lround(v.track)));
                if (track.size() >= 4 && track.size() <= 6) track.insert(track.size() - 3, ' ');
            }
            const double alt = british ? v.alt / 0.3048 : v.alt;
            speed->setImage(speedBitmap.draw(dpr, kmh, track, alt));
        }
    }
    if (auto *az = find<vcl::Image>("ImageAzimut")) az->setImage(azimutBitmap.draw(dpr, bg, v.gps ? v.course : -1));
    if (auto *gs = find<vcl::Image>("ImageGSensor"); gs && !gs->isHidden()) {
        gs->setImage(gsensorBitmap.draw(dpr, bg, v.gsensor, v.x, v.y, v.z));
    }
}

// --- Seek bar and graphs ----------------------------------------------------------------------

// PV-034.
void MainForm::ImageSeekMouseDown(vcl::Event *e) {
    if (!e || e->button == Qt::RightButton || pinGet || playback->group().isEmpty()) return;
    auto *w = qobject_cast<QWidget *>(e->sender);
    if (!w) return;
    const int x = w->mapTo(this, e->pos).x() - widget("PanelBottomGraphics")->mapTo(this, QPoint(0, 0)).x();
    const double p = seek.progressAt(x);
    if (w == widget("ImageSeek") && std::abs(x - seek.xOf(lastPosition)) < seek.pinH() / 2) {
        pinGet = true;
        pinGetValue = p;
        return;
    }
    seekTo(p, true);
}

// PV-035.
void MainForm::ImageSeekMouseMove(vcl::Event *e) {
    auto *w = e ? qobject_cast<QWidget *>(e->sender) : nullptr;
    if (!w || !widget("PanelBottomGraphics")) return;
    const int x = w->mapTo(this, e->pos).x() - widget("PanelBottomGraphics")->mapTo(this, QPoint(0, 0)).x();
    if (pinGet) {
        pinGetValue = seek.progressAt(x);
        TimerTimer(nullptr);
        showDetails(w, x);
        return;
    }
    showDetails(w, x);
    const bool onPin = w == widget("ImageSeek") && !playback->group().isEmpty() &&
                       std::abs(x - seek.xOf(lastPosition)) < seek.pinH() / 2;
    w->setCursor(onPin ? Qt::PointingHandCursor : Qt::ArrowCursor);
}

// PV-036.
void MainForm::ImageSeekMouseUp(vcl::Event *e) {
    if (!pinGet || !e) return;
    auto *w = qobject_cast<QWidget *>(e->sender);
    const int x = w->mapTo(this, e->pos).x() - widget("PanelBottomGraphics")->mapTo(this, QPoint(0, 0)).x();
    seekTo(seek.progressAt(x), true);
}

// GM-063 / PV-032: gaps, files without a duration, locked files and the A–B selection.
QVector<draw::Band> MainForm::bands() const {
    const Group &g = playback ? playback->group() : Group();
    QVector<draw::Band> out;
    QWidget *panel = widget("PanelBottomGraphics");
    if (g.isEmpty() || !panel || zoomR <= zoomL) return out;
    const int w = std::max(1, panel->width() - 2 * seek.border());
    const double span = zoomR - zoomL;
    auto add = [&](double p1, double p2, QColor fill, QColor line) {
        if (p2 < zoomL || p1 > zoomR) return;
        int x1 = int(std::floor(w * (p1 - zoomL) / span));
        int x2 = int(std::ceil(w * (p2 - zoomL) / span));
        x1 = std::clamp(x1, 0, w);
        x2 = std::clamp(x2, 0, w);
        if (x2 <= x1) x2 = x1 + 1;
        out.append({x1, x2, fill, line});
    };
    for (const GroupFile &f : g.files) {
        if (f.locked) add(f.pStart, f.pFinish, QColor(255, 255, 180), QColor(255, 255, 180));
    }
    for (const GroupFile &f : g.files) {
        if (f.durationState == 2) add(f.pStart, f.pFinish, QColor(240, 240, 0), QColor(240, 240, 0));
    }
    const QColor highlight = Lang::vclColor("clHighlight");
    if (selectL < selectR) add(selectL, selectR, highlight, highlight);
    // Areas without video, and a separator where two files touch.
    QVector<std::pair<double, double>> gaps;
    if (g.files.first().pStart > 0) gaps.append({0, g.files.first().pStart});
    for (int i = 0; i + 1 < g.files.size(); ++i) gaps.append({g.files[i].pFinish, g.files[i + 1].pStart});
    if (g.files.last().pFinish < 1) gaps.append({g.files.last().pFinish, 1});
    const QColor sep = gaps.size() < 5 ? QColor(100, 100, 100) : QColor(200, 200, 200);
    for (const auto &[a, b] : gaps) add(a, std::max(a, b), QColor(200, 200, 200), sep);
    return out;
}

// --- Time zoom --------------------------------------------------------------------------------

// PV-042 (with Q-16, the scrollbar range fixed).
void MainForm::setupZoomBar() {
    if (zoomL > zoomR) std::swap(zoomL, zoomR);
    const Group &g = playback ? playback->group() : Group();
    const double minZoom = 0.5 / (1 + g.duration * 86400.0);
    if (zoomR - zoomL < minZoom) {
        const double c = (zoomL + zoomR) / 2;
        zoomL = c - minZoom / 2;
        zoomR = c + minZoom / 2;
    }
    const double width = zoomR - zoomL;
    if (zoomR > 1) {
        zoomL = 1 - width;
        zoomR = 1;
    }
    if (zoomL < 0) {
        zoomL = 0;
        zoomR = std::min(1.0, width);
    }
    auto *bar = find<QScrollBar>("ScrollBarZoom");
    QWidget *panel = widget("PanelBottomGraphics");
    if (!bar || !panel) return;
    QSignalBlocker block(bar);
    const int page = std::max(1, (panel->width() - 2 * seek.border()) / 10);
    const int max = std::max(0, int(std::lround(page / std::max(width, 1e-9))) - page);
    bar->setRange(0, max);
    bar->setPageStep(page);
    bar->setValue(max > 0 ? int(std::lround(max * zoomL / std::max(1e-9, 1 - width))) : 0);
    bar->setEnabled(width < 0.999 && max > 0);
    drawMarkersAB();
}

// PV-043: zoom in or out around the current position.
void MainForm::ButtonZoomInClick(vcl::Event *e) {
    if (e) {
        const QString name = e->sender ? e->sender->objectName() : QString();
        const bool in = name == "ButtonZoomIn" || name == "N_View_Time_ZoomIn" || name.startsWith("N_View_Time_ZoomIn");
        double zoom = std::abs(zoomR - zoomL);
        zoom = in ? zoom / zoomK : zoom * zoomK;
        zoomL = lastPosition - zoom / 2;
        zoomR = lastPosition + zoom / 2;
    }
    setupZoomBar();
    FormResize(nullptr);
}

// PV-044.
void MainForm::ScrollBarZoomChange(vcl::Event *) {
    auto *bar = find<QScrollBar>("ScrollBarZoom");
    if (!bar) return;
    const double width = zoomR - zoomL;
    if (!bar->isEnabled() || bar->maximum() <= 0) {
        zoomL = 0;
        zoomR = 1;
    } else {
        zoomL = double(bar->value()) / bar->maximum() * (1 - width);
        zoomR = zoomL + width;
    }
    drawMarkersAB();
    FormResize(nullptr);
}

// --- Jumps and frames -------------------------------------------------------------------------

// PV-056.
void MainForm::N_Playback_LeftClick(vcl::Event *e) {
    const Group &g = playback->group();
    double p = currentPosition();
    if (g.isEmpty() || p < 0 || g.duration <= 0) return;
    const bool right = e && e->sender && e->sender->objectName() == "N_Playback_Right";
    p += (right ? kJumpSeconds : -kJumpSeconds) * kSecond / g.duration;
    seekTo(std::clamp(p, 0.0, 1.0), true, playback->active());
}

void MainForm::pauseForStep() {
    if (auto *play = find<QToolButton>("ButtonPlay"); play && play->isChecked()) {
        play->setChecked(false);
        ButtonPlayClick(nullptr);
    }
}

// PV-057.
void MainForm::ButtonFrameNextClick(vcl::Event *) {
    if (!playback->active()) return;
    pauseForStep();
    playback->stepForward();
}

// PV-058 with Q-8: one frame back.
void MainForm::ButtonFramePrevClick(vcl::Event *) {
    if (!playback->active()) return;
    pauseForStep();
    playback->stepBack();
}

// PV-045 / PV-046.
void MainForm::N_View_Time_SetAClick(vcl::Event *e) {
    const double p = currentPosition();
    if (e && e->sender && e->sender->objectName() == "N_View_Time_SetB") {
        selectR = p;
        if (selectR <= selectL) selectL = 0;
    } else {
        selectL = p;
        if (selectR <= selectL) selectR = 1;
    }
    FormResize(nullptr);
}

// PV-047: the menu item and the check box mirror each other.
void MainForm::N_View_Time_LoopABClick(vcl::Event *e) {
    auto *box = find<QCheckBox>("CheckBoxLoop");
    bool on = box && box->isChecked();
    if (e && qobject_cast<QAction *>(e->sender)) on = !isChecked("N_View_Time_LoopAB");
    setChecked("N_View_Time_LoopAB", on);
    if (box) {
        QSignalBlocker block(box);
        box->setChecked(on);
    }
}

// GM-090.
void MainForm::N_View_Refresh_SlowClick(vcl::Event *e) {
    if (auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr) menuItemClick(a);
    QTimer *t = timer("Timer");
    if (!t) return;
    if (isChecked("N_View_Refresh_Slow")) t->setInterval(1000);
    else if (isChecked("N_View_Refresh_Norm")) t->setInterval(300);
    else t->setInterval(100);
}

// GM-120…GM-122.
void MainForm::showDetails(QWidget *image, int x) {
    const Group &g = playback->group();
    QWidget *panel = widget("PanelBottomGraphics");
    if (!panel || !isChecked("N_View_Details") || isChecked("N_FullScreen") || g.isEmpty()) {
        hideDetails();
        return;
    }
    const int W = panel->width();
    if (x < seek.border() - 5 || x > W - seek.border() + 5) {
        hideDetails();
        return;
    }
    if (!details) {
        details = new FormDetails(this);
        if (!details->load(":/original/forms/TFORMDETAILS.dfm")) {
            delete details;
            details = nullptr;
            return;
        }
    }
    const double p = seek.progressAt(x);
    const int k = g.fileAt(p);
    FormDetails::Content c;
    c.file = QFileInfo(g.files[k].path).fileName();
    if (g.files.size() > 1) c.file = QStringLiteral("[%1] ").arg(k + 1) + c.file;
    const QDateTime when = fmt::toQDateTime(g.timeAt(p) + tzShift);
    c.systemTime = QLocale::system().toString(when.date(), QLocale::ShortFormat) + " " + when.time().toString("HH:mm:ss");
    const qint64 ms = qint64(std::lround(g.elapsedSeconds(p) * 1000));
    c.videoTime = QStringLiteral("%1:%2:%3.%4")
                      .arg(ms / 3600000)
                      .arg(ms / 60000 % 60, 2, 10, QChar('0'))
                      .arg(ms / 1000 % 60, 2, 10, QChar('0'))
                      .arg(ms % 1000, 3, 10, QChar('0'));
    c.value = moving.at(g.timeAt(p));
    c.british = isChecked("N_View_Measu_British");
    c.profile = profile() == Profile::Video ? FormDetails::Profile::Video
                : profile() == Profile::Gps ? FormDetails::Profile::Gps
                                            : FormDetails::Profile::GpsG;
    // The mini-graph covers about 20 timeline pixels around the mouse, for a graph image only.
    detailsWidth = (zoomR - zoomL) * 20 / std::max(32, W - 2 * seek.border());
    detailsCenter = p;
    detailsColumns = 0;
    const double minZoom = 0.5 / (1 + g.duration * 86400.0);
    for (const GraphImage &gi : graphs) {
        if (gi.image != image || detailsWidth < minZoom) continue;
        detailsColumns = std::max(2, details->width() - 2 - 40);
        c.graph = gi.graphic.kind();
        if (QAction *a = action(gi.graphic.menuItem())) c.graphName = a->text();
        c.bins = moving.bins(gi.graphic.kind(), g.timeAt(p - detailsWidth / 2), g.timeAt(p + detailsWidth / 2), detailsColumns,
                             c.british);
        const double f = (lastPosition - (p - detailsWidth / 2)) / detailsWidth;
        c.cursorColumn = f >= 0 && f <= 1 ? int(std::lround(f * (detailsColumns - 1))) : -1;
    }
    const QPoint top = panel->mapToGlobal(QPoint(0, 0));
    details->popup(c, QCursor::pos(), top.y(), top.y() + panel->height());
}

void MainForm::hideDetails() {
    if (details) details->popdown();
    detailsWidth = 0;
}

void MainForm::OnImageMouseLeave(vcl::Event *) {
    hideDetails();
}

void MainForm::PopupMenuGraphicsPopup(vcl::Event *) {
    hideDetails();
}
