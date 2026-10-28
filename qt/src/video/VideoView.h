// The video picture inside PanelVideo (the original's VMR/EVR window): the current frame drawn
// into the picture rectangle chosen by the zoom mode, on black. Spec: playback-video.md §3.11.
#pragma once

#include <QImage>
#include <QWidget>

class VideoView : public QWidget {
    Q_OBJECT
public:
    enum class Fit { Letterbox, Stretch, Rect };

    explicit VideoView(QWidget *parent = nullptr);
    void setFrame(const QImage &frame);
    void clear();
    QSize frameSize() const { return frame.size(); }

    // PV-062: Letterbox keeps the aspect ratio inside the widget, Stretch fills it, Rect draws
    // the picture into `rect` (Custom zoom and Pan&Scan; parts outside the widget are cut off).
    void setFit(Fit fit, const QRectF &rect = {});
    void setFlips(bool vertical, bool horizontal);
    // Rectangle the picture occupies, in widget coordinates.
    QRectF pictureRect() const;

protected:
    void paintEvent(QPaintEvent *e) override;

private:
    QImage frame;
    Fit fit = Fit::Letterbox;
    QRectF target;
    bool flipV = false, flipH = false;
};
