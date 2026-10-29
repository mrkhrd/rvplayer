#include "MainForm.h"

#include "core/Playback.h"
#include "core/Settings.h"
#include "core/Trace.h"
#include "i18n/Lang.h"
#include "map/MapView.h"
#include "vcl/AlignLayout.h"
#include "vcl/Widgets.h"

#include <QAbstractButton>
#include <QApplication>
#include <QDateTime>
#include <QKeyEvent>
#include <QMenu>
#include <QWheelEvent>
#include <QAction>
#include <QEvent>
#include <QMenu>
#include <QSlider>
#include <QToolButton>
#include <cmath>

using vcl::Align;

MainForm::MainForm(QWidget *parent) : vcl::Form(parent) {}

bool MainForm::AppEventFilter::eventFilter(QObject *watched, QEvent *ev) {
    if (ev->type() != QEvent::Wheel && ev->type() != QEvent::KeyPress) return false;
    auto *w = qobject_cast<QWidget *>(watched);
    QWidget *active = QApplication::activeWindow();
    QWidget *fs = form->fullScreen;
    if (!w || w->isWindow() || (w->window() != form && (!fs || w->window() != fs))) return false;
    // The wheel goes to the window under the cursor; keys only to the active window.
    if (ev->type() == QEvent::Wheel) return form->routeWheel(static_cast<QWheelEvent *>(ev));
    if (active != form && (!fs || active != fs)) return false;
    return form->extraKey(static_cast<QKeyEvent *>(ev));
}

// --- Start-up ---------------------------------------------------------------------------------

void MainForm::FormCreate(vcl::Event *) {
    addLanguages();  // L909–L910: generated items take part in the menu restore
    addTimeZones();
    restoreMenuChecks();  // GM-002
    setupGlyphs();
    buildPopupCopies();
    createSpeedButtons();
    // FormCreate L891–L892: two list items reuse the main menu's bitmaps.
    for (const auto &[item, from] : {std::pair{"N_FileList_Save", "N_File_Save_Video"},
                                     std::pair{"N_FileList_Repair", "N_File_Repair_Video"}}) {
        if (QAction *a = action(item), *b = action(from); a && b) a->setIcon(b->icon());
    }
    createFileListMenuButtons();
    setupListView();
    setupPlayback();
    if (QWidget *seekImage = widget("ImageSeek")) bindHandler(seekImage, "OnMouseLeave", "OnImageMouseLeave");

    // GM-011: saved panel sizes; the ads and analytics browsers are never shown (decisions Q-1).
    Settings &s = Settings::instance();
    if (QWidget *right = widget("PanelRight")) {
        setControlSize(right, QSize(std::max(5, s.readInt("panel_Right", 510)), controlSize(right).height()));
    }
    if (QWidget *bottom = widget("PanelBottom")) {
        setControlSize(bottom, QSize(controlSize(bottom).width(), s.readInt("panel_Bottom", 185)));
    }
    browserH = s.readFloat("panel_Browser", 0.6);
    for (const char *name : {"WebBrowserAds", "WebBrowserAnalytics"}) {
        if (QWidget *w = widget(name)) w->hide();
    }
    if (QWidget *video = widget("PanelVideo")) {
        QPalette p = video->palette();
        p.setColor(QPalette::Window, Qt::black);
        video->setPalette(p);
    }

    setupMapMenu();
    playback->setTrimTail(s.readString("trimTailMS", "0").toDouble() / 1000.0);
    if (QAction *brand = action("N_Help_Site_Brand")) brand->setVisible(false);
    if (QAction *site = action("N_Help_Site_My")) site->setVisible(!Lang::instance().trDef("MySite", QString()).isEmpty());
    if (auto *volume = find<QSlider>("TrackBarVolume")) volume->setValue(s.readInt("volume", 100));  // L984
    N_View_TimeZone_LocalClick(nullptr);
    N_View_Refresh_SlowClick(nullptr);
    // Wheel routing and extra keys see every event of the application (FormMouseWheel,
    // ApplicationEvents). A separate filter object: the form itself must stay a per-control filter,
    // or each control's handlers would run twice.
    qApp->installEventFilter(new AppEventFilter(this));
    // GM-012: start-up handler order (Sender = nil).
    N_View_FileListClick(nullptr);
    N_View_Profile_GPSClick(nullptr);
    N_View_Graphics_SClick(nullptr);
    clearGauges();
    doTranslate();

    // Nested panels get their final size after the window's resize event; re-run the parts of
    // FormResize that depend on them when they change.
    for (const char *name : {"PanelControl", "PanelRight2", "PanelZoomScroll", "PanelBottomGraphics", "PanelVideo"}) {
        if (QWidget *w = widget(name)) w->installEventFilter(this);
    }
}

bool MainForm::eventFilter(QObject *watched, QEvent *ev) {
    if (watched == fullScreen && ev->type() == QEvent::Close) {
        close();  // PV-074: closing the full-screen window closes the application
        return false;
    }
    if (ev->type() == QEvent::Resize && watched->isWidgetType() && watched->objectName() == "PanelVideo") layoutVideo();
    if (ev->type() == QEvent::Resize && watched->isWidgetType()) {
        const QString name = watched->objectName();
        if (name == "PanelControl" || name == "PanelRight2" || name == "PanelZoomScroll" || name == "PanelBottomGraphics") {
            FormResize(nullptr);
        }
    }
    return vcl::Form::eventFilter(watched, ev);
}

void MainForm::setupGlyphs() {
    // Speed buttons take their glyph from the BTN_<name>.BMP resource of the same name.
    for (const char *name : {"ButtonPlay", "ButtonStop", "ButtonFramePrev", "ButtonFrameNext", "ButtonVideoZoomIn",
                             "ButtonVideoZoomOut", "ButtonZoomIn", "ButtonZoomOut"}) {
        auto *b = find<QToolButton>(name);
        if (!b) continue;
        const QString file = "BTN_" + QString(name).mid(6).toUpper() + ".BMP";
        const QPixmap strip = vcl::transparentBitmap(QImage(":/original/images/" + file));
        if (strip.isNull()) continue;
        // TSpeedButton.NumGlyphs: the bitmap holds the up, disabled (and down) glyphs side by side.
        const vcl::DfmObject *def = definition(name);
        const int n = std::max(1, int(def ? def->intProp("NumGlyphs", 1) : 1));
        const int w = strip.width() / n;
        QIcon icon;
        icon.addPixmap(crispPixmap(strip.copy(0, 0, w, strip.height())), QIcon::Normal);
        if (n > 1) icon.addPixmap(crispPixmap(strip.copy(w, 0, w, strip.height())), QIcon::Disabled);
        b->setIcon(icon);
        b->setIconSize(QSize(w, strip.height()));
        b->setToolButtonStyle(Qt::ToolButtonIconOnly);
    }
    // FormCreate L913–L914: Play and Stop are flat.
    for (const char *name : {"ButtonPlay", "ButtonStop"}) {
        if (auto *b = find<QToolButton>(name)) b->setAutoRaise(true);
    }
}

void MainForm::doTranslate() {
    // TR-020: the checked language item decides the language.
    if (QMenu *m = menu("N_View_Language")) {
        for (QAction *a : m->actions()) {
            if (isChecked(a)) Lang::instance().setCurrent(a->objectName().mid(16).replace('_', '-'));
        }
    }
    translateMenus();
    // GM-093: "Local System (+3)".
    if (QAction *local = action("N_View_TimeZone_Local")) {
        const int off = int(std::lround(QDateTime::currentDateTime().offsetFromUtc() / 3600.0));
        local->setText(QStringLiteral("%1 (%2%3)").arg(Lang::instance().tr("N_View_TimeZone_Local"), off < 0 ? "-" : "+").arg(std::abs(off)));
    }
    translateListView();
    // Hints that come from translation keys rather than the form (DoTranslate L4700–L4707).
    if (QWidget *w = widget("PanelPlaySpeed")) w->setToolTip(Lang::instance().tr("PlaySpeed"));
    for (QToolButton *b : std::as_const(speedButtons)) b->setToolTip(Lang::instance().tr("PlaySpeed"));
    if (QWidget *w = widget("TrackBarVolume")) w->setToolTip(Lang::instance().tr("Volume"));
    if (auto *c = find<QAbstractButton>("CheckBoxLoop")) {
        c->setText(Lang::instance().tr("N_View_Time_LoopAB"));
        c->setToolTip(Lang::instance().tr("N_View_Time_LoopABHint"));
    }
    // TR-023: the Language item keeps an English hint when another language is active.
    if (Lang::instance().current() != "en") {
        if (QAction *a = action("N_View_Language")) a->setText(a->text() + "   ( Language )");
    }
    // TR-029 (without the DATAKAM brand, see decisions.md).
    setWindowTitle(QStringLiteral("Registrator Viewer"));
}

// --- Layout -----------------------------------------------------------------------------------

MainForm::Profile MainForm::profile() const {
    if (isChecked("N_View_Profile_Video")) return Profile::Video;
    if (isChecked("N_View_Profile_GPS")) return Profile::Gps;
    return Profile::GpsG;
}

bool MainForm::mapVisible() const {
    // GM-150 (reduced by decisions Q-10): any provider except "None", and not the video profile.
    return profile() != Profile::Video && !isChecked("N_View_NoMaps");
}

// GM-016: File List / Control Panel / Details and map visibility; composition of the right column.
void MainForm::N_View_FileListClick(vcl::Event *e) {
    auto *src = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (src) menuItemClick(src);

    const bool list = isChecked("N_View_FileList");
    const bool control = isChecked("N_View_ControlPanel");
    const bool map = mapVisible();
    QWidget *listPanel = widget("PanelListView");
    QWidget *browser = widget("WebBrowser");
    QWidget *listSplitter = widget("SplitterListView");
    QWidget *right = widget("PanelRight");
    QWidget *right2 = widget("PanelRight2");
    if (!listPanel || !browser || !listSplitter || !right || !right2) return;

    listPanel->setVisible(list);
    if (QWidget *pc = widget("PanelControl")) pc->setVisible(control);
    listSplitter->setVisible(list && map);
    browser->setVisible(map);

    // Q-13: the column is shown only when one of its parts is (the original also counted the
    // control panel while it sat next to the graphs).
    const bool controlInColumn = control && widget("PanelControl") && widget("PanelControl")->parentWidget() == right;
    const bool content = list || map || controlInColumn;
    QSize rs = controlSize(right);
    if (!content) rs.setWidth(0);
    else if (rs.width() < 20) rs.setWidth(510);
    setControlSize(right, rs);
    if (QWidget *sr = widget("SplitterRight")) setControlSize(sr, QSize(content ? 7 : 0, controlSize(sr).height()));

    vcl::AlignLayout *l = layoutOf(right2);
    if (list) {
        QRect b = l->controlRect(browser);
        if (map && b.height() == 0) b.setHeight(10);
        b.moveTop(0);
        l->setControlRect(browser, b);
        l->setAlign(browser, Align::Top);
        QRect s = l->controlRect(listSplitter);
        s.moveTop(b.height());
        l->setControlRect(listSplitter, s);
        l->setAlign(listSplitter, Align::Top);
        l->setAlign(listPanel, Align::Client);
    } else {
        l->setAlign(listSplitter, Align::None);
        l->setAlign(listPanel, Align::None);
        l->setAlign(browser, Align::Client);
    }
    if (src) {
        FormResize(nullptr);
        persistSplitters();
    }
}

// GM-020: profile switch; GM-021 control-panel placement.
void MainForm::N_View_Profile_GPSClick(vcl::Event *e) {
    auto *src = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (src) menuItemClick(src);
    const Profile p = profile();

    if (QWidget *g = widget("ImageGSensor")) g->setVisible(p == Profile::GpsG);
    if (QWidget *s = widget("ImageSpeed")) s->setVisible(p != Profile::Video);
    if (QWidget *a = widget("ImageAzimut")) a->setVisible(p != Profile::Video);

    QWidget *pc = widget("PanelControl");
    QWidget *addon = widget("PanelGraphicsAddon");
    if (pc && addon) {
        if (p != Profile::Video) {
            setControlSize(addon, QSize(260, controlSize(addon).height()));
            reparent(pc, addon, Align::Client);
        } else {
            setControlSize(addon, QSize(0, controlSize(addon).height()));
            reparent(pc, widget("PanelRight"), Align::Bottom);
        }
    }

    if (src) {
        // Map provider and graph selection on a user switch (GM-020 step 5; map per decisions Q-11).
        if (p == Profile::Video) setChecked("N_View_NoMaps", true);
        else if (isChecked("N_View_NoMaps")) setChecked("N_View_OpenStreetMaps2", true);
        N_View_FileListClick(nullptr);
        FormResize(nullptr);
        persistSplitters();
    }

    // Menu enable states (steps 6 and 7).
    const bool maps = p != Profile::Video;
    for (const char *name : {"N_View_Maps", "N_View_Arrow", "N_View_TimePoints", "N_View_Graphics", "N_View_Coordinates",
                             "N_View_Measurement"}) {
        if (QAction *a = action(name)) a->setEnabled(maps);
    }
    for (const char *name : {"N_View_Graphics_G", "N_View_Graphics_V", "N_View_Graphics_X", "N_View_Graphics_Y",
                             "N_View_Graphics_Z"}) {
        if (QAction *a = action(name)) a->setVisible(p == Profile::GpsG);
    }
}

// GM-018: window resize.
void MainForm::FormResize(vcl::Event *) {
    RV_TRACE_SCOPE("MainForm::FormResize");
    if (QWidget *pc = widget("PanelControl"); pc && !pc->isHidden()) alignControlPanel();

    // Step 5: the map keeps its fraction of the column height.
    QWidget *browser = widget("WebBrowser");
    QWidget *right2 = widget("PanelRight2");
    if (browser && right2 && !browser->isHidden()) {
        vcl::AlignLayout *l = layoutOf(right2);
        if (l->alignOf(browser) == Align::Top || l->alignOf(browser) == Align::Bottom) {
            QRect b = l->controlRect(browser);
            b.setHeight(int(std::lround(right2->height() * browserH)));
            l->setControlRect(browser, b);
            if (QWidget *s = widget("SplitterListView")) {
                QRect sr = l->controlRect(s);
                sr.moveTop(b.height());
                l->setControlRect(s, sr);
            }
        }
    }

    // Step 6: zoom bar — ButtonZoomIn at the right edge, ScrollBarZoom fills between the buttons.
    QWidget *zoom = widget("PanelZoomScroll");
    QWidget *zin = widget("ButtonZoomIn");
    QWidget *zout = widget("ButtonZoomOut");
    QWidget *bar = widget("ScrollBarZoom");
    if (zoom && zin && zout && bar) {
        vcl::AlignLayout *l = layoutOf(zoom);
        const int w = zoom->width();
        QRect in = l->controlRect(zin);
        in.moveLeft(w - in.width());
        l->setControlRect(zin, in);
        const QRect out = l->controlRect(zout);
        QRect b = l->controlRect(bar);
        b.setLeft(out.right() + 1);
        b.setRight(in.left() - 1);
        l->setControlRect(bar, b);
    }

    // Steps 7–8: graphs below the seek bar, then the seek bar and graph images.
    layoutGraphs();
    drawTimeline();
    layoutVideo();  // PV-061 runs on every resize
}

// GM-021: control-panel flow layout.
void MainForm::alignControlPanel() {
    QWidget *pc = widget("PanelControl");
    vcl::AlignLayout *l = layoutOf(pc);
    vcl::AlignLayout *parentLayout = pc ? layoutOf(pc->parentWidget()) : nullptr;
    if (!pc || !l || !parentLayout) return;

    QList<QWidget *> controls;
    for (const char *name : {"PanelControl_s1", "PanelPlaySpeed", "PanelControl_s2", "TrackBarVolume", "CheckBoxLoop"}) {
        if (QWidget *w = widget(name)) controls.append(w);
    }
    for (const char *name : {"ImageSpeed", "ImageGSensor", "ImageAzimut"}) {
        if (QWidget *w = widget(name); w && !w->isHidden()) controls.append(w);
    }
    if (controls.isEmpty()) return;

    const Align a = parentLayout->alignOf(pc);
    const bool strip = a == Align::Top || a == Align::Bottom;
    const int x0 = 4, y0 = strip ? 4 : 0, dx = 8, dy = 4;
    int panelH = pc->height();
    if (strip) {
        QWidget *speed = widget("ImageSpeed");
        const int pw = controlSize(pc).width() > 0 ? pc->width() : controlSize(pc).width();
        if (!speed || speed->isHidden()) panelH = l->controlRect(controls[0]).height() + 8;
        else panelH = pw >= 621 ? 60 : pw >= 431 ? 106 : 135;
        setControlSize(pc, QSize(controlSize(pc).width(), panelH));
    }

    QRect prev = l->controlRect(controls[0]);
    int w = prev.width();
    int left = x0;
    prev.moveTo(x0, y0);
    l->setControlRect(controls[0], prev);
    for (int i = 1; i < controls.size(); ++i) {
        QWidget *c = controls[i];
        QRect r = l->controlRect(c);
        if (prev.width() + r.width() >= 1.1 * w) {
            if (prev.bottom() + 1 + dy + r.height() <= panelH) {
                r.moveTo(left, prev.bottom() + 1 + dy);
            } else {
                left = left + w + dx;
                w = 0;
                r.moveTo(left, y0);
            }
        } else {
            r.moveTo(prev.right() + 1 + dx, prev.top());
        }
        if (auto *s = qobject_cast<QSlider *>(c); s && s->orientation() == Qt::Horizontal) r.translate(-4, -3);
        if (r.bottom() + 1 > panelH) r.moveBottom(panelH - 1);
        l->setControlRect(c, r);
        w = std::max(w, r.right() + 1 - left);
        prev = r;
    }
    if (!strip) {
        if (QWidget *addon = widget("PanelGraphicsAddon"); addon && widget("PanelBottom")) {
            setControlSize(addon, QSize(std::min(left + w, widget("PanelBottom")->width() / 2), controlSize(addon).height()));
        }
    }
}

// --- Splitters --------------------------------------------------------------------------------

void MainForm::persistSplitters() {
    Settings &s = Settings::instance();
    if (QWidget *right = widget("PanelRight"); right && right->width() > 0) s.writeInt("panel_Right", right->width());
    if (QWidget *bottom = widget("PanelBottom")) s.writeInt("panel_Bottom", bottom->height());
    s.writeFloat("panel_Browser", browserH);
}

// GM-014
void MainForm::SplitterRightMoved(vcl::Event *) {
    FormResize(nullptr);
    persistSplitters();
}

// GM-015
void MainForm::SplitterListViewMoved(vcl::Event *e) {
    QWidget *browser = widget("WebBrowser");
    QWidget *right2 = widget("PanelRight2");
    if (!browser || !right2 || right2->height() <= 20 || browser->isHidden()) return;
    const Align a = layoutOf(right2)->alignOf(browser);
    if (a != Align::Top && a != Align::Bottom) return;
    browserH = double(browser->height()) / right2->height();
    if (e) SplitterRightMoved(nullptr);
}
