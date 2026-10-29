#include "FormDetails.h"

#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"
#include "vcl/Widgets.h"

#include <QFontMetrics>
#include <QLabel>
#include <QMouseEvent>
#include <QScreen>

#include <cmath>

namespace {

constexpr int kGraphHeight = 90;

// Caption label → value label; the caption ends 2 px before its value (FormCreate L122–L138).
const char *const kPairs[][2] = {
    {"LabelSystemTimeCaption", "LabelSystemTime"}, {"LabelVideoTimeCaption", "LabelVideoTime"},
    {"LabelFileCaption", "LabelFile"},             {"LabelGPS", "LabelGPSIndex"},
    {"LabelGPSXCaption", "LabelGPSX"},             {"LabelGPSYCaption", "LabelGPSY"},
    {"LabelGPSAltitudeCaption", "LabelGPSAltitude"}, {"LabelGPSSpeedCaption", "LabelGPSSpeed"},
    {"LabelGPSAzimutCaption", "LabelGPSAzimut"},   {"LabelGPSTrackCaption", "LabelGPSTrack"},
    {"LabelGSensorXCaption", "LabelGSensorX"},     {"LabelGSensorYCaption", "LabelGSensorY"},
    {"LabelGSensorZCaption", "LabelGSensorZ"},     {"LabelGSensorGCaption", "LabelGSensorG"},
};

// " 0.123" / "-0.123" (GM-122).
QString signedValue(double v) {
    return (v < 0 ? QStringLiteral("-") : QStringLiteral(" ")) + QString::number(std::abs(v), 'f', 3);
}

}  // namespace

FormDetails::FormDetails(QWidget *parent) : vcl::Form(parent) {
    // GM-121: borderless, white, 220/255 opaque; shown without taking the focus.
    setWindowFlags(Qt::ToolTip | Qt::FramelessWindowHint);
    setAttribute(Qt::WA_ShowWithoutActivating);
    setWindowOpacity(220.0 / 255.0);
}

void FormDetails::FormCreate(vcl::Event *) {
    QPalette pal = palette();
    pal.setColor(QPalette::Window, Qt::white);
    setPalette(pal);
    setAutoFillBackground(true);
    // The 1-px black frame around the whole form.
    if (auto *border = find<QFrame>("ShapeBorder")) {
        border->setFrameStyle(QFrame::Box | QFrame::Plain);
        border->setLineWidth(1);
        QPalette bp = border->palette();
        bp.setColor(QPalette::WindowText, Qt::black);
        border->setPalette(bp);
        layoutOf(this)->setAlign(border, vcl::Align::Client);
        border->lower();
    }
    const Lang &l = Lang::instance();
    setLabel("LabelSystemTimeCaption", l.tr("SystemTime") + ":");
    setLabel("LabelVideoTimeCaption", l.tr("VideoTime") + ":");
    setLabel("LabelFileCaption", l.tr("File") + ":");
    setLabel("LabelGPSXCaption", l.tr("Longitude") + ":");
    setLabel("LabelGPSYCaption", l.tr("Latitude") + ":");
    setLabel("LabelGPSAltitudeCaption", l.tr("Altitude") + ":");
    setLabel("LabelGPSAzimutCaption", l.tr("Azimut") + ":");
    setLabel("LabelGPSSpeedCaption", l.tr("Speed") + ":");
    setLabel("LabelGPSTrackCaption", l.tr("Details_Track_Length") + ":");
}

void FormDetails::setLabel(const char *name, const QString &text) {
    if (auto *label = find<QLabel>(name)) {
        label->setText(text);
        label->adjustSize();
    }
}

// FormCreate L117–L138: the time block's values start after its widest caption; every caption
// ends 2 px before its value.
void FormDetails::alignCaptions() {
    vcl::AlignLayout *l = layoutOf(this);
    int widest = 0;
    for (const char *name : {"LabelSystemTimeCaption", "LabelVideoTimeCaption", "LabelFileCaption"}) {
        if (auto *c = find<QLabel>(name)) widest = std::max(widest, c->sizeHint().width());
    }
    for (const char *name : {"LabelSystemTime", "LabelVideoTime", "LabelFile"}) {
        if (auto *v = find<QLabel>(name)) {
            QRect r = l->controlRect(v);
            r.moveLeft(widest + 4 + 2);
            l->setControlRect(v, r);
        }
    }
    for (const auto &[caption, value] : kPairs) {
        auto *c = find<QLabel>(caption);
        auto *v = find<QLabel>(value);
        if (!c || !v) continue;
        QRect cr = l->controlRect(c);
        cr.setWidth(c->sizeHint().width());
        cr.moveLeft(l->controlRect(v).left() - 2 - cr.width());
        l->setControlRect(c, cr);
        QRect vr = l->controlRect(v);
        vr.setWidth(v->sizeHint().width());
        l->setControlRect(v, vr);
    }
}

// GM-121: what each layout profile shows.
void FormDetails::applyProfile(Profile p) {
    for (QObject *o : children()) {
        auto *w = qobject_cast<QWidget *>(o);
        if (!w || w->objectName() == "ShapeBorder" || w->objectName() == "Image" || w->objectName() == "LabelGraphicName") {
            continue;
        }
        const QString n = w->objectName();
        bool visible = true;
        if (n.contains("GSensor")) visible = p == Profile::GpsG;
        else if (n.contains("GPS")) visible = p != Profile::Video;
        w->setVisible(visible);
    }
}

void FormDetails::popup(const Content &c, QPoint cursor, int panelTop, int panelBottom) {
    const Lang &lang = Lang::instance();
    const MovingValue &v = c.value;
    applyProfile(c.profile);
    const QString p = v.gps && !v.gpsGood ? QStringLiteral("(?) ") : QString();

    // The file name is shortened in the middle until it fits.
    if (auto *file = find<QLabel>("LabelFile")) {
        const int room = width() - layoutOf(this)->controlRect(file).left() - 6;
        file->setText(QFontMetrics(file->font()).elidedText(c.file, Qt::ElideMiddle, std::max(room, 20)));
        file->adjustSize();
    }
    setLabel("LabelSystemTime", c.systemTime);
    setLabel("LabelVideoTime", c.videoTime);
    if (v.gps) {
        setLabel("LabelGPSIndex", QStringLiteral("[%1]").arg(v.gpsIndex));
        setLabel("LabelGPSX", p + (v.lon < 0 ? "W" : "E") + QString::number(std::abs(v.lon), 'f', 6));
        setLabel("LabelGPSY", p + (v.lat < 0 ? "S" : "N") + QString::number(std::abs(v.lat), 'f', 6));
        if (v.alt == 0) setLabel("LabelGPSAltitude", p);
        else if (c.british) setLabel("LabelGPSAltitude", p + QString::number(std::lround(v.alt / 0.3048)) + " " + lang.tr("Feets"));
        else setLabel("LabelGPSAltitude", p + QString::number(std::lround(v.alt)) + " " + lang.tr("Meters"));
        setLabel("LabelGPSAzimut", v.course < 0 ? p : p + QString::number(std::lround(v.course)) + QChar(0x00B0));
        const double speed = c.british ? v.speed / 1.609344 : v.speed;
        const double tenth = std::trunc(speed * 10) / 10;  // at most one decimal, truncated, no ".0"
        setLabel("LabelGPSSpeed", p + QString::number(tenth, 'g', 10) + " " + lang.tr(c.british ? "MilesPerH" : "KmetersPerH"));
        if (c.british) {
            setLabel("LabelGPSTrack", p + QString::number(v.track / 1000 / 1.609344, 'f', 3) + " " + lang.tr("Miles"));
        } else {
            QString m = QString::number(std::lround(v.track));
            if (m.size() > 3) m.insert(m.size() - 3, ' ');
            setLabel("LabelGPSTrack", p + m + " " + lang.tr("Meters"));
        }
    } else {
        for (const char *n : {"LabelGPSIndex", "LabelGPSX", "LabelGPSY", "LabelGPSAltitude", "LabelGPSAzimut", "LabelGPSSpeed",
                              "LabelGPSTrack"}) {
            setLabel(n, QString());
        }
    }
    if (v.gsensor) {
        setLabel("LabelGSensorIndex", QStringLiteral("[%1]").arg(v.gIndex));
        setLabel("LabelGSensorX", signedValue(v.x));
        setLabel("LabelGSensorY", signedValue(v.y));
        setLabel("LabelGSensorZ", signedValue(v.z));
        setLabel("LabelGSensorG", signedValue(std::sqrt(v.x * v.x + v.y * v.y + v.z * v.z)));
    } else {
        for (const char *n : {"LabelGSensorIndex", "LabelGSensorX", "LabelGSensorY", "LabelGSensorZ", "LabelGSensorG"}) {
            setLabel(n, QString());
        }
    }
    alignCaptions();

    // Size: the time block only (Video profile), the values, or the values and the mini-graph.
    hasGraph = c.graph.has_value() && c.profile != Profile::Video;
    auto *image = find<vcl::Image>("Image");
    auto *graphName = find<QLabel>("LabelGraphicName");
    int h = c.profile == Profile::Video ? 45 : 116;
    if (graphName) graphName->setVisible(hasGraph);
    if (image) image->setVisible(hasGraph);
    if (hasGraph && image) {
        setLabel("LabelGraphicName", c.graphName);
        const int top = 113;
        const QSize size(width() - 2, kGraphHeight);
        layoutOf(this)->setControlRect(image, QRect(QPoint(1, top), size));
        graphic = draw::Graphic(*c.graph);
        graphic.prepare(size, devicePixelRatioF(), 20, {}, c.bins);
        h = top + kGraphHeight + 1;
        image->setImage(graphic.draw(c.cursorColumn < 0 ? -1 : 20 + c.cursorColumn));
    }
    resize(width(), h);

    // GM-123: right of the cursor, under the graphs, or above them when there is no room.
    const QRect work = screen() ? screen()->availableGeometry() : QRect(0, 0, 1920, 1080);
    int top = std::max(cursor.y() + 24, panelBottom);
    if (!hasGraph || top + h > work.bottom()) top = panelTop - h;
    move(cursor.x() + 16, top);
    if (!isVisible()) show();
}

void FormDetails::popdown() {
    hide();
}

void FormDetails::drawCursor(int column) {
    if (!hasGraph || !isVisible()) return;
    if (auto *image = find<vcl::Image>("Image")) image->setImage(graphic.draw(column < 0 ? -1 : 20 + column));
}

void FormDetails::mouseMoveEvent(QMouseEvent *e) {
    vcl::Event ev;
    ev.pos = e->position().toPoint();
    FormMouseMove(&ev);
}

// GM-123: the popup jumps away when the mouse reaches it.
void FormDetails::FormMouseMove(vcl::Event *e) {
    if (!e) return;
    const QRect work = screen() ? screen()->availableGeometry() : QRect(0, 0, 1920, 1080);
    move(x() + e->pos.x() + 16, std::min(y() + e->pos.y() + 24, work.bottom() - height()));
}
