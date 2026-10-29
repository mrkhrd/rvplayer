#include "VideoView.h"

#include "core/Trace.h"

#include <QPainter>

VideoView::VideoView(QWidget *parent) : QWidget(parent) {
    setAttribute(Qt::WA_OpaquePaintEvent);
    setAutoFillBackground(false);
}

void VideoView::setFrame(const QImage &f) {
    frame = f;
    update();
}

void VideoView::clear() {
    frame = QImage();
    update();
}

void VideoView::setFit(Fit f, const QRectF &r) {
    fit = f;
    target = r;
    update();
}

void VideoView::setFlips(bool vertical, bool horizontal) {
    flipV = vertical;
    flipH = horizontal;
    update();
}

QRectF VideoView::pictureRect() const {
    if (frame.isNull()) return {};
    switch (fit) {
    case Fit::Stretch: return QRectF(this->rect());
    case Fit::Rect: return target;
    case Fit::Letterbox:
    default: {
        const QSizeF s = QSizeF(frame.size()).scaled(QSizeF(size()), Qt::KeepAspectRatio);
        return QRectF(QPointF((width() - s.width()) / 2, (height() - s.height()) / 2), s);
    }
    }
}

void VideoView::paintEvent(QPaintEvent *) {
    RV_TRACE_SCOPE("VideoView::paintEvent");
    QPainter p(this);
    p.fillRect(this->rect(), Qt::black);
    const QRectF r = pictureRect();
    if (r.isEmpty()) return;
    p.setRenderHint(QPainter::SmoothPixmapTransform);
    if (flipV || flipH) {
        // Mirror around the picture's centre.
        p.translate(r.center());
        p.scale(flipH ? -1 : 1, flipV ? -1 : 1);
        p.translate(-r.center());
    }
    p.drawImage(r, frame);
}
