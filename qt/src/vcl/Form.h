// A window built at run time from an original Delphi form file, the way VCL streams forms:
// components are created from the .dfm, laid out with VCL alignment rules, and each
// "OnXxx = Handler" entry is connected to the Q_INVOKABLE method `Handler(vcl::Event *)` of the
// concrete form class. Handlers that do not exist yet are listed by unimplementedHandlers().
#pragma once

#include "AlignLayout.h"
#include "Dfm.h"
#include "Widgets.h"

#include <QHash>
#include <QPointer>
#include <QWidget>

class QAction;
class QMenu;
class QMenuBar;
class QTimer;

namespace vcl {

class AlignLayout;

class Form : public QWidget {
    Q_OBJECT
public:
    explicit Form(QWidget *parent = nullptr);

    // Builds the form from a text .dfm resource and runs OnCreate. Returns false on parse errors.
    bool load(const QString &dfmPath);
    QString loadError() const { return error; }

    QObject *component(const QString &name) const;
    template <class T>
    T *find(const QString &name) const { return qobject_cast<T *>(component(name)); }
    QAction *action(const QString &name) const;
    QMenu *menu(const QString &name) const;        // popup menu or submenu of an item
    QMenuBar *mainMenu() const { return menuBar; }
    QTimer *timer(const QString &name) const;
    const DfmObject *definition(const QString &name) const;
    AlignLayout *layoutOf(const QWidget *container) const;
    QStringList unimplementedHandlers() const { return missing; }

    // VCL TMenuItem.Checked semantics: never toggled by a click itself; setting a RadioItem
    // unchecks the RadioItems with the same GroupIndex under the same parent.
    bool isChecked(const QAction *a) const;
    void setChecked(QAction *a, bool checked);
    bool isChecked(const QString &actionName) const;
    void setChecked(const QString &actionName, bool checked);

    // Translates menu captions by item name (spec TR-022).
    void translateMenus();

    // Spec GM-002/GM-003: a click checks a radio item or toggles a plain one, then persists it
    // (a radio item saves its whole group); the value is deleted when equal to the form default.
    void menuItemClick(QAction *a);
    void menuItemSet(QAction *a, bool checked);
    // Applies saved check states over the form-file defaults (start-up).
    void restoreMenuChecks();

    // Moves a control to another container with a new alignment (VCL: Parent := ...; Align := ...).
    void reparent(QWidget *w, QWidget *newParent, Align align);

    // Size of an aligned control as VCL Width/Height (what splitters and code change).
    QSize controlSize(QWidget *w) const;
    void setControlSize(QWidget *w, QSize s);

    static QPixmap resourceBitmap(const QString &fileName);  // ":/original/images/<fileName>"
    // Pixel art for the screen: scaled by the whole device pixel ratio without smoothing.
    static QPixmap crispPixmap(const QPixmap &pix);

    // Run-time equivalents of "OnXxx = Handler" and "PopupMenu = Name" for controls the code
    // creates itself (VCL: assigning the event property after Create).
    void bindHandler(QObject *target, const QString &event, const QString &handler);
    void attachPopup(QWidget *w, const QString &menuName);
    // A menu item created at run time (AddLanguages, AddTimeZones), registered like a form item
    // with `checked` as its default; a radio item joins the GroupIndex 0 items of `into`.
    QAction *addMenuItem(QMenu *into, const QString &name, const QString &caption, bool radio, bool checked,
                         const QString &onClick);

protected:
    // Override to create a custom widget for a component; return nullptr for the default.
    virtual QWidget *createControl(const DfmObject &def, QWidget *parent);

    // Calls the handler connected to `sender`'s `eventName`; returns false when none is set.
    bool fire(QObject *sender, const QString &eventName, Event *e = nullptr);

    void closeEvent(QCloseEvent *e) override;
    void resizeEvent(QResizeEvent *e) override;
    void showEvent(QShowEvent *e) override;
    void wheelEvent(QWheelEvent *e) override;
    void mouseDoubleClickEvent(QMouseEvent *e) override;
    bool eventFilter(QObject *watched, QEvent *ev) override;

private:
    void build(const DfmObject &def, QWidget *parent);
    QWidget *createDefault(const DfmObject &def, QWidget *parent);
    void place(QWidget *w, const DfmObject &def, QWidget *parent);
    void applyCommon(QWidget *w, const DfmObject &def);
    void makeContainer(QWidget *w, const DfmObject &def, QMargins insets = {});
    void buildMenuItems(const DfmObject &def, QMenu *into, QWidget *owner);
    void bindHandlers(QObject *target, const DfmObject &def);
    void connectEvent(QObject *target, const QString &event);
    void attachPopupMenus();
    void syncChecks();

    struct MenuItemInfo {
        QAction *action = nullptr;
        QObject *parentMenu = nullptr;
        int groupIndex = 0;
        bool radio = false;
        bool checked = false;
        bool designChecked = false;  // form-file default (ReadDafaultsMenu)
    };
    void persistMenuItem(const MenuItemInfo &info);

    DfmObject root;
    QString error;
    QHash<QString, QPointer<QObject>> components;
    QHash<QString, DfmObject> definitions;
    QHash<const QObject *, QHash<QString, QString>> handlers;  // object → event → handler
    QHash<const QAction *, MenuItemInfo> menuItems;
    QList<QPair<QWidget *, QString>> pendingPopups;            // control, popup menu name
    QMenuBar *menuBar = nullptr;
    QStringList missing;
    bool keyPreview = false;
    bool created = false;
};

}  // namespace vcl
