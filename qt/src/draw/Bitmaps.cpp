#include "Bitmaps.h"

#include "i18n/Lang.h"

#include <QFontMetrics>
#include <QLocale>
#include <QPainter>
#include <QPainterPath>
#include <QPalette>
#include <QApplication>
#include <QTime>

#include <algorithm>
#include <cmath>

namespace draw {

namespace {

constexpr double kSecond = 1.0 / 86400.0;
constexpr double kMinute = 1.0 / 1440.0;

// Delphi Round: banker's rounding is irrelevant at pixel precision, so round half away from zero.
int rnd(double v) {
    return int(std::lround(v));
}

// TCanvas.RoundRect(x1, y1, x2, y2, w, h): the right and bottom edges are exclusive and w/h are
// the corner ellipse's diameters.
void roundRect(QPainter &p, double x1, double y1, double x2, double y2, double w, double h) {
    p.drawRoundedRect(QRectF(x1 + 0.5, y1 + 0.5, x2 - x1 - 1, y2 - y1 - 1), w / 2, h / 2);
}

void ellipse(QPainter &p, double x1, double y1, double x2, double y2) {
    p.drawEllipse(QRectF(x1 + 0.5, y1 + 0.5, x2 - x1 - 1, y2 - y1 - 1));
}

// TCanvas.FloodFill(x, y, colour, fsSurface): repaint the 4-connected area of `target` around (x, y).
void floodFill(QImage &img, QPoint start, QRgb target, QRgb with) {
    if (!img.rect().contains(start) || img.pixel(start) != target || target == with) return;
    QVector<QPoint> stack{start};
    while (!stack.isEmpty()) {
        const QPoint pt = stack.takeLast();
        if (!img.rect().contains(pt) || img.pixel(pt) != target) continue;
        img.setPixel(pt, with);
        stack << QPoint(pt.x() + 1, pt.y()) << QPoint(pt.x() - 1, pt.y()) << QPoint(pt.x(), pt.y() + 1)
              << QPoint(pt.x(), pt.y() - 1);
    }
}

QColor btnShadow() {
    return Lang::vclColor("clBtnShadow");
}

QColor btnHighlight() {
    return Lang::vclColor("clBtnHighlight");
}

}  // namespace

QFont vclFont(const QString &family, int size, bool bold) {
    QFont f(family);
    f.setPixelSize(std::max(1, rnd(size * 96.0 / 72.0)));
    f.setBold(bold);
    return f;
}

QFont defaultFont(int size) {
    return vclFont(QStringLiteral("Microsoft Sans Serif"), size);
}

QImage newImage(QSize size, qreal dpr, const QColor &fill) {
    QImage img(std::max(1, int(std::ceil(size.width() * dpr))), std::max(1, int(std::ceil(size.height() * dpr))),
               QImage::Format_RGB32);
    img.setDevicePixelRatio(dpr);
    img.fill(fill);
    return img;
}

void textOut(QPainter &p, double x, double y, const QString &s) {
    p.drawText(QPointF(x, y + QFontMetricsF(p.font()).ascent()), s);
}

int textWidth(const QFont &f, const QString &s) {
    return QFontMetrics(f).horizontalAdvance(s);
}

int textHeight(const QFont &f) {
    return QFontMetrics(f).height();
}

void lineTo(QPainter &p, QPointF from, QPointF to) {
    if (from.x() == to.x()) {
        const double len = std::abs(to.y() - from.y());
        p.fillRect(QRectF(from.x(), to.y() < from.y() ? to.y() + 1 : from.y(), 1, len), p.pen().color());
        return;
    }
    if (from.y() == to.y()) {
        const double len = std::abs(to.x() - from.x());
        p.fillRect(QRectF(to.x() < from.x() ? to.x() + 1 : from.x(), from.y(), len, 1), p.pen().color());
        return;
    }
    p.drawLine(from + QPointF(0.5, 0.5), to + QPointF(0.5, 0.5));
}

void drawBands(QPainter &p, const QVector<Band> &bands, int border, int height) {
    for (const Band &b : bands) {
        if (b.x2 == b.x1 + 1) {
            p.fillRect(QRectF(border + b.x1, 0, 1, height), b.line);
        } else if (b.x2 > b.x1) {
            p.fillRect(QRectF(border + b.x1, 0, b.x2 - b.x1, height), b.fill);
        }
    }
}

// --- TSpeedBitmap -----------------------------------------------------------------------------

void SpeedBitmap::prepare(const QImage &dial, const QColor &bg) {
    background = dial.convertToFormat(QImage::Format_RGB32);
    const int w = background.width(), h = background.height();
    // L149–L152: the four corners take the panel colour.
    for (QPoint c : {QPoint(w - 1, h - 1), QPoint(w - 1, 0), QPoint(0, h - 1), QPoint(0, 0)}) {
        floodFill(background, c, background.pixel(0, 0), bg.rgb());
    }
}

QImage SpeedBitmap::draw(qreal dpr, double speed, const QString &track, double altitude) const {
    QImage img(background.size() * dpr, QImage::Format_RGB32);
    img.setDevicePixelRatio(dpr);
    QPainter p(&img);
    p.setRenderHint(QPainter::SmoothPixmapTransform);
    p.drawImage(QRectF(QPointF(0, 0), background.size()), background);
    p.setRenderHint(QPainter::Antialiasing);

    const QColor arrow = Lang::instance().color("colorSpeedArrow");
    if (speed >= 0) {
        // L171–L181: a triangle with its tip at radius 60 and its base across the pivot.
        const double ang = (180.0 + std::clamp(speed, 0.0, 200.0)) * M_PI / 180.0;  // Q-15: kept on the dial
        const double si = std::sin(ang), co = std::cos(ang);
        const QPointF c(67.5, 67.5);
        const QPointF base(6 * si, -6 * co);
        QPainterPath needle;
        needle.moveTo(c + base);
        needle.lineTo(c + QPointF(60 * co, 60 * si));
        needle.lineTo(c - base);
        needle.closeSubpath();
        p.setPen(QPen(arrow, 1));
        p.setBrush(arrow);
        p.drawPath(needle);
    }

    p.setPen(QPen(Lang::instance().color("colorSpeedText"), 1));
    p.setBrush(Lang::instance().color("colorSpeedBG"));
    roundRect(p, 37, 56, 97, 95, 10, 10);
    roundRect(p, 42, 40, 92, 54, 5, 5);
    roundRect(p, 50, 26, 84, 38, 5, 5);
    if (speed >= 0) {
        p.setFont(vclFont("Arial", 22));
        p.drawText(QRectF(37, 56, 60, 40), Qt::AlignCenter, QString::number(rnd(speed)));
        p.setFont(vclFont("Arial", 8));
        p.drawText(QRectF(0, 40, 88, 14), Qt::AlignRight | Qt::AlignVCenter, track);
        if (rnd(altitude) != 0) p.drawText(QRectF(50, 26, 34, 12), Qt::AlignCenter, QString::number(rnd(altitude)));
    }
    return img;
}

// --- TAzimutBitmap ----------------------------------------------------------------------------

QImage AzimutBitmap::draw(qreal dpr, const QColor &bg, double bearing) const {
    const int w = 57, h = 57;
    QImage img = newImage(QSize(w, h), dpr, bg);
    QPainter p(&img);
    p.setRenderHint(QPainter::Antialiasing);

    // L227–L243: white disc, grey rim, black outline and three ticks.
    p.setBrush(Qt::white);
    p.setPen(QPen(QColor(150, 150, 150), 2));
    p.drawEllipse(QRectF(1, 1, w - 2, h - 2));
    p.setBrush(Qt::NoBrush);
    p.setPen(QPen(Qt::black, 1));
    ellipse(p, 0, 0, w, h);
    p.setPen(QPen(Qt::black, 1));
    lineTo(p, QPointF(0, h / 2), QPointF(4, h / 2));
    lineTo(p, QPointF(w - 1, h / 2), QPointF(w - 5, h / 2));
    lineTo(p, QPointF(w / 2, h - 1), QPointF(w / 2, h - 5));

    const QPointF c(w / 2.0, h / 2.0);
    if (bearing >= 0) {
        const double a = bearing * M_PI / 180.0;
        const QPointF dir(std::sin(a), std::cos(a));
        const QPointF side(-dir.y() * 7, dir.x() * 7);
        const double r = 27.5 - 2;
        auto half = [&](QPointF tip, const QColor &fill, const QColor &line) {
            QPainterPath path;
            path.moveTo(c + side);
            path.lineTo(c + tip);
            path.lineTo(c - side);
            path.closeSubpath();
            p.setPen(QPen(line, 1));
            p.setBrush(fill);
            p.drawPath(path);
        };
        half(dir * r, Qt::red, QColor(255, 100, 100));
        half(-dir * r, Qt::blue, QColor(80, 80, 255));

        p.setPen(QPen(Qt::black, 1));
        p.setBrush(Qt::white);
        roundRect(p, w / 2 + 2, h / 4 - 3, w / 2 + 19, h / 4 + 7, 5, 5);
        p.setFont(vclFont("Arial", 6));
        p.drawText(QRectF(w / 2 + 2, h / 4 - 3, 17, 10), Qt::AlignCenter, QString::number(rnd(bearing) % 360));
    }

    // Heading mark: centre to top, a small triangle at the top and the hub.
    p.setPen(QPen(Qt::black, 1));
    p.drawLine(c, QPointF(c.x(), 4));
    QPainterPath mark;
    mark.moveTo(c.x(), 1);
    mark.lineTo(c.x() - 3.5, 7);
    mark.lineTo(c.x() + 3.5, 7);
    mark.closeSubpath();
    p.setPen(Qt::NoPen);
    p.setBrush(Qt::black);
    p.drawPath(mark);
    p.drawEllipse(c, 2.5, 2.5);
    return img;
}

// --- TGSensorBitmap ---------------------------------------------------------------------------

QImage GSensorBitmap::draw(qreal dpr, const QColor &bg, bool valid, double x, double y, double z) const {
    const int w = 65;
    const int oneG = w / 2 - 10;                // 22 px per g
    const int oneGY = rnd(oneG / 1.4);          // 16 px per g on the diagonal
    QImage img = newImage(QSize(w, w), dpr, bg);
    QPainter p(&img);
    p.setRenderHint(QPainter::Antialiasing);
    p.setPen(QPen(Qt::black, 1));
    p.setFont(vclFont("Arial", 8));
    const QFontMetricsF fm(p.font());

    // An axis through `o` along unit vector `d`, ±(1 g + 5 px), with arrowheads, a 7-px cross at
    // the origin and ticks at ±1 g.
    auto axis = [&](QPointF o, QPointF d, double g) {
        const QPointF n(-d.y(), d.x());
        const double len = g + 5;
        const QPointF a = o - d * len, b = o + d * len;
        p.drawLine(a, b);
        for (const auto &[tip, dir] : {std::pair{a, -d}, std::pair{b, d}}) {
            p.drawLine(tip, tip - dir * 3 + n * 2.5);
            p.drawLine(tip, tip - dir * 3 - n * 2.5);
        }
        p.drawLine(o - n * 3.5, o + n * 3.5);
        for (double s : {-g, g}) p.drawLine(o + d * s - n * 2, o + d * s + n * 2);
    };
    const QPointF ox(29.5, 60.5), oz(60.5, 29.5), oy(25.5, 25.5);
    const QPointF diag(-std::sqrt(0.5), std::sqrt(0.5));
    axis(ox, QPointF(1, 0), oneG);
    axis(oz, QPointF(0, 1), oneG);
    axis(oy, diag, oneGY * std::sqrt(2.0));
    p.drawText(QPointF(ox.x() - fm.horizontalAdvance("x") / 2, ox.y() - 3), "x");
    p.drawText(QPointF(oz.x() - 4 - fm.horizontalAdvance("z"), oz.y() + fm.ascent() / 2 - 1), "z");
    p.drawText(QPointF(oy.x() - 3 - fm.horizontalAdvance("y"), oy.y() - 2), "y");

    if (valid) {
        p.setBrush(Qt::white);
        auto marker = [&](QPointF c) { p.drawEllipse(c, 2.5, 2.5); };
        marker(QPointF(ox.x() - oneG * x, ox.y()));
        marker(QPointF(oz.x(), oz.y() + oneG * z));
        marker(QPointF(oy.x() - oneGY * y, oy.y() + oneGY * y));
    }
    return img;
}

// --- TSeekBitmap ------------------------------------------------------------------------------

QString timeToStrSmall(double days, bool seconds) {
    const qint64 totalSec = qint64(std::floor(days * 86400.0 + 1e-6));
    const qint64 d = totalSec / 86400, h = totalSec / 3600 % 24, m = totalSec / 60 % 60, s = totalSec % 60;
    QString r;
    if (d > 0) r = QStringLiteral("%1d%2:%3").arg(d).arg(h, 2, 10, QChar('0')).arg(m, 2, 10, QChar('0'));
    else if (h > 0) r = QStringLiteral("%1:%2").arg(h).arg(m, 2, 10, QChar('0'));
    else r = QString::number(m);
    if (seconds) r += QStringLiteral(":%1").arg(s, 2, 10, QChar('0'));
    return r;
}

namespace {

// Locale long time (Delphi TimeToStr).
QString clockText(double days) {
    const double frac = days - std::floor(days);
    const int ms = int(std::lround(frac * 86400000.0)) % 86400000;
    return QLocale::system().toString(QTime::fromMSecsSinceStartOfDay(ms), QStringLiteral("H:mm:ss"));
}

}  // namespace

SeekBitmap::SeekBitmap() : font(defaultFont(8)) {
    border_ = textWidth(font, QStringLiteral("99:99:99")) / 2 + 4;  // L405
}

void SeekBitmap::prepare(QSize size, qreal dpr, const QColor &bg, const QVector<Band> &bands, double minT,
                         double maxT, double pL, double pR, double sh) {
    minTime = minT;
    maxTime = maxT;
    minP = pL;
    maxP = pR;
    shift = sh;
    background = newImage(size, dpr, bg);
    QPainter p(&background);
    p.setFont(font);
    const int W = size.width(), H = size.height();
    drawBands(p, bands, border_, H);

    const double t1 = minP * (maxTime - minTime) + minTime + shift;
    const double t2 = maxP * (maxTime - minTime) + minTime + shift;
    const double base = minTime + shift;
    const bool seconds = std::abs(t2 - t1) < 4 * kMinute;  // L427
    const int th = textHeight(font);
    if (t1 < t2) {
        p.setPen(Qt::black);
        const int w = std::max(W - 2 * border_, 1);
        auto xAt = [&](double t) { return border_ + rnd((t - t1) / (t2 - t1) * w); };

        // L434–L455: the upper row, time since the start of the group.
        int maxCnt = w / (textWidth(font, seconds ? "99:99" : "99") + 20);
        const double step = seconds ? kSecond : kMinute;
        double dt = step;
        while (std::floor((t2 - t1) / dt) > maxCnt) dt += step;
        int prevX = -100;
        for (double t = std::ceil((t1 - base) / dt) * dt; t < t2 - base; t += dt) {
            const int x = xAt(t + base);
            const QString s = timeToStrSmall(t, seconds);
            if (textWidth(font, s) < x - prevX) {
                textOut(p, x - textWidth(font, s) / 2, H / 2 - pinH_ / 2 - th, s);
                lineTo(p, QPointF(x, H / 2), QPointF(x, H / 2 - pinH_ / 2 - 2));
                prevX = x;
            }
        }

        // L458–L478: the lower row, clock time on whole steps of the day.
        maxCnt = w / (textWidth(font, "99:99:99") + 20);
        dt = step;
        while (std::floor((t2 - t1) / dt) > maxCnt) dt += step;
        prevX = -100;
        const double day = std::floor(t1);
        for (double t = day + std::ceil((t1 - day) / dt) * dt; t < t2; t += dt) {
            const int x = xAt(t);
            const QString s = clockText(t);
            if (textWidth(font, s) < x - prevX) {
                textOut(p, x - textWidth(font, s) / 2, H / 2 + pinH_ / 2 + 1, s);
                lineTo(p, QPointF(x, H / 2), QPointF(x, H / 2 + pinH_ / 2 + 1));
                prevX = x;
            }
        }
    }

    // L482–L485: the groove.
    p.setRenderHint(QPainter::Antialiasing);
    p.setPen(QPen(Qt::black, 1));
    p.setBrush(btnShadow());
    roundRect(p, border_ - pinH_ / 2, H / 2 - pinH_ / 2, W - border_ + pinH_ / 2, H / 2 + pinH_ / 2, pinH_ - 1,
              pinH_ - 1);
}

int SeekBitmap::xOf(double pos) const {
    if (pos < minP || pos > maxP || maxP <= minP) return -1;
    const int W = int(background.width() / background.devicePixelRatio());
    return border_ + rnd((pos - minP) / (maxP - minP) * (W - 2 * border_));
}

double SeekBitmap::progressAt(int x) const {
    const int W = int(background.width() / background.devicePixelRatio());
    const double f = std::clamp(double(x - border_) / std::max(1, W - 2 * border_), 0.0, 1.0);
    return minP + f * (maxP - minP);
}

QImage SeekBitmap::draw(double pos) const {
    QImage img = background;
    const int x = xOf(pos);
    if (x < 0 || maxTime <= minTime) return img;
    const int H = int(background.height() / background.devicePixelRatio());
    QPainter p(&img);
    p.setFont(font);
    const int h2 = H / 2, ph = pinH_ / 2;

    // L512–L514: the pin's stem.
    p.setPen(Qt::black);
    lineTo(p, QPointF(x, h2 + ph + 5), QPointF(x, h2 - ph - 5));

    // L515–L522: elapsed time; finer fractions when one pixel is less than a tenth of a second.
    const double span = (maxP - minP) * (maxTime - minTime);
    const double perPx = span * 86400.0 / std::max(1, int(background.width() / background.devicePixelRatio()) - 2 * border_);
    const double elapsed = pos * (maxTime - minTime);
    QString top = timeToStrSmall(elapsed, true);
    const double secFrac = elapsed * 86400.0 - std::floor(elapsed * 86400.0 + 1e-6);
    if (perPx < 0.01) top += QStringLiteral(".%1").arg(int(secFrac * 1000) % 1000, 3, 10, QChar('0'));
    else if (perPx < 0.1) top += QStringLiteral(".%1").arg(int(secFrac * 10) % 10);
    const QString bottom = clockText(minTime + elapsed + shift);

    p.setRenderHint(QPainter::Antialiasing);
    const QColor bubbleLine(100, 100, 100);
    auto bubble = [&](const QString &s, int y1, int y2, int ty) {
        const int w = textWidth(font, s);
        p.setPen(QPen(bubbleLine, 1));
        p.setBrush(Qt::white);
        roundRect(p, x - w / 2 - 4, y1, x + w / 2 + 5, y2, 5, 5);
        p.setPen(Qt::black);
        textOut(p, x - w / 2, ty, s);
    };
    bubble(top, 0, h2 - ph - 2, -2);
    bubble(bottom, h2 + ph + 2, H, h2 + ph + 2);

    // L541–L543: the pin.
    p.setPen(QPen(Qt::black, 1));
    p.setBrush(btnHighlight());
    ellipse(p, x - ph, h2 - ph, x + ph, h2 + ph);
    return img;
}

}  // namespace draw
