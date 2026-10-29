#include "Widgets.h"

#include "AlignLayout.h"

#include <QKeySequence>
#include <QMouseEvent>
#include <QPainter>

namespace vcl {

namespace {

// Windows virtual-key codes used by Delphi shortcuts, mapped to Qt keys.
int qtKeyFromVk(int vk) {
    if (vk >= 'A' && vk <= 'Z') return Qt::Key_A + (vk - 'A');
    if (vk >= '0' && vk <= '9') return Qt::Key_0 + (vk - '0');
    if (vk >= 0x70 && vk <= 0x87) return Qt::Key_F1 + (vk - 0x70);  // F1..F24
    if (vk >= 0x60 && vk <= 0x69) return Qt::Key_0 + (vk - 0x60);  // numpad digits
    switch (vk) {
    case 0x08: return Qt::Key_Backspace;
    case 0x09: return Qt::Key_Tab;
    case 0x0D: return Qt::Key_Return;
    case 0x1B: return Qt::Key_Escape;
    case 0x20: return Qt::Key_Space;
    case 0x21: return Qt::Key_PageUp;
    case 0x22: return Qt::Key_PageDown;
    case 0x23: return Qt::Key_End;
    case 0x24: return Qt::Key_Home;
    case 0x25: return Qt::Key_Left;
    case 0x26: return Qt::Key_Up;
    case 0x27: return Qt::Key_Right;
    case 0x28: return Qt::Key_Down;
    case 0x2D: return Qt::Key_Insert;
    case 0x2E: return Qt::Key_Delete;
    case 0x6A: return Qt::Key_Asterisk;
    case 0x6B: return Qt::Key_Plus;
    case 0x6D: return Qt::Key_Minus;
    case 0x6E: return Qt::Key_Period;
    case 0x6F: return Qt::Key_Slash;
    case 0xBB: return Qt::Key_Equal;
    case 0xBC: return Qt::Key_Comma;
    case 0xBD: return Qt::Key_Minus;
    case 0xBE: return Qt::Key_Period;
    default: return 0;
    }
}

}  // namespace

QKeySequence shortcutFromDelphi(int code) {
    const int key = qtKeyFromVk(code & 0xFF);
    if (!key) return {};
    int mods = 0;
    if (code & 0x2000) mods |= Qt::SHIFT;
    if (code & 0x4000) mods |= Qt::CTRL;
    if (code & 0x8000) mods |= Qt::ALT;
    return QKeySequence(key | mods);
}

QPixmap transparentBitmap(const QImage &image) {
    if (image.isNull()) return {};
    QImage img = image.convertToFormat(QImage::Format_ARGB32);
    const QRgb clear = img.pixel(0, img.height() - 1);
    for (int y = 0; y < img.height(); ++y) {
        auto *line = reinterpret_cast<QRgb *>(img.scanLine(y));
        for (int x = 0; x < img.width(); ++x) {
            if ((line[x] & 0xFFFFFF) == (clear & 0xFFFFFF)) line[x] = 0;
        }
    }
    return QPixmap::fromImage(img);
}

Panel::Panel(QWidget *parent) : QFrame(parent) {
    setFrameShape(QFrame::NoFrame);
}

void Panel::setBevels(Bevel o, Bevel i, int width, int border) {
    outer = o;
    inner = i;
    bevelWidth = width;
    const int inset = (outer != Bevel::None ? bevelWidth : 0) + border + (inner != Bevel::None ? bevelWidth : 0);
    if (layout_) layout_->setAlignMargins(QMargins(inset, inset, inset, inset));
    update();
}

void Panel::paintEvent(QPaintEvent *e) {
    QFrame::paintEvent(e);
    QPainter p(this);
    const QColor light = palette().color(QPalette::Light);
    const QColor dark = palette().color(QPalette::Dark);
    auto bevel = [&](QRect r, Bevel b) {
        if (b == Bevel::None) return;
        const QColor tl = b == Bevel::Raised ? light : dark;
        const QColor br = b == Bevel::Raised ? dark : light;
        for (int k = 0; k < bevelWidth; ++k) {
            p.setPen(tl);
            p.drawLine(r.topLeft(), r.topRight());
            p.drawLine(r.topLeft(), r.bottomLeft());
            p.setPen(br);
            p.drawLine(r.bottomLeft(), r.bottomRight());
            p.drawLine(r.topRight(), r.bottomRight());
            r.adjust(1, 1, -1, -1);
        }
    };
    bevel(rect(), outer);
    if (inner != Bevel::None) bevel(rect().adjusted(bevelWidth, bevelWidth, -bevelWidth, -bevelWidth), inner);
}

void Panel::mousePressEvent(QMouseEvent *e) {
    e->accept();
}

void Panel::mouseReleaseEvent(QMouseEvent *e) {
    e->accept();
}

void Panel::mouseDoubleClickEvent(QMouseEvent *e) {
    e->accept();
}

void Panel::mouseMoveEvent(QMouseEvent *e) {
    e->accept();
}

Splitter::Splitter(QWidget *parent) : QWidget(parent) {}

QWidget *Splitter::target() const {
    auto *parentLayout = parentWidget() ? qobject_cast<AlignLayout *>(parentWidget()->layout()) : nullptr;
    if (!parentLayout) return nullptr;
    const Align a = parentLayout->alignOf(this);
    // The resized control is the visible sibling with the same alignment touching the splitter
    // on the side it is docked to.
    const QRect me = geometry();
    for (QObject *o : parentWidget()->children()) {
        auto *w = qobject_cast<QWidget *>(o);
        if (!w || w == this || w->isHidden() || parentLayout->alignOf(w) != a) continue;
        const QRect g = w->geometry();
        if ((a == Align::Right && g.left() == me.right() + 1) || (a == Align::Left && g.right() == me.left() - 1) ||
            (a == Align::Bottom && g.top() == me.bottom() + 1) || (a == Align::Top && g.bottom() == me.top() - 1)) {
            return w;
        }
    }
    return nullptr;
}

void Splitter::mousePressEvent(QMouseEvent *e) {
    if (e->button() != Qt::LeftButton) return;
    dragTarget = target();
    pressGlobal = e->globalPosition().toPoint();
    if (auto *l = parentWidget() ? qobject_cast<AlignLayout *>(parentWidget()->layout()) : nullptr; l && dragTarget) {
        const Align a = l->alignOf(this);
        startSize = (a == Align::Left || a == Align::Right) ? dragTarget->width() : dragTarget->height();
    }
}

void Splitter::mouseMoveEvent(QMouseEvent *e) {
    auto *l = parentWidget() ? qobject_cast<AlignLayout *>(parentWidget()->layout()) : nullptr;
    if (!dragTarget || !l || !(e->buttons() & Qt::LeftButton)) return;
    const Align a = l->alignOf(this);
    const QPoint d = e->globalPosition().toPoint() - pressGlobal;
    int size = startSize;
    switch (a) {
    case Align::Left: size += d.x(); break;
    case Align::Right: size -= d.x(); break;
    case Align::Top: size += d.y(); break;
    case Align::Bottom: size -= d.y(); break;
    default: return;
    }
    size = std::max(size, minSize);
    QSize s = l->controlRect(dragTarget).size();
    if (a == Align::Left || a == Align::Right) s.setWidth(size);
    else s.setHeight(size);
    l->setControlSize(dragTarget, s);
}

void Splitter::mouseReleaseEvent(QMouseEvent *e) {
    if (e->button() == Qt::LeftButton && dragTarget) {
        dragTarget = nullptr;
        emit moved();
    }
}

Image::Image(QWidget *parent) : QWidget(parent) {}

void Image::setPixmap(const QPixmap &p) {
    pix = p;
    update();
}

void Image::paintEvent(QPaintEvent *) {
    if (pix.isNull()) return;
    QPainter p(this);
    if (stretch) {
        p.drawPixmap(rect(), pix);
    } else {
        const QSize s = pix.deviceIndependentSize().toSize();
        const QPoint at = center ? QPoint((width() - s.width()) / 2, (height() - s.height()) / 2) : QPoint(0, 0);
        p.drawPixmap(at, pix);
    }
}


void Image::mousePressEvent(QMouseEvent *e) {
    e->accept();
}

void Image::mouseReleaseEvent(QMouseEvent *e) {
    e->accept();
}

void Image::mouseDoubleClickEvent(QMouseEvent *e) {
    e->accept();
}

void Image::mouseMoveEvent(QMouseEvent *e) {
    e->accept();
}

}  // namespace vcl
