// View menus, video zoom, full screen, mouse wheel and extra keys of TFormMain.
// Spec: shell-settings-forms.md (AddLanguages, AddTimeZones, TR-010), playback-video.md §2.6,
// §3.11–§3.14, graphs-map-layout.md GM-091…GM-093.

#include "MainForm.h"

#include "core/Playback.h"
#include "core/Settings.h"
#include "map/MapView.h"
#include "filelist/FileListView.h"
#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"
#include "video/VideoView.h"

#include <QAction>
#include <QApplication>
#include <QCursor>
#include <QDateTime>
#include <QInputDialog>
#include <QKeyEvent>
#include <QMenu>
#include <QMenuBar>
#include <QScreen>
#include <QScrollBar>
#include <QSlider>
#include <QTimeZone>
#include <QTimer>
#include <QToolButton>
#include <QWheelEvent>

#include <cmath>

using vcl::Align;

namespace {

// Hours of the local time zone now (for "Local System (+3)" and the GMT shifts).
double localOffsetHours() {
    return QDateTime::currentDateTime().offsetFromUtc() / 3600.0;
}

bool isInside(const QWidget *w, const QWidget *container) {
    return w && container && (w == container || container->isAncestorOf(w));
}

}  // namespace

// --- Languages, time zones and units ----------------------------------------------------------

// AddLanguages (L533–L546): one radio item per language; the click is DoTranslate.
void MainForm::addLanguages() {
    QAction *item = action("N_View_Language");
    if (!item) return;
    // The form file gives the item no children, so it needs a submenu to hold the languages.
    QMenu *m = item->menu();
    if (!m) {
        m = new QMenu(this);
        item->setMenu(m);
    }
    Lang &lang = Lang::instance();
    for (const QString &id : lang.ids()) {
        const QString name = "N_View_Language_" + QString(id).replace('-', '_');
        addMenuItem(m, name, "(" + id + ") " + lang.nativeName(id), true, id == lang.current(), "DoTranslate");
    }
}

// AddTimeZones (L551–L560): GMT-12 … GMT+12 after "Local System".
void MainForm::addTimeZones() {
    QMenu *m = menu("N_View_TimeZone");
    if (!m) return;
    for (int off = -12; off <= 12; ++off) {
        const QString caption = QStringLiteral("GMT%1%2").arg(off < 0 ? "-" : "+").arg(std::abs(off));
        addMenuItem(m, QStringLiteral("N_View_TimeZone_GMT%1").arg(off + 24), caption, true, false,
                    "N_View_TimeZone_LocalClick");
    }
}

// TR-010: the language items' click.
void MainForm::DoTranslate(vcl::Event *e) {
    if (auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr) menuItemClick(a);
    if (QMenu *m = menu("N_View_Language")) {
        for (QAction *a : m->actions()) {
            if (isChecked(a)) Lang::instance().setCurrent(a->objectName().mid(16).replace('_', '-'));
        }
    }
    doTranslate();
    FormResize(nullptr);
}

// GM-093.
void MainForm::N_View_TimeZone_LocalClick(vcl::Event *e) {
    auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (a) menuItemClick(a);
    tzShift = 0;
    if (QMenu *m = menu("N_View_TimeZone")) {
        for (QAction *item : m->actions()) {
            const QString name = item->objectName();
            if (name.startsWith("N_View_TimeZone_GMT") && isChecked(item)) {
                tzShift = (name.mid(19).toInt() - 24) / 24.0 - localOffsetHours() / 24.0;
            }
        }
    }
    if (FileListView *list = listView()) {
        list->listModel()->setTimeZoneShift(tzShift);
        list->viewport()->update();
    }
    if (a) FormResize(nullptr);
}

// GM-091.
void MainForm::N_View_Measu_MetricClick(vcl::Event *e) {
    if (auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr) menuItemClick(a);
    doTranslate();
    FormResize(nullptr);
}

// GM-166 / GM-162: persisted; the next tick redraws.
void MainForm::N_View_Arrow_SmallClick(vcl::Event *e) {
    if (auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr) menuItemClick(a);
    TimerTimer(nullptr);
}

void MainForm::N_View_TimePoints_1mClick(vcl::Event *e) {
    if (auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr) menuItemClick(a);
    redrawRoute();
}

void MainForm::N_Playback_PlayNextClick(vcl::Event *e) {
    if (auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr) menuItemClick(a);
}

void MainForm::N_Playback_PauseOnClickClick(vcl::Event *e) {
    if (auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr) menuItemClick(a);
}

void MainForm::N_File_ExitClick(vcl::Event *) {
    close();
}

// PV-055: the value is kept (deleted at 100); sound output is not implemented yet.
void MainForm::TrackBarVolumeChange(vcl::Event *) {
    auto *s = find<QSlider>("TrackBarVolume");
    if (!s || !isVisible()) return;
    if (s->value() == 100) Settings::instance().remove("volume");
    else Settings::instance().writeInt("volume", s->value());
}

// --- Context menus (CopyMainMenu L631–L653) ---------------------------------------------------

void MainForm::buildPopupCopies() {
    // Q-3: the DirectShow splitter, decoder and renderer menus are gone.
    for (const char *name : {"N_Playback_Splitters", "N_Playback_Decoders", "N_Playback_Renderer", "N_S6"}) {
        if (QAction *a = action(name)) a->setVisible(false);
    }
    // Shared actions keep captions, checks and shortcuts in step with the main menu.
    if (QMenu *video = menu("PopupMenuVideoWindow"); video && menu("N_Playback")) {
        for (QAction *a : menu("N_Playback")->actions()) {
            if (a->isVisible()) video->addAction(a);
        }
        video->addSeparator();
        for (const char *name : {"N_File", "N_View", "N_VideoZoom"}) {
            if (QMenu *m = menu(name)) video->addMenu(m);
        }
        video->addSeparator();
        if (QAction *exit = action("N_File_Exit")) video->addAction(exit);
    }
    if (QMenu *g = menu("PopupMenuGraphics"); g && menu("N_View_Time") && menu("N_View_Graphics")) {
        for (QAction *a : menu("N_View_Time")->actions()) {
            if (a->objectName() != "N_View_Time_LoopAB") g->addAction(a);
        }
        if (QAction *save = action("N_File_Save_Video")) g->addAction(save);
        g->addSeparator();
        for (QAction *a : menu("N_View_Graphics")->actions()) g->addAction(a);
        g->addSeparator();
        if (QAction *details = action("N_View_Details")) g->addAction(details);
    }
}

// --- Video zoom -------------------------------------------------------------------------------

QSize MainForm::nativeVideoSize() const {
    if (video && !video->frameSize().isEmpty()) return video->frameSize();
    const Group &g = playback->group();
    const int i = std::max(0, playback->fileIndex());
    if (!g.isEmpty() && i < g.files.size()) return QSize(g.files[i].width, g.files[i].height);
    return {};
}

// PV-061: zoom actions (with a sender) and the re-layout of the picture (always).
void MainForm::ButtonVideoZoomOutClick(vcl::Event *e) {
    const QSize native = nativeVideoSize();
    const double cx = native.width(), cy = native.height();
    const double W = video ? video->width() : 0, H = video ? video->height() : 0;
    if (e && e->sender && cx > 0 && cy > 0 && W > 0 && H > 0) {
        const QString name = e->sender->objectName();
        const bool custom = isChecked("N_VideoZoom_Custom");
        double prevZoom = 0;
        if (!custom) {
            // The largest power of zoomK at which the picture fits the panel.
            double z = 1;
            while (z * cx > W + 1 || z * cy > H + 1) z /= zoomK;
            while (z * zoomK * cx <= W + 1 && z * zoomK * cy <= H + 1) z *= zoomK;
            videoZoom = z;
        } else {
            prevZoom = videoZoom;
        }
        if (name == "ButtonVideoZoomOut" || name == "N_VideoZoom_ZoomOut") {
            if (custom) videoZoom /= zoomK;
        } else if (name == "ButtonVideoZoomIn" || name == "N_VideoZoom_ZoomIn") {
            videoZoom *= zoomK;
        }
        if (auto *a = qobject_cast<QAction *>(e->sender); a && a->text().contains('%')) {
            videoZoom = a->text().section('%', 0, 0).toDouble() / 100;
        }
        menuItemSet(action("N_VideoZoom_Custom"), true);
        // Zooming through the fitting size returns to Letterbox.
        const double fit = std::min(W / cx, H / cy);
        if (prevZoom > 0 && ((fit > prevZoom && fit < videoZoom) || (fit < prevZoom && fit > videoZoom))) {
            menuItemSet(action("N_VideoZoom_Letterbox"), true);
        }
    }
    videoZoom = std::clamp(videoZoom, 0.02, 16.0);
    layoutVideo();
}

// PV-062 and PV-063.
void MainForm::layoutVideo() {
    if (!video) return;
    if (QAction *c = action("N_VideoZoom_Custom")) {
        const QString base = Lang::instance().tr("N_VideoZoom_Custom");
        c->setText(isChecked(c) ? QStringLiteral("%1 (%2%)").arg(base).arg(std::lround(videoZoom * 100)) : base);
    }
    video->setFlips(isChecked("N_VideoZoom_FlipV"), isChecked("N_VideoZoom_FlipH"));
    const QSize native = nativeVideoSize();
    const double cx = native.width(), cy = native.height();
    const double W = video->width(), H = video->height();
    if (cx <= 0 || cy <= 0 || W <= 0 || H <= 0) {
        video->setFit(VideoView::Fit::Letterbox);
        return;
    }
    if (isChecked("N_VideoZoom_Custom")) {
        const double pw = videoZoom * cx, ph = videoZoom * cy;
        const double bx = std::min(0.5, (W / 2) / pw), by = std::min(0.5, (H / 2) / ph);
        videoDx = std::clamp(videoDx, bx, 1 - bx);
        videoDy = std::clamp(videoDy, by, 1 - by);
        video->setFit(VideoView::Fit::Rect, QRectF(W / 2 - (1 - videoDx) * pw, H / 2 - (1 - videoDy) * ph, pw, ph));
    } else if (isChecked("N_VideoZoom_PanAndScan")) {
        if (cx / cy <= W / H) {
            const double h = W * cy / cx;
            video->setFit(VideoView::Fit::Rect, QRectF(0, (H - h) / 2, W, h));
        } else {
            const double w = H * cx / cy;
            video->setFit(VideoView::Fit::Rect, QRectF((W - w) / 2, 0, w, H));
        }
    } else if (isChecked("N_VideoZoom_Stretch") || isChecked("N_VideoZoom_NotLinear")) {
        video->setFit(VideoView::Fit::Stretch);
    } else {
        video->setFit(VideoView::Fit::Letterbox);
    }
}

// PV-064.
void MainForm::N_VideoZoom_LetterboxClick(vcl::Event *e) {
    auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (a && !a->isEnabled()) return;
    if (a) menuItemClick(a);
    layoutVideo();
}

// PV-065.
void MainForm::N_VideoZoom_OtherClick(vcl::Event *) {
    const Lang &lang = Lang::instance();
    bool ok = false;
    const double v = QInputDialog::getDouble(this, lang.tr("Mes_CustomZoomTitle"), lang.tr("Mes_CustomZoomText") + ":",
                                             videoZoom * 100, 2, 1600, 2, &ok);
    if (!ok) return;
    videoZoom = std::clamp(v / 100, 0.02, 16.0);
    menuItemSet(action("N_VideoZoom_Custom"), true);
    layoutVideo();
}

// --- Video mouse ------------------------------------------------------------------------------

// PV-020, PV-066, PV-067.
void MainForm::VideoWindowWMRMouseDown(vcl::Event *e) {
    if (!e) return;
    if (e->button == Qt::MiddleButton) {
        N_FullScreenClick(nullptr);
        return;
    }
    if (e->button != Qt::LeftButton) return;
    if (isChecked("N_VideoZoom_Custom")) {
        videoGrab = true;
        grabPos = QCursor::pos();
        return;
    }
    if (isChecked("N_Playback_PauseOnClick") && playback->active()) N_Playback_PlayClick(nullptr);
}

// PV-066, PV-068, PV-073.
void MainForm::VideoWindowWMRMouseMove(vcl::Event *e) {
    hideDetails();
    if (video && video->cursor().shape() == Qt::BlankCursor) video->setCursor(Qt::ArrowCursor);
    if (QTimer *t = timer("TimerMouseHide")) {
        auto *play = find<QToolButton>("ButtonPlay");
        if (play && play->isChecked()) t->start();
    }
    if (fullScreen && fullScreen->isVisible() && e) {
        QWidget *pv = widget("PanelVideo");
        auto *w = qobject_cast<QWidget *>(e->sender);
        const int y = w && pv ? w->mapTo(pv, e->pos).y() : 0;
        if (pv && y >= pv->height() - 3) showTimelineInFullScreen(true);
        else if (pv && y < pv->height() - 50) showTimelineInFullScreen(false);
    }
    if (videoGrab && video) {
        const QPoint now = QCursor::pos();
        const QPoint d = now - grabPos;
        grabPos = now;
        const QRectF r = video->pictureRect();
        if (r.width() > 0 && r.height() > 0) {
            videoDx += d.x() / r.width();
            videoDy += d.y() / r.height();
            layoutVideo();
        }
    }
}

void MainForm::VideoWindowWMRMouseUp(vcl::Event *) {
    if (!videoGrab) return;
    videoGrab = false;
    layoutVideo();
}

void MainForm::TimerMouseHideTimer(vcl::Event *e) {
    if (auto *t = e ? qobject_cast<QTimer *>(e->sender) : nullptr) t->stop();
    if (video && video->underMouse()) video->setCursor(Qt::BlankCursor);
}

// --- Full screen ------------------------------------------------------------------------------

// PV-070…PV-072.
void MainForm::N_FullScreenClick(vcl::Event *e) {
    QObject *s = e ? e->sender : nullptr;
    const bool videoSurface = s && (s == widget("PanelVideo") || s == video);
    if (videoSurface && e->name == "OnDblClick" && isChecked("N_Playback_PauseOnClick")) return;
    QWidget *pv = widget("PanelVideo");
    if (!pv) return;
    showTimelineInFullScreen(false);
    const bool on = !isChecked("N_FullScreen");
    menuItemSet(action("N_FullScreen"), on);
    if (on) {
        if (!fullScreen) {
            fullScreen = new vcl::Panel(nullptr);
            fullScreen->setWindowFlags(Qt::Window | Qt::FramelessWindowHint);
            fullScreen->setWindowTitle(QStringLiteral("Full Screen"));
            fullScreen->setBevels(vcl::Panel::Bevel::None, vcl::Panel::Bevel::None, 1, 0);
            auto *l = new vcl::AlignLayout(fullScreen, QSize(800, 600));
            fullScreen->setAlignLayout(l);
            QPalette pal = fullScreen->palette();
            pal.setColor(QPalette::Window, Qt::black);
            fullScreen->setPalette(pal);
            fullScreen->setAutoFillBackground(true);
            // PV-074: the main window's shortcuts work here, and closing it closes the app.
            std::function<void(QMenu *)> share = [&](QMenu *m) {
                for (QAction *a : m->actions()) {
                    if (a->menu()) share(a->menu());
                    else if (!a->shortcut().isEmpty()) fullScreen->addAction(a);
                }
            };
            for (QAction *top : mainMenu()->actions()) {
                if (top->menu()) share(top->menu());
            }
            fullScreen->installEventFilter(this);
        }
        reparent(pv, fullScreen, Align::Client);
        if (QScreen *screen = this->screen()) fullScreen->setGeometry(screen->geometry());
        fullScreen->showFullScreen();
        fullScreen->activateWindow();
    } else if (fullScreen) {
        fullScreen->hide();
        reparent(pv, widget("PanelLeft"), Align::Client);
        activateWindow();
    }
    layoutVideo();
    TimerTimer(nullptr);
}

// PV-073: the timeline slides into the full-screen window at the bottom edge.
void MainForm::showTimelineInFullScreen(bool show) {
    QWidget *bottom = widget("PanelBottom");
    QWidget *splitter = widget("SplitterBottom");
    if (!bottom || !fullScreen) return;
    const bool there = bottom->parentWidget() == fullScreen;
    if (show == there) return;
    QWidget *to = show ? fullScreen : widget("PanelLeft");
    reparent(bottom, to, Align::Bottom);
    if (splitter) reparent(splitter, to, Align::Bottom);
    FormResize(nullptr);
}

// --- Mouse wheel and extra keys ---------------------------------------------------------------

// PV-075 / GM-075. Returns true when the wheel was used here.
bool MainForm::routeWheel(QWheelEvent *e) {
    QWidget *under = QApplication::widgetAt(e->globalPosition().toPoint());
    const int d = e->angleDelta().y() / 120;
    auto zoomEvent = [&](const char *senderName) {
        vcl::Event ev;
        ev.sender = component(senderName);
        return ev;
    };
    if (isInside(under, widget("PanelVideo"))) {
        if (d != 0) {
            zoomK = 1.2;
            vcl::Event ev = zoomEvent(d > 0 ? "N_VideoZoom_ZoomIn" : "N_VideoZoom_ZoomOut");
            ButtonVideoZoomOutClick(&ev);
            zoomK = 2;
        }
        return true;
    }
    if (isInside(under, widget("PanelBottomGraphics"))) {
        if (d != 0) {
            zoomK = 1.5;
            vcl::Event ev = zoomEvent(d > 0 ? "ButtonZoomIn" : "ButtonZoomOut");
            ButtonZoomInClick(&ev);
            zoomK = 2;
        }
        return true;
    }
    if (isInside(under, widget("PanelZoomScroll"))) {
        if (auto *bar = find<QScrollBar>("ScrollBarZoom"); bar && d != 0) {
            bar->setValue(std::clamp(bar->value() - 10 * d, 0, bar->maximum()));
        }
        return true;
    }
    if (isInside(under, widget("PanelControl"))) {
        auto *play = find<QToolButton>("ButtonPlay");
        if (d != 0 && play && play->isEnabled()) {
            if (playback->state() == Playback::State::Running) {
                vcl::Event ev = zoomEvent(d > 0 ? "N_Playback_Left" : "N_Playback_Right");
                N_Playback_LeftClick(&ev);
            } else if (d > 0) {
                ButtonFramePrevClick(nullptr);
            } else {
                ButtonFrameNextClick(nullptr);
            }
        }
        return true;
    }
    if (MapView *map = mapView(); map && isInside(under, map)) return mapWheel(e->angleDelta().y(), e->globalPosition().toPoint());
    return false;  // the list scrolls itself
}

// PV-081 (ApplicationEventsShortCut L4411–L4445).
bool MainForm::extraKey(QKeyEvent *k) {
    const bool ctrl = k->modifiers() & Qt::ControlModifier;
    const bool keypad = k->modifiers() & Qt::KeypadModifier;
    auto zoom = [&](const char *senderName) {
        vcl::Event ev;
        ev.sender = component(senderName);
        const QString name = senderName;
        if (name.startsWith("Button") || name.contains("_Time_")) ButtonZoomInClick(&ev);
        else ButtonVideoZoomOutClick(&ev);
    };
    switch (k->key()) {
    case Qt::Key_Plus:
    case Qt::Key_Equal:
        if (ctrl && keypad) zoom("N_View_Time_ZoomIn");
        else if (!ctrl) zoom("N_VideoZoom_ZoomIn");
        else return false;  // Ctrl+= is the menu shortcut
        return true;
    case Qt::Key_Minus:
        if (ctrl && keypad) zoom("N_View_Time_ZoomOut");
        else if (!ctrl) zoom("N_VideoZoom_ZoomOut");
        else return false;
        return true;
    case Qt::Key_Asterisk:
        if (!keypad) return false;
        zoom("N_VideoZoom_100");
        return true;
    case Qt::Key_Slash: {
        if (!keypad) return false;
        const bool letterbox = isChecked("N_VideoZoom_Letterbox");
        menuItemSet(action(letterbox ? "N_VideoZoom_PanAndScan" : "N_VideoZoom_Letterbox"), true);
        layoutVideo();
        return true;
    }
    case Qt::Key_A:
        if (!ctrl) return false;
        if (FileListView *list = listView()) list->selectAll();
        return true;
    default:
        return false;
    }
}
