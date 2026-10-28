#include "Graphics.h"

#include "i18n/Lang.h"

#include <QLocale>
#include <QPainter>

#include <algorithm>
#include <cmath>

namespace draw {

namespace {

constexpr double kSecond = 1.0 / 86400.0;

int rnd(double v) {
    return int(std::lround(v));
}

// Delphi FloatToStr with the user's decimal separator.
QString floatToStr(double v) {
    if (std::abs(v) < 1e-12) v = 0;
    return QLocale::system().toString(v, 'g', 12);
}

}  // namespace

QString Graphic::menuItem() const {
    return QStringLiteral("N_View_Graphics_") + title();
}

QString Graphic::title() const {
    static const char *names[] = {"S", "H", "G", "V", "X", "Y", "Z"};
    return QString::fromLatin1(names[int(kind_)]);
}

double Graphic::osDelta() const {
    switch (kind_) {
    case Kind::S: return 10;
    case Kind::H: return 5;
    default: return 0.1;
    }
}

QColor Graphic::color() const {
    const Lang &l = Lang::instance();
    switch (kind_) {
    case Kind::S: return l.color("colorGraph_S");
    case Kind::H: return l.color("colorGraph_H");
    case Kind::G:
    case Kind::V: return l.color("colorGraph_G");
    case Kind::X: return l.color("colorGraph_X");
    case Kind::Y: return l.color("colorGraph_Y");
    case Kind::Z: return l.color("colorGraph_Z");
    }
    return Qt::black;
}

// GM-035 yConv.
int Graphic::yConv(double v) const {
    return y0 - rnd(v * (y0 - borderT) / maxV);
}

void Graphic::prepare(QSize size, qreal dpr, int bx, const QVector<Band> &bands, QVector<Bin> bins) {
    width = size.width();
    height = size.height();
    borderX = bx;
    const int th = textHeight(font);
    borderT = height >= 60 ? th : th / 2 + 1;  // TMyGraphic.Resized L103–L111
    borderB = th / 2 + 1;

    // GM-035: zero altitude means "no data".
    if (kind_ == Kind::H) {
        for (Bin &b : bins) {
            if (b.count == 0) continue;
            if (b.min == 0) b.min = b.max;
            if (b.max == 0) b.count = 0;
        }
    }
    maxV = 0;
    bool positiveOnly = true;
    for (int i = 0; i < bins.size(); ++i) {
        const Bin &b = bins[i];
        if (b.count == 0) continue;
        maxV = std::max({maxV, std::abs(b.min), std::abs(b.max)});
        if (i >= 1 && b.min < 0) positiveOnly = false;
    }
    maxV = std::max(maxV, 1.05 * osDelta());
    y0 = positiveOnly ? height - borderB : (height + borderT - borderB) / 2;

    background = newImage(size, dpr, Lang::instance().color("colorGraphBG"));
    QPainter p(&background);
    p.setFont(font);
    drawBands(p, bands, borderX, height);
    drawOs(p);
    drawData(p, std::move(bins));
    drawTopAndBottom(p);
}

// GM-037 (TMyGraphic.DrawOs L255–L294).
void Graphic::drawOs(QPainter &p) {
    const Lang &l = Lang::instance();
    const QColor os = l.color("colorGraphOS"), osm = l.color("colorGraphOSm"), text = l.color("colorGraphText");
    const int th = textHeight(font);
    p.setPen(os);
    lineTo(p, QPointF(borderX, 0), QPointF(borderX, height));
    lineTo(p, QPointF(width - borderX, 0), QPointF(width - borderX, height));
    lineTo(p, QPointF(borderX, y0), QPointF(width - borderX, y0));

    double d = osDelta();
    while ((y0 - borderT) / (maxV / d) < 1.2 * th && d < 1000 * osDelta()) d += osDelta();

    const int n = int(std::ceil(maxV / d));
    for (int i = -n; i <= n; ++i) {
        if (std::abs(i * d) >= maxV && i != 0) continue;
        const int y = yConv(i * d);
        if (y < borderT || y > height - borderB) continue;
        p.setPen(os);
        lineTo(p, QPointF(borderX - 1, y), QPointF(borderX + 1, y));
        const QString s = floatToStr(i * d);
        p.setPen(text);
        textOut(p, borderX - 2 - textWidth(font, s), y - th / 2, s);
        if (y != y0) {
            p.setPen(osm);
            lineTo(p, QPointF(borderX + 1, y), QPointF(width - borderX, y));
        }
    }
    p.setPen(text);
    const QString t = title();
    textOut(p, width - borderX + (borderX - textWidth(font, t)) / 2, (borderT + height - borderB) / 2 - th / 2, t);
}

// GM-038 data trace and GM-039 peak labels (TMyGraphic.DrawBack L158–L245).
void Graphic::drawData(QPainter &p, QVector<Bin> bins) {
    p.setRenderHint(QPainter::Antialiasing);
    p.setPen(QPen(color(), 1));
    int last = -1, lastY1 = 0, lastY2 = 0;
    double lastTime = 0;
    for (int i = 0; i < bins.size(); ++i) {
        const Bin &b = bins[i];
        if (b.count == 0) continue;
        const int y1 = yConv(b.max), y2 = yConv(b.min);
        const int x = borderX + i;
        const bool broken = last < 0 || (i - last >= 4 && std::abs(b.time - lastTime) > 11 * kSecond) ||
                            (b.time == 0 && i - last > 50);
        if (!broken) {
            const int lx = borderX + last;
            if (lastY1 > y2) lineTo(p, QPointF(lx, lastY1), QPointF(x, y2));
            else if (y1 > lastY2) lineTo(p, QPointF(lx, lastY2), QPointF(x, y1));
            else if (i - last > 1) lineTo(p, QPointF(lx, lastY1), QPointF(x, y1));
        }
        lineTo(p, QPointF(x, y1), QPointF(x, y2 + 1));
        last = i;
        lastY1 = y1;
        lastY2 = y2;
        lastTime = b.time;
    }
    p.setRenderHint(QPainter::Antialiasing, false);

    // Peak labels: the five largest hills, each label rounded to a tenth of the axis step.
    const QColor text = Lang::instance().color("colorGraphText");
    const int th = textHeight(font);
    auto dominant = [](const Bin &b) { return std::abs(b.min) > b.max ? b.min : b.max; };
    for (int k = 0; k < 5; ++k) {
        int best = -1;
        for (int i = 0; i < bins.size(); ++i) {
            if (bins[i].count == 0) continue;
            if (best < 0 || std::abs(dominant(bins[i])) > std::abs(dominant(bins[best]))) best = i;
        }
        if (best < 0) break;
        const double lv = dominant(bins[best]);
        const double q = osDelta() / 10;
        const double rounded = std::round(lv / q) * q;
        const QString s = floatToStr(rounded);
        const int x = borderX + best, y = yConv(lv);
        p.setPen(text);
        lineTo(p, QPointF(x, y - 1), QPointF(x, y + 2));
        lineTo(p, QPointF(x - 1, y), QPointF(x + 2, y));
        if (lv >= 0) textOut(p, x + 2, std::max(y - th, 1), s);
        else textOut(p, x + 2, std::min(y, height - th), s);

        // L225–L242: mark the hill as used. Walk right, then left, emptying columns until at least
        // 2 × label width + 10 columns away and the trace (column maxima, in pixels) rises again.
        const int far = 2 * textWidth(font, s) + 10;
        for (int dir : {1, -1}) {
            int py = 0;
            for (int j = best; j >= 0 && j < bins.size(); j += dir) {
                if (std::abs(j - best) >= far && bins[j].count != 0 && yConv(bins[j].max) < py) break;
                if (bins[j].count > 0) {
                    py = yConv(bins[j].max);
                    bins[j].count = 0;
                }
            }
        }
    }
}

// GM-040.
void Graphic::drawTopAndBottom(QPainter &p) {
    const Lang &l = Lang::instance();
    p.fillRect(QRectF(0, 0, width, 1), l.color("colorGraphBorderT"));
    p.fillRect(QRectF(0, height - 1, width, 1), l.color("colorGraphBorderB"));
}

QImage Graphic::draw(int cursorX) const {
    QImage img = background;
    if (cursorX >= 0) {
        QPainter p(&img);
        p.setCompositionMode(QPainter::RasterOp_NotSourceAndNotDestination);
        p.fillRect(QRectF(cursorX, 0, 1, height), Qt::red);
    }
    return img;
}

}  // namespace draw
