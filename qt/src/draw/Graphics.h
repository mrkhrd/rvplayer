// The timeline graphs under the seek bar, ported from UnitGraphics (TMyGraphic and its seven
// kinds). Spec: graphs-map-layout.md GM-030…GM-041.
#pragma once

#include "Bitmaps.h"

#include <QImage>
#include <QString>
#include <QVector>

namespace draw {

// One pixel column: the samples that fell into it (GM-034). count == 0 is a gap.
struct Bin {
    double min = 0, max = 0;
    int count = 0;
    double time = 0;  // time of the first sample, days
};

class Graphic {
public:
    enum class Kind { S, H, G, V, X, Y, Z };
    static constexpr Kind kAll[] = {Kind::S, Kind::H, Kind::G, Kind::V, Kind::X, Kind::Y, Kind::Z};

    explicit Graphic(Kind kind) : kind_(kind) {}
    Kind kind() const { return kind_; }
    QString menuItem() const;  // N_View_Graphics_S, ...
    QString title() const;     // "S", ...
    double osDelta() const;
    QColor color() const;

    // Rebuilds the cached background (GM-031 steps 2–4): bands, data, axes, peak labels and the
    // top/bottom lines.
    void prepare(QSize size, qreal dpr, int borderX, const QVector<Band> &bands, QVector<Bin> bins);
    // GM-041: the background with the play cursor at column x (none when x < 0).
    QImage draw(int cursorX) const;

private:
    int yConv(double v) const;
    void drawData(QPainter &p, QVector<Bin> bins);
    void drawOs(QPainter &p);
    void drawTopAndBottom(QPainter &p);

    Kind kind_;
    QImage background;
    QFont font = defaultFont(8);
    int width = 0, height = 0;
    int borderX = 0, borderT = 0, borderB = 0;
    int y0 = 0;
    double maxV = 1;
};

}  // namespace draw
