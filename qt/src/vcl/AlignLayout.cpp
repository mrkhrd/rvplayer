#include "AlignLayout.h"

#include <QWidget>
#include <algorithm>

namespace vcl {

Align parseAlign(const QString &ident) {
    if (ident == "alTop") return Align::Top;
    if (ident == "alBottom") return Align::Bottom;
    if (ident == "alLeft") return Align::Left;
    if (ident == "alRight") return Align::Right;
    if (ident == "alClient") return Align::Client;
    if (ident == "alCustom") return Align::Custom;
    return Align::None;
}

Anchors parseAnchors(const QStringList &set) {
    Anchors a{false, false, false, false};
    for (const QString &s : set) {
        if (s == "akLeft") a.left = true;
        if (s == "akTop") a.top = true;
        if (s == "akRight") a.right = true;
        if (s == "akBottom") a.bottom = true;
    }
    return a;
}

AlignLayout::AlignLayout(QWidget *parent, QSize designClientSize) : QLayout(parent), designClient(designClientSize) {
    setContentsMargins(0, 0, 0, 0);
    setSpacing(0);
}

AlignLayout::~AlignLayout() {
    for (Entry &e : entries) {
        delete e.item;
    }
}

void AlignLayout::addControl(QWidget *w, Align align, Anchors anchors, QRect designRect) {
    addChildWidget(w);
    entries.append({new QWidgetItem(w), align, anchors, designRect, nextOrder++});
    invalidate();
}

void AlignLayout::setAlignMargins(QMargins m) {
    alignMargins = m;
    invalidate();
}

AlignLayout::Entry *AlignLayout::find(const QWidget *w) {
    for (Entry &e : entries) {
        if (e.item->widget() == w) return &e;
    }
    return nullptr;
}

const AlignLayout::Entry *AlignLayout::find(const QWidget *w) const {
    for (const Entry &e : entries) {
        if (e.item->widget() == w) return &e;
    }
    return nullptr;
}

QRect AlignLayout::controlRect(const QWidget *w) const {
    const Entry *e = find(w);
    return e ? e->rect : QRect();
}

void AlignLayout::setControlSize(QWidget *w, QSize size) {
    if (Entry *e = find(w)) {
        e->rect.setSize(size);
        invalidate();
    }
}

void AlignLayout::setControlRect(QWidget *w, QRect r) {
    if (Entry *e = find(w)) {
        e->rect = r;
        invalidate();
    }
}

Align AlignLayout::alignOf(const QWidget *w) const {
    const Entry *e = find(w);
    return e ? e->align : Align::None;
}

void AlignLayout::setAlign(QWidget *w, Align align) {
    if (Entry *e = find(w)) {
        e->align = align;
        invalidate();
    }
}

void AlignLayout::addItem(QLayoutItem *item) {
    entries.append({item, Align::None, Anchors{}, item->geometry(), nextOrder++});
}

QLayoutItem *AlignLayout::itemAt(int index) const {
    return index >= 0 && index < entries.size() ? entries[index].item : nullptr;
}

QLayoutItem *AlignLayout::takeAt(int index) {
    if (index < 0 || index >= entries.size()) return nullptr;
    QLayoutItem *item = entries[index].item;
    entries.removeAt(index);
    return item;
}

int AlignLayout::count() const {
    return int(entries.size());
}

QSize AlignLayout::sizeHint() const {
    return designClient;
}

QSize AlignLayout::minimumSize() const {
    return {0, 0};
}

void AlignLayout::setGeometry(const QRect &r) {
    QLayout::setGeometry(r);
    QRect area = r.marginsRemoved(alignMargins);

    // VCL processes alignments in this order; within one alignment, controls are ordered by
    // their position (the one nearest the edge first), then by creation order.
    const Align sequence[] = {Align::Top, Align::Bottom, Align::Left, Align::Right, Align::Client};
    for (Align a : sequence) {
        QList<Entry *> group;
        for (Entry &e : entries) {
            QWidget *w = e.item->widget();
            if (e.align == a && w && !w->isHidden()) group.append(&e);
        }
        std::stable_sort(group.begin(), group.end(), [a](const Entry *x, const Entry *y) {
            switch (a) {
            case Align::Top: return x->rect.top() < y->rect.top();
            case Align::Bottom: return x->rect.bottom() > y->rect.bottom();
            case Align::Left: return x->rect.left() < y->rect.left();
            case Align::Right: return x->rect.right() > y->rect.right();
            default: return x->order < y->order;
            }
        });
        for (Entry *e : group) {
            QRect g;
            switch (a) {
            case Align::Top:
                g = QRect(area.left(), area.top(), area.width(), e->rect.height());
                area.setTop(area.top() + e->rect.height());
                break;
            case Align::Bottom:
                g = QRect(area.left(), area.bottom() + 1 - e->rect.height(), area.width(), e->rect.height());
                area.setBottom(area.bottom() - e->rect.height());
                break;
            case Align::Left:
                g = QRect(area.left(), area.top(), e->rect.width(), area.height());
                area.setLeft(area.left() + e->rect.width());
                break;
            case Align::Right:
                g = QRect(area.right() + 1 - e->rect.width(), area.top(), e->rect.width(), area.height());
                area.setRight(area.right() - e->rect.width());
                break;
            default:
                g = area;
                break;
            }
            if (g.width() < 0) g.setWidth(0);
            if (g.height() < 0) g.setHeight(0);
            e->item->setGeometry(g);
            // Keep the requested position in sync, as VCL updates Left/Top of aligned controls.
            e->rect.moveTopLeft(g.topLeft() - r.topLeft());
        }
    }
    lastClient = area;

    // Non-aligned controls: design position, adjusted by anchors against the design client size.
    for (Entry &e : entries) {
        if (e.align != Align::None && e.align != Align::Custom) continue;
        QWidget *w = e.item->widget();
        if (!w || w->isHidden()) continue;
        QRect g = e.rect;
        const int dw = r.width() - designClient.width();
        const int dh = r.height() - designClient.height();
        if (e.anchors.right) {
            if (e.anchors.left) g.setWidth(std::max(0, g.width() + dw));
            else g.moveLeft(g.left() + dw);
        } else if (!e.anchors.left) {
            g.moveLeft(g.left() + dw / 2);
        }
        if (e.anchors.bottom) {
            if (e.anchors.top) g.setHeight(std::max(0, g.height() + dh));
            else g.moveTop(g.top() + dh);
        } else if (!e.anchors.top) {
            g.moveTop(g.top() + dh / 2);
        }
        e.item->setGeometry(g.translated(r.topLeft()));
    }
}

}  // namespace vcl
