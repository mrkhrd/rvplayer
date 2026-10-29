// Qt counterparts of VCL controls that have no direct Qt equivalent, plus the event record
// passed to form handlers.
#pragma once

#include <QFrame>
#include <QMetaType>
#include <QPixmap>
#include <QPoint>
#include <QPointer>

class QMouseEvent;

namespace vcl {

class AlignLayout;

// Arguments of a VCL event handler. Handlers receive it as `vcl::Event *` and may set `handled`.
struct Event {
    QObject *sender = nullptr;
    QString name;                       // OnClick, OnMouseDown, ...
    QPoint pos;                         // mouse position in sender coordinates
    Qt::MouseButton button = Qt::NoButton;
    Qt::KeyboardModifiers modifiers;
    int wheelDelta = 0;                 // OnMouseWheel: +-120 per notch
    int key = 0;                        // OnKeyDown/Up: Qt key code
    bool handled = false;
};

// Decodes a Delphi TShortCut (low byte virtual key, $2000 Shift, $4000 Ctrl, $8000 Alt).
QKeySequence shortcutFromDelphi(int code);

// VCL bitmap with the TBitmap.Transparent convention: the bottom-left pixel's colour is clear.
QPixmap transparentBitmap(const QImage &image);

// TPanel: a container with optional raised/lowered bevel and its own alignment layout.
class Panel : public QFrame {
    Q_OBJECT
public:
    enum class Bevel { None, Raised, Lowered };
    explicit Panel(QWidget *parent = nullptr);
    void setBevels(Bevel outer, Bevel inner, int bevelWidth, int borderWidth);
    AlignLayout *alignLayout() const { return layout_; }
    void setAlignLayout(AlignLayout *l) { layout_ = l; }

protected:
    void paintEvent(QPaintEvent *e) override;
    // VCL delivers mouse input only to the control under the cursor; accepting it here keeps Qt
    // from passing it on to the parents (which may have the same handlers).
    void mousePressEvent(QMouseEvent *e) override;
    void mouseReleaseEvent(QMouseEvent *e) override;
    void mouseDoubleClickEvent(QMouseEvent *e) override;
    void mouseMoveEvent(QMouseEvent *e) override;

private:
    Bevel outer = Bevel::Raised;
    Bevel inner = Bevel::None;
    int bevelWidth = 1;
    AlignLayout *layout_ = nullptr;
};

// TSplitter: dragging resizes the control aligned on the same side just before it.
class Splitter : public QWidget {
    Q_OBJECT
public:
    explicit Splitter(QWidget *parent = nullptr);
    void setMinSize(int px) { minSize = px; }

signals:
    void moved();

protected:
    void mousePressEvent(QMouseEvent *e) override;
    void mouseMoveEvent(QMouseEvent *e) override;
    void mouseReleaseEvent(QMouseEvent *e) override;

private:
    QWidget *target() const;
    QPointer<QWidget> dragTarget;
    QPoint pressGlobal;
    int startSize = 0;
    int minSize = 30;
};

// TImage: shows a picture; mouse events go through the event filter like any control.
class Image : public QWidget {
    Q_OBJECT
public:
    explicit Image(QWidget *parent = nullptr);
    void setPixmap(const QPixmap &p);
    void setImage(const QImage &img) { setPixmap(QPixmap::fromImage(img)); }
    const QPixmap &pixmap() const { return pix; }
    void setStretch(bool s) { stretch = s; update(); }
    void setCenter(bool c) { center = c; update(); }

protected:
    void paintEvent(QPaintEvent *e) override;
    void mousePressEvent(QMouseEvent *e) override;
    void mouseReleaseEvent(QMouseEvent *e) override;
    void mouseDoubleClickEvent(QMouseEvent *e) override;
    void mouseMoveEvent(QMouseEvent *e) override;

private:
    QPixmap pix;
    bool stretch = false;
    bool center = false;
};

}  // namespace vcl

Q_DECLARE_METATYPE(vcl::Event *)
