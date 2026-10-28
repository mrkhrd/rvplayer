// Gauges and the seek bar, ported from UnitBitmaps (TSpeedBitmap, TAzimutBitmap, TGSensorBitmap,
// TSeekBitmap). Coordinates are the original's 96-dpi pixels; the images are rendered at the
// screen's device pixel ratio so lines and text stay sharp on high-DPI displays.
#pragma once

#include <QColor>
#include <QFont>
#include <QImage>
#include <QString>
#include <QVector>

class QPainter;

namespace draw {

// A Delphi font: Size is in points at 96 dpi, so the pixel height is Size * 96 / 72.
QFont vclFont(const QString &family, int size, bool bold = false);
// The VCL canvas default, "MS Sans Serif" 8.
QFont defaultFont(int size = 8);

// Blank image of a logical size for painting at the given device pixel ratio.
QImage newImage(QSize size, qreal dpr, const QColor &fill);

// Delphi TCanvas.TextOut: (x, y) is the top-left corner of the text cell.
void textOut(QPainter &p, double x, double y, const QString &s);
int textWidth(const QFont &f, const QString &s);
int textHeight(const QFont &f);

// Delphi MoveTo/LineTo: the end point is not drawn.
void lineTo(QPainter &p, QPointF from, QPointF to);

// A coloured column range over the full height (GM-063). x2 == x1 + 1 is a 1-px line.
struct Band {
    int x1 = 0, x2 = 0;
    QColor fill, line;
};
// TMyBitmap.DrawDelections: bands shifted right by `border`.
void drawBands(QPainter &p, const QVector<Band> &bands, int border, int height);

// GM-081. 134×100, needle pivot (67, 67).
class SpeedBitmap {
public:
    void prepare(const QImage &dial, const QColor &bg);
    // speed < 0: no needle and empty boxes (the ClearAll state).
    QImage draw(qreal dpr, double speed, const QString &track, double altitude) const;

private:
    QImage background;
};

// GM-082. 57×57 compass, heading-up.
class AzimutBitmap {
public:
    QImage draw(qreal dpr, const QColor &bg, double bearing) const;  // bearing < 0: no needle
};

// GM-083. 65×65 three-axis accelerometer.
class GSensorBitmap {
public:
    QImage draw(qreal dpr, const QColor &bg, bool valid, double x = 0, double y = 0, double z = 0) const;
};

// GM-060…GM-062. The seek bar: time rows, groove and the pin with its two time bubbles.
class SeekBitmap {
public:
    SeekBitmap();
    int border() const { return border_; }
    int pinH() const { return pinH_; }

    // Times in days. minP/maxP is the visible zoom window as fractions of [minTime, maxTime].
    // `shift` is the time-zone shift added to the clock row.
    void prepare(QSize size, qreal dpr, const QColor &bg, const QVector<Band> &bands, double minTime, double maxTime,
                 double minP, double maxP, double shift);
    // Background plus the pin at position p (fraction of the whole range); p < 0: no pin.
    QImage draw(double p) const;
    // GM-064: position (fraction of the whole range) at pixel column x.
    double progressAt(int x) const;
    // Pixel column of position p, or -1 outside the visible window.
    int xOf(double p) const;

private:
    QFont font;
    int border_ = 0;
    int pinH_ = 10;
    QImage background;
    double minTime = 0, maxTime = 0, minP = 0, maxP = 1, shift = 0;
};

// TimeToStrSmall: "1d02:03", "2:03", "3" (minutes), plus ":ss" when `seconds`.
QString timeToStrSmall(double days, bool seconds);

}  // namespace draw
