// Qt layout reproducing the Delphi VCL alignment rules (TWinControl.AlignControls):
// controls aligned alTop, alBottom, alLeft, alRight are stacked against the remaining client
// rectangle in that order, alClient takes what is left, and non-aligned controls keep their
// design position, moved or stretched by their anchors when the parent is resized.
#pragma once

#include <QLayout>
#include <QList>
#include <QMargins>
#include <QRect>

namespace vcl {

enum class Align { None, Top, Bottom, Left, Right, Client, Custom };

struct Anchors {
    bool left = true;
    bool top = true;
    bool right = false;
    bool bottom = false;
};

Align parseAlign(const QString &ident);
Anchors parseAnchors(const QStringList &set);

class AlignLayout : public QLayout {
    Q_OBJECT
public:
    // `designClientSize` is the parent's client size in the form file; anchors keep the
    // distances to the parent's edges measured against it.
    explicit AlignLayout(QWidget *parent, QSize designClientSize);
    ~AlignLayout() override;

    void addControl(QWidget *w, Align align, Anchors anchors, QRect designRect);

    // Insets applied to the area available for aligned controls (bevels, border width).
    void setAlignMargins(QMargins m);

    // Current size used for an aligned control (its Width for alLeft/alRight, Height for
    // alTop/alBottom). Splitters and code change it, like setting Width/Height in VCL.
    QRect controlRect(const QWidget *w) const;
    void setControlSize(QWidget *w, QSize size);
    void setControlRect(QWidget *w, QRect r);
    Align alignOf(const QWidget *w) const;
    void setAlign(QWidget *w, Align align);

    // Area that remained for alClient after the last layout pass.
    QRect clientArea() const { return lastClient; }

    void addItem(QLayoutItem *item) override;
    QLayoutItem *itemAt(int index) const override;
    QLayoutItem *takeAt(int index) override;
    int count() const override;
    QSize sizeHint() const override;
    QSize minimumSize() const override;
    void setGeometry(const QRect &r) override;

private:
    struct Entry {
        QLayoutItem *item;
        Align align;
        Anchors anchors;
        QRect rect;   // requested geometry (design or set at run time)
        int order;    // creation order, tie-breaker within an alignment
    };
    Entry *find(const QWidget *w);
    const Entry *find(const QWidget *w) const;

    QList<Entry> entries;
    QSize designClient;
    QMargins alignMargins;
    QRect lastClient;
    int nextOrder = 0;
};

}  // namespace vcl
