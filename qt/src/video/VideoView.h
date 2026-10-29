// The video picture inside PanelVideo (the original's VMR/EVR window): the current frame drawn
// into the picture rectangle chosen by the zoom mode, on black. Spec: playback-video.md §3.11.
#pragma once

#include <QImage>
#include <QMouseEvent>
#include <QWidget>

class VideoView : public QWidget {
    Q_OBJECT
public:
    enum class Fit { Letterbox, Stretch, Rect };

    explicit VideoView(QWidget *parent = nullptr);
    void setFrame(const QImage &frame);
    void clear();
    QSize frameSize() const { return frame.size(); }
    const QImage &currentFrame() const { return frame; }

    // PV-062: Letterbox keeps the aspect ratio inside the widget, Stretch fills it, Rect draws
    // the picture into `rect` (Custom zoom and Pan&Scan; parts outside the widget are cut off).
    void setFit(Fit fit, const QRectF &rect = {});
    void setFlips(bool vertical, bool horizontal);
    // Rectangle the picture occupies, in widget coordinates.
    QRectF pictureRect() const;

protected:
    void paintEvent(QPaintEvent *e) override;
    // Mouse input stays with the picture, as with the original's video window (not PanelVideo too).
    void mousePressEvent(QMouseEvent *e) override { e->accept(); }
    void mouseReleaseEvent(QMouseEvent *e) override { e->accept(); }
    void mouseDoubleClickEvent(QMouseEvent *e) override { e->accept(); }
    void mouseMoveEvent(QMouseEvent *e) override { e->accept(); }

private:
    QImage frame;
    Fit fit = Fit::Letterbox;
    QRectF target;
    bool flipV = false, flipH = false;
};
