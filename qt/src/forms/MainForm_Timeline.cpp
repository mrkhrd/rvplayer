// Seek bar, graphs, gauges and the play-speed buttons of TFormMain.
// Spec: graphs-map-layout.md §3.4–§3.8, playback-video.md §3.8.

#include "MainForm.h"

#include "core/Settings.h"
#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"
#include "vcl/Widgets.h"
#include "core/Playback.h"

#include <QAction>
#include <QButtonGroup>
#include <QPainter>
#include <QToolButton>

#include <algorithm>

using vcl::Align;

// PV-050 (CreateSpeedButtons L658–L683, SetupSpeedBtn L1054–L1068): six radio buttons with tags
// −3, −1, 0, 3, 15, 63 for ¼, ½, 1, 4, 16 and 64 times.
void MainForm::createSpeedButtons() {
    auto *panel = find<vcl::Panel>("PanelPlaySpeed");
    if (!panel) return;
    panel->setBevels(vcl::Panel::Bevel::None, vcl::Panel::Bevel::None, 1, 0);
    // The panel is disabled until a file is selected, but its speed buttons keep their normal
    // look as VCL graphic controls do; clicks are refused in TrackBarPlaySpeedChange.
    panel->setEnabled(true);
    const QSize design = controlSize(panel);
    const int w = design.width() / 6;
    auto *group = new QButtonGroup(panel);
    group->setExclusive(true);
    int x = 0;
    for (int tag : {-3, -1, 0, 3, 15, 63}) {
        auto *b = new QToolButton(panel);
        b->setObjectName(QStringLiteral("SpeedButton%1").arg(tag));
        b->setCheckable(true);
        b->setChecked(tag == 0);
        b->setFocusPolicy(Qt::NoFocus);
        b->setProperty("tag", tag);
        setSpeedButtonTag(b, tag);
        b->setToolTip(panel->toolTip());
        group->addButton(b);
        layoutOf(panel)->addControl(b, Align::None, vcl::Anchors{}, QRect(x, 0, w, design.height()));
        x += w;
        bindHandler(b, "OnClick", "TrackBarPlaySpeedChange");
        speedButtons.append(b);
    }
}

// SetupSpeedBtn (L1054–L1068): the label is drawn into the glyph, so it is never elided.
void MainForm::setSpeedButtonTag(QToolButton *b, int tag) {
    b->setProperty("tag", tag);
    const QString s = tag < 0 ? QStringLiteral("1/%1").arg(1 - tag) : QStringLiteral("%1x").arg(tag + 1);
    const QFont f = draw::vclFont(Lang::instance().trDef("LangFont", "Arial"), 7, true);
    const QSize size(draw::textWidth(f, s), draw::textHeight(f));
    const qreal dpr = devicePixelRatioF();
    QPixmap glyph(size * dpr);
    glyph.setDevicePixelRatio(dpr);
    glyph.fill(Qt::transparent);
    QPainter p(&glyph);
    p.setFont(f);
    p.setPen(palette().color(QPalette::ButtonText));
    draw::textOut(p, 0, 0, s);
    p.end();
    b->setIcon(QIcon(glyph));
    b->setIconSize(size);
    b->setToolButtonStyle(Qt::ToolButtonIconOnly);
}

namespace {

double rateOfTag(int d) {
    return d >= 0 ? d + 1 : 1.0 / (1 - d);
}

}  // namespace

// PV-052: rate d + 1 for tags d >= 0, 1 / (1 - d) below. The speed is not persisted.
void MainForm::TrackBarPlaySpeedChange(vcl::Event *e) {
    auto *b = e ? qobject_cast<QToolButton *>(e->sender) : nullptr;
    if (!b) return;
    if (!playSpeedEnabled) {
        // PanelPlaySpeed is disabled: the click is refused and the current speed stays down.
        for (QToolButton *other : std::as_const(speedButtons)) {
            if (std::abs(rateOfTag(other->property("tag").toInt()) - playback->rate()) < 1e-9) other->setChecked(true);
        }
        return;
    }
    playback->setRate(rateOfTag(b->property("tag").toInt()));
}

// --- Graphs -----------------------------------------------------------------------------------

// GM-030.
void MainForm::N_View_Graphics_SClick(vcl::Event *e) {
    auto *src = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (src) menuItemClick(src);
    rebuildGraphs(src != nullptr);
}

// GM-033 (F6 hides, F7 shows).
void MainForm::N_View_Graphics_ShowClick(vcl::Event *e) {
    auto *src = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    bool b = src && src->objectName() == "N_View_Graphics_Show";
    for (const char *name : {"N_View_Graphics_S", "N_View_Graphics_H"}) {
        if (QAction *a = action(name)) menuItemSet(a, b);
    }
    if (QAction *g = action("N_View_Graphics_G"); g && !g->isVisible()) b = false;
    for (const char *name : {"N_View_Graphics_G", "N_View_Graphics_V", "N_View_Graphics_X", "N_View_Graphics_Y",
                             "N_View_Graphics_Z"}) {
        if (QAction *a = action(name)) menuItemSet(a, b);
    }
    rebuildGraphs(true);
}

void MainForm::rebuildGraphs(bool user) {
    QWidget *panel = widget("PanelBottomGraphics");
    if (!panel) return;
    for (GraphImage &g : graphs) {
        layoutOf(panel)->removeWidget(g.image);
        delete g.image;
    }
    graphs.clear();
    for (draw::Graphic::Kind k : draw::Graphic::kAll) {
        draw::Graphic graphic(k);
        if (!isChecked(graphic.menuItem())) continue;
        auto *img = new vcl::Image(panel);
        img->setObjectName("Graphic_" + graphic.title());
        layoutOf(panel)->addControl(img, Align::None, vcl::Anchors{}, QRect(0, 0, panel->width(), 5));
        for (const char *ev : {"OnMouseDown", "OnMouseMove", "OnMouseUp"}) {
            bindHandler(img, ev, QString(ev).replace("On", "ImageSeek"));
        }
        attachPopup(img, "PopupMenuGraphics");
        img->show();
        graphs.push_back({graphic, img});
    }
    if (QWidget *s = widget("SplitterBottom")) s->setVisible(!graphs.empty());

    if (user) {
        QWidget *bottom = widget("PanelBottom");
        QWidget *seekImage = widget("ImageSeek");
        QWidget *zoom = widget("PanelZoomScroll");
        if (bottom && seekImage && zoom) {
            const int base = controlSize(seekImage).height() + controlSize(zoom).height();
            QSize s = controlSize(bottom);
            if (graphs.empty()) s.setHeight(base);
            else if (graphs.size() == std::size(draw::Graphic::kAll) && s.height() <= base + 150) s.setHeight(320);
            else if (s.height() < 186) s.setHeight(185);
            setControlSize(bottom, s);
        }
        persistSplitters();
    }
    FormResize(nullptr);
}

// GM-031: the graphs share the space under the seek bar; the last one takes the remainder.
void MainForm::layoutGraphs() {
    QWidget *panel = widget("PanelBottomGraphics");
    QWidget *seekImage = widget("ImageSeek");
    if (!panel || !seekImage || graphs.empty()) return;
    vcl::AlignLayout *l = layoutOf(panel);
    const int top0 = l->controlRect(seekImage).height();
    const int count = int(graphs.size());
    const int h = std::max((panel->height() - top0) / std::max(1, count), 5);
    for (int i = 0; i < count; ++i) {
        const int top = top0 + i * h;
        const int height = i == count - 1 ? std::max(panel->height() - top, 5) : h;
        l->setControlRect(graphs[i].image, QRect(0, top, panel->width(), height));
    }
}

void MainForm::drawTimeline() {
    auto *seekImage = find<vcl::Image>("ImageSeek");
    QWidget *panel = widget("PanelBottomGraphics");
    if (!seekImage || !panel || panel->width() <= 0) return;
    const qreal dpr = devicePixelRatioF();
    const QColor bg = Lang::instance().color("colorMainBG");
    const int w = panel->width();
    const QVector<draw::Band> b = bands();
    const Group empty;
    const Group &group = playback ? playback->group() : empty;
    seek.prepare(QSize(w, layoutOf(panel)->controlRect(seekImage).height()), dpr, bg, b, group.start(), group.finish,
                 zoomL, zoomR, tzShift);
    vcl::AlignLayout *l = layoutOf(panel);
    const int columns = std::max(w - 2 * seek.border(), 0);
    const bool british = isChecked("N_View_Measu_British");
    for (GraphImage &g : graphs) {
        const QRect r = l->controlRect(g.image);
        QVector<draw::Bin> bins = group.isEmpty()
                                      ? QVector<draw::Bin>(columns)
                                      : moving.bins(g.graphic.kind(), group.timeAt(zoomL), group.timeAt(zoomR), columns, british);
        g.graphic.prepare(QSize(w, r.height()), dpr, seek.border(), b, std::move(bins));
    }
    drawCursors();
}

void MainForm::drawCursors() {
    if (auto *seekImage = find<vcl::Image>("ImageSeek")) seekImage->setImage(seek.draw(position));
    const int x = position < 0 ? -1 : seek.xOf(position);
    for (GraphImage &g : graphs) g.image->setImage(g.graphic.draw(x));
}

// --- Gauges -----------------------------------------------------------------------------------

// GM-080 step 4 (ClearAll): no needles, empty boxes, G-sensor markers off the image.
void MainForm::clearGauges() {
    const qreal dpr = devicePixelRatioF();
    const QColor bg = Lang::instance().color("colorMainBG");
    if (auto *speed = find<vcl::Image>("ImageSpeed")) {
        speedBitmap.prepare(QImage(":/original/images/FORM_MAIN_SPEED.BMP"), bg);
        speed->setImage(speedBitmap.draw(dpr, -1, QString(), 0));
    }
    if (auto *az = find<vcl::Image>("ImageAzimut")) az->setImage(azimutBitmap.draw(dpr, bg, -1));
    if (auto *g = find<vcl::Image>("ImageGSensor")) g->setImage(gsensorBitmap.draw(dpr, bg, false));
}
