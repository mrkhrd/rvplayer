#include "Form.h"

#include "AlignLayout.h"
#include "core/Settings.h"
#include "i18n/Lang.h"

#include <QAbstractScrollArea>
#include <QAction>
#include <QActionGroup>
#include <QApplication>
#include <QButtonGroup>
#include <QCheckBox>
#include <QCloseEvent>
#include <QComboBox>
#include <QFile>
#include <QFileSystemModel>
#include <QGroupBox>
#include <QHeaderView>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QMenuBar>
#include <QMetaMethod>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QRegularExpression>
#include <QPushButton>
#include <QRadioButton>
#include <QScrollBar>
#include <QSignalBlocker>
#include <QSlider>
#include <QTableWidget>
#include <QTimer>
#include <QToolButton>
#include <QTreeView>
#include <QTreeWidget>

#include <cmath>
#include <functional>

namespace vcl {

namespace {

// Delphi TColor: 0x00BBGGRR, or a system colour identifier.
QColor delphiColor(const DfmValue &v, const QPalette &pal) {
    if (v.kind == DfmValue::Kind::Int) {
        const quint32 c = quint32(v.toInt());
        if ((c & 0xFF000000) == 0) return QColor(c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF);
        return {};
    }
    const QString s = v.toString().toLower();
    if (s == "clbtnface") return pal.color(QPalette::Button);
    if (s == "clwindow") return pal.color(QPalette::Base);
    if (s == "clwindowtext") return pal.color(QPalette::Text);
    if (s == "clblack") return Qt::black;
    if (s == "clwhite") return Qt::white;
    if (s == "clgray") return Qt::gray;
    if (s == "clsilver") return QColor(192, 192, 192);
    if (s == "clred") return Qt::red;
    if (s == "clblue") return Qt::blue;
    if (s == "clnavy") return QColor(0, 0, 128);
    if (s == "clyellow") return Qt::yellow;
    if (s == "clinfobk") return pal.color(QPalette::ToolTipBase);
    return {};
}

QString fontFamily(const QString &delphiName) {
    if (delphiName.compare("MS Sans Serif", Qt::CaseInsensitive) == 0) return QStringLiteral("Microsoft Sans Serif");
    return delphiName;
}

bool isContainer(const QString &cls) {
    return cls == "TPanel" || cls == "TGroupBox" || cls == "TScrollBox";
}

bool isNonVisual(const QString &cls) {
    static const QStringList list = {"TOpenDialog", "TSaveDialog", "TImageList", "TXPManifest", "TFilterGraph",
                                     "TFilter", "TApplicationEvents", "TColorDialog", "TFontDialog"};
    return list.contains(cls);
}

}  // namespace

Form::Form(QWidget *parent) : QWidget(parent) {}

QPixmap Form::resourceBitmap(const QString &fileName) {
    return crispPixmap(transparentBitmap(QImage(":/original/images/" + fileName)));
}

QPixmap Form::crispPixmap(const QPixmap &pix) {
    // Scaled by a whole factor the pixels stay sharp; a smooth scale would blur them.
    const int k = int(std::lround(qApp->devicePixelRatio()));
    if (k < 2 || pix.isNull()) return pix;
    QPixmap out = pix.scaled(pix.size() * k, Qt::IgnoreAspectRatio, Qt::FastTransformation);
    out.setDevicePixelRatio(k);
    return out;
}

bool Form::load(const QString &dfmPath) {
    QFile f(dfmPath);
    if (!f.open(QIODevice::ReadOnly)) {
        error = "cannot open " + dfmPath;
        return false;
    }
    root = parseDfm(QString::fromUtf8(f.readAll()), &error);
    if (root.className.isEmpty()) return false;

    setObjectName(root.name);
    components.insert(root.name, this);
    definitions.insert(root.name, root);
    keyPreview = root.boolProp("KeyPreview");
    applyCommon(this, root);

    const bool hasClient = root.has("ClientWidth");
    const bool hasMenu = root.has("Menu");
    const QSize client = hasClient ? QSize(int(root.intProp("ClientWidth")), int(root.intProp("ClientHeight")))
                                   : QSize(int(root.intProp("Width")) - 16, int(root.intProp("Height")) - 39 - (hasMenu ? 20 : 0));
    auto *layout = new AlignLayout(this, client);
    Q_UNUSED(layout);

    for (const DfmObject &child : root.children) build(child, this);
    if (menuBar) {
        menuBar->setFont(QApplication::font("QMenuBar"));  // menus use the system menu font
        this->layout()->setMenuBar(menuBar);
    }
    attachPopupMenus();
    bindHandlers(this, root);
    resize(client.width(), client.height() + (menuBar ? menuBar->sizeHint().height() : 0));
    if (keyPreview) qApp->installEventFilter(this);

    created = true;
    fire(this, "OnCreate");
    return true;
}

QObject *Form::component(const QString &name) const {
    return components.value(name);
}

QAction *Form::action(const QString &name) const {
    return qobject_cast<QAction *>(components.value(name));
}

QMenu *Form::menu(const QString &name) const {
    if (auto *m = qobject_cast<QMenu *>(components.value(name))) return m;
    if (auto *a = action(name)) return a->menu();
    return nullptr;
}

QTimer *Form::timer(const QString &name) const {
    return qobject_cast<QTimer *>(components.value(name));
}

const DfmObject *Form::definition(const QString &name) const {
    auto it = definitions.constFind(name);
    return it == definitions.constEnd() ? nullptr : &it.value();
}

AlignLayout *Form::layoutOf(const QWidget *container) const {
    return container ? qobject_cast<AlignLayout *>(container->layout()) : nullptr;
}

QSize Form::controlSize(QWidget *w) const {
    AlignLayout *l = w ? layoutOf(w->parentWidget()) : nullptr;
    return l ? l->controlRect(w).size() : (w ? w->size() : QSize());
}

void Form::setControlSize(QWidget *w, QSize s) {
    if (AlignLayout *l = w ? layoutOf(w->parentWidget()) : nullptr) l->setControlSize(w, s);
    else if (w) w->resize(s);
}

QWidget *Form::createControl(const DfmObject &, QWidget *) {
    return nullptr;
}

void Form::build(const DfmObject &def, QWidget *parent) {
    definitions.insert(def.name, def);
    const QString &cls = def.className;

    if (cls == "TMainMenu") {
        menuBar = new QMenuBar(this);
        menuBar->setObjectName(def.name);
        components.insert(def.name, menuBar);
        for (const DfmObject &item : def.children) {
            QMenu *m = menuBar->addMenu(item.stringProp("Caption"));
            QAction *a = m->menuAction();
            a->setObjectName(item.name);
            components.insert(item.name, a);
            definitions.insert(item.name, item);
            menuItems.insert(a, {a, menuBar, 0, false, false});
            buildMenuItems(item, m, this);
        }
        return;
    }
    if (cls == "TPopupMenu") {
        auto *m = new QMenu(this);
        m->setObjectName(def.name);
        components.insert(def.name, m);
        buildMenuItems(def, m, this);
        bindHandlers(m, def);
        return;
    }
    if (cls == "TTimer") {
        auto *t = new QTimer(this);
        t->setObjectName(def.name);
        t->setInterval(int(def.intProp("Interval", 1000)));
        components.insert(def.name, t);
        bindHandlers(t, def);
        if (def.boolProp("Enabled", true)) {
            // VCL timers start with the form; defer so OnCreate runs first.
            QTimer::singleShot(0, t, [t] { t->start(); });
        }
        return;
    }
    if (isNonVisual(cls)) return;

    QWidget *w = createControl(def, parent);
    if (!w) w = createDefault(def, parent);
    if (!w) {
        qWarning("vcl: no widget for %s: %s", qPrintable(def.name), qPrintable(cls));
        return;
    }
    w->setObjectName(def.name);
    components.insert(def.name, w);
    place(w, def, parent);
    applyCommon(w, def);
    if (def.has("PopupMenu")) pendingPopups.append({w, def.stringProp("PopupMenu")});
    for (const DfmObject &child : def.children) build(child, w);
    bindHandlers(w, def);
}

void Form::makeContainer(QWidget *w, const DfmObject &def, QMargins insets) {
    auto *l = new AlignLayout(w, QSize(int(def.intProp("Width")), int(def.intProp("Height"))));
    l->setAlignMargins(insets);
}

QWidget *Form::createDefault(const DfmObject &def, QWidget *parent) {
    const QString &cls = def.className;
    const QString caption = def.stringProp("Caption");

    if (cls == "TPanel") {
        auto *p = new Panel(parent);
        makeContainer(p, def);
        p->setAlignLayout(layoutOf(p));
        auto bevel = [](const QString &s, Panel::Bevel dflt) {
            if (s == "bvNone") return Panel::Bevel::None;
            if (s == "bvLowered") return Panel::Bevel::Lowered;
            if (s == "bvRaised") return Panel::Bevel::Raised;
            return dflt;
        };
        p->setBevels(bevel(def.stringProp("BevelOuter"), Panel::Bevel::Raised), bevel(def.stringProp("BevelInner"), Panel::Bevel::None),
                     int(def.intProp("BevelWidth", 1)), int(def.intProp("BorderWidth", 0)));
        if (!caption.isEmpty()) {
            auto *label = new QLabel(caption, p);
            label->setAlignment(Qt::AlignCenter);
            label->setGeometry(0, 0, int(def.intProp("Width")), int(def.intProp("Height")));
            label->lower();
        }
        return p;
    }
    if (cls == "TGroupBox") {
        auto *g = new QGroupBox(caption, parent);
        makeContainer(g, def, QMargins(2, 16, 2, 2));
        return g;
    }
    if (cls == "TScrollBox") {
        auto *p = new Panel(parent);
        p->setBevels(Panel::Bevel::None, Panel::Bevel::None, 1, 0);
        makeContainer(p, def);
        p->setAlignLayout(layoutOf(p));
        return p;
    }
    if (cls == "TSplitter") {
        auto *s = new Splitter(parent);
        s->setMinSize(int(def.intProp("MinSize", 30)));
        const Align a = parseAlign(def.stringProp("Align", "alLeft"));
        s->setCursor((a == Align::Top || a == Align::Bottom) ? Qt::SizeVerCursor : Qt::SizeHorCursor);
        connect(s, &Splitter::moved, this, [this, s] { fire(s, "OnMoved"); });
        return s;
    }
    if (cls == "TLabel" || cls == "TStaticText") {
        auto *l = new QLabel(caption, parent);
        Qt::Alignment h = Qt::AlignLeft;
        const QString al = def.stringProp("Alignment");
        if (al == "taRightJustify") h = Qt::AlignRight;
        if (al == "taCenter") h = Qt::AlignHCenter;
        const QString lay = def.stringProp("Layout");
        const Qt::Alignment v = lay == "tlCenter" ? Qt::AlignVCenter : lay == "tlBottom" ? Qt::AlignBottom : Qt::AlignTop;
        l->setAlignment(h | v);
        l->setWordWrap(def.boolProp("WordWrap"));
        if (cls == "TStaticText" && def.stringProp("BorderStyle") == "sbsSunken") l->setFrameStyle(QFrame::Panel | QFrame::Sunken);
        return l;
    }
    if (cls == "TButton") {
        auto *b = new QPushButton(caption, parent);
        b->setDefault(def.boolProp("Default"));
        b->setAutoDefault(false);
        return b;
    }
    if (cls == "TSpeedButton") {
        auto *b = new QToolButton(parent);
        b->setText(caption);
        b->setAutoRaise(def.boolProp("Flat"));
        b->setToolButtonStyle(Qt::ToolButtonTextOnly);
        const int group = int(def.intProp("GroupIndex"));
        if (group > 0) {
            b->setCheckable(true);
            b->setChecked(def.boolProp("Down"));
            if (!def.boolProp("AllowAllUp")) {
                const QString key = QStringLiteral("vclSpeedGroup%1").arg(group);
                auto *bg = parent->findChild<QButtonGroup *>(key, Qt::FindDirectChildrenOnly);
                if (!bg) {
                    bg = new QButtonGroup(parent);
                    bg->setObjectName(key);
                    bg->setExclusive(true);
                }
                bg->addButton(b);
            }
        }
        return b;
    }
    if (cls == "TCheckBox") {
        auto *c = new QCheckBox(caption, parent);
        c->setChecked(def.boolProp("Checked") || def.stringProp("State") == "cbChecked");
        return c;
    }
    if (cls == "TRadioButton") {
        auto *r = new QRadioButton(caption, parent);
        r->setChecked(def.boolProp("Checked"));
        return r;
    }
    if (cls == "TEdit") {
        auto *e = new QLineEdit(def.stringProp("Text"), parent);
        e->setReadOnly(def.boolProp("ReadOnly"));
        return e;
    }
    if (cls == "TMemo") {
        auto *m = new QPlainTextEdit(parent);
        if (const DfmValue *lines = def.prop("Lines.Strings")) m->setPlainText(lines->toStringList().join('\n'));
        m->setReadOnly(def.boolProp("ReadOnly"));
        m->setLineWrapMode(def.boolProp("WordWrap", true) ? QPlainTextEdit::WidgetWidth : QPlainTextEdit::NoWrap);
        return m;
    }
    if (cls == "TComboBox") {
        auto *c = new QComboBox(parent);
        if (const DfmValue *items = def.prop("Items.Strings")) c->addItems(items->toStringList());
        c->setEditable(def.stringProp("Style", "csDropDown") == "csDropDown");
        c->setCurrentIndex(int(def.intProp("ItemIndex", -1)));
        if (c->isEditable()) c->setEditText(def.stringProp("Text"));
        return c;
    }
    if (cls == "TTrackBar") {
        auto *s = new QSlider(def.stringProp("Orientation") == "trVertical" ? Qt::Vertical : Qt::Horizontal, parent);
        s->setRange(int(def.intProp("Min", 0)), int(def.intProp("Max", 10)));
        s->setValue(int(def.intProp("Position", 0)));
        s->setPageStep(int(def.intProp("PageSize", 2)));
        s->setTickInterval(int(def.intProp("Frequency", 1)));
        const QString ticks = def.stringProp("TickStyle", "tsAuto");
        s->setTickPosition(ticks == "tsNone" ? QSlider::NoTicks : QSlider::TicksBelow);
        return s;
    }
    if (cls == "TScrollBar") {
        auto *s = new QScrollBar(def.stringProp("Kind") == "sbVertical" ? Qt::Vertical : Qt::Horizontal, parent);
        s->setRange(int(def.intProp("Min", 0)), int(def.intProp("Max", 100)));
        s->setValue(int(def.intProp("Position", 0)));
        return s;
    }
    if (cls == "TProgressBar") {
        auto *p = new QProgressBar(parent);
        p->setRange(int(def.intProp("Min", 0)), int(def.intProp("Max", 100)));
        p->setValue(int(def.intProp("Position", 0)));
        p->setTextVisible(false);
        return p;
    }
    if (cls == "TImage") {
        auto *i = new Image(parent);
        i->setStretch(def.boolProp("Stretch"));
        i->setCenter(def.boolProp("Center"));
        return i;
    }
    if (cls == "TListView") {
        auto *t = new QTreeWidget(parent);
        t->setRootIsDecorated(false);
        t->setUniformRowHeights(true);
        t->setSelectionBehavior(QAbstractItemView::SelectRows);
        t->setSelectionMode(def.boolProp("MultiSelect") ? QAbstractItemView::ExtendedSelection : QAbstractItemView::SingleSelection);
        QStringList headers;
        if (const DfmValue *cols = def.prop("Columns")) {
            for (const DfmItem &c : cols->items) {
                QString h;
                for (const auto &p : c.props) if (p.first == "Caption") h = p.second.toString();
                headers.append(h);
            }
            t->setHeaderLabels(headers);
            for (int i = 0; i < cols->items.size(); ++i) {
                for (const auto &p : cols->items[i].props) if (p.first == "Width") t->setColumnWidth(i, int(p.second.toInt()));
            }
        }
        t->setHeaderHidden(def.stringProp("ViewStyle", "vsIcon") != "vsReport");
        connect(t, &QTreeWidget::itemSelectionChanged, this, [this, t] { fire(t, "OnSelectItem"); });
        return t;
    }
    if (cls == "TStringGrid") {
        auto *g = new QTableWidget(int(def.intProp("RowCount", 5)), int(def.intProp("ColCount", 5)), parent);
        g->verticalHeader()->hide();
        return g;
    }
    if (cls == "TCheckListBox") {
        return new QListWidget(parent);
    }
    if (cls == "TShellTreeView") {
        auto *v = new QTreeView(parent);
        auto *model = new QFileSystemModel(v);
        model->setFilter(QDir::Dirs | QDir::Drives | QDir::NoDotAndDotDot);
        model->setRootPath(QString());
        v->setModel(model);
        for (int c = 1; c < model->columnCount(); ++c) v->hideColumn(c);
        v->setHeaderHidden(true);
        return v;
    }
    if (cls == "TBevel" || cls == "TShape") {
        auto *f = new QFrame(parent);
        const QString shape = def.stringProp("Shape");
        if (shape == "bsTopLine" || shape == "bsBottomLine") f->setFrameShape(QFrame::HLine);
        else if (shape == "bsLeftLine" || shape == "bsRightLine") f->setFrameShape(QFrame::VLine);
        else f->setFrameShape(QFrame::Box);
        f->setFrameShadow(QFrame::Sunken);
        return f;
    }
    // TWebBrowser, TVideoWindow and anything unknown: an empty placeholder the form can replace.
    auto *placeholder = new QWidget(parent);
    placeholder->setProperty("vclClass", cls);
    return placeholder;
}

void Form::place(QWidget *w, const DfmObject &def, QWidget *parent) {
    const QRect r(int(def.intProp("Left")), int(def.intProp("Top")), int(def.intProp("Width", 100)), int(def.intProp("Height", 25)));
    Anchors anchors;
    if (const DfmValue *a = def.prop("Anchors")) anchors = parseAnchors(a->set);
    if (AlignLayout *l = layoutOf(parent)) {
        l->addControl(w, parseAlign(def.stringProp("Align", "alNone")), anchors, r);
    } else {
        w->setGeometry(r);
    }
}

void Form::applyCommon(QWidget *w, const DfmObject &def) {
    // A form always has a font: VCL's default is MS Sans Serif, Height -11 (8 pt).
    if (w == this || def.has("Font.Name") || def.has("Font.Height") || def.has("Font.Style")) {
        QFont f = w->font();
        if (def.has("Font.Name")) f.setFamily(fontFamily(def.stringProp("Font.Name")));
        else if (w == this) f.setFamily(fontFamily("MS Sans Serif"));
        if (const qint64 h = def.intProp("Font.Height", w == this ? -11 : 0); h != 0) f.setPixelSize(int(h < 0 ? -h : h * 3 / 4));
        if (const DfmValue *st = def.prop("Font.Style")) {
            f.setBold(st->set.contains("fsBold"));
            f.setItalic(st->set.contains("fsItalic"));
            f.setUnderline(st->set.contains("fsUnderline"));
        }
        w->setFont(f);
    }
    if (const DfmValue *c = def.prop("Font.Color")) {
        const QColor col = delphiColor(*c, w->palette());
        if (col.isValid()) {
            QPalette p = w->palette();
            p.setColor(QPalette::WindowText, col);
            p.setColor(QPalette::Text, col);
            w->setPalette(p);
        }
    }
    if (const DfmValue *c = def.prop("Color"); c && !def.boolProp("ParentColor")) {
        const QColor col = delphiColor(*c, w->palette());
        if (col.isValid()) {
            QPalette p = w->palette();
            p.setColor(QPalette::Window, col);
            p.setColor(QPalette::Base, col);
            w->setPalette(p);
            w->setAutoFillBackground(true);
        }
    }
    if (const QString hint = def.stringProp("Hint"); !hint.isEmpty()) w->setToolTip(hint);
    if (def.has("Enabled") && !def.boolProp("Enabled", true)) w->setEnabled(false);
    if (w != this && def.has("Visible") && !def.boolProp("Visible", true)) w->hide();
    if (def.stringProp("Cursor") == "crHandPoint") w->setCursor(Qt::PointingHandCursor);
}

void Form::buildMenuItems(const DfmObject &def, QMenu *into, QWidget *owner) {
    for (const DfmObject &item : def.children) {
        definitions.insert(item.name, item);
        const QString caption = item.stringProp("Caption");
        if (caption == "-") {
            QAction *sep = into->addSeparator();
            sep->setObjectName(item.name);
            components.insert(item.name, sep);
            if (item.has("Visible") && !item.boolProp("Visible", true)) sep->setVisible(false);
            continue;
        }
        QAction *a = nullptr;
        if (!item.children.isEmpty()) {
            QMenu *sub = into->addMenu(caption);
            a = sub->menuAction();
            buildMenuItems(item, sub, owner);
        } else {
            a = into->addAction(caption);
        }
        a->setObjectName(item.name);
        components.insert(item.name, a);
        if (item.has("ShortCut")) a->setShortcut(shortcutFromDelphi(int(item.intProp("ShortCut"))));
        a->setShortcutContext(Qt::WindowShortcut);
        if (item.has("Enabled")) a->setEnabled(item.boolProp("Enabled", true));
        if (item.has("Visible")) a->setVisible(item.boolProp("Visible", true));
        if (item.has("Hint")) a->setStatusTip(item.stringProp("Hint"));
        if (item.boolProp("Default")) into->setDefaultAction(a);

        const QPixmap icon = resourceBitmap(item.name.toUpper() + ".BMP");
        if (!icon.isNull()) a->setIcon(QIcon(icon));

        MenuItemInfo info{a, into, int(item.intProp("GroupIndex")), item.boolProp("RadioItem"), item.boolProp("Checked"),
                          item.boolProp("Checked")};
        if (info.radio || item.has("Checked")) {
            a->setCheckable(true);
            if (info.radio) {
                const QString key = QStringLiteral("vclRadio%1").arg(info.groupIndex);
                auto *group = into->findChild<QActionGroup *>(key, Qt::FindDirectChildrenOnly);
                if (!group) {
                    group = new QActionGroup(into);
                    group->setObjectName(key);
                    group->setExclusionPolicy(QActionGroup::ExclusionPolicy::ExclusiveOptional);
                }
                group->addAction(a);
            }
            a->setChecked(info.checked);
        }
        menuItems.insert(a, info);
        if (item.children.isEmpty()) {
            connect(a, &QAction::triggered, this, [this, a] {
                syncChecks();  // VCL items never toggle themselves (no AutoCheck in the forms)
                fire(a, "OnClick");
            });
        }
        bindHandlers(a, item);
    }
}

void Form::attachPopupMenus() {
    for (const auto &[w, name] : pendingPopups) {
        QMenu *m = qobject_cast<QMenu *>(components.value(name));
        if (!m || !w) continue;
        w->setContextMenuPolicy(Qt::CustomContextMenu);
        connect(w, &QWidget::customContextMenuRequested, this, [this, w, m](const QPoint &pos) {
            fire(m, "OnPopup");
            // Scroll areas report positions in their viewport.
            auto *area = qobject_cast<QAbstractScrollArea *>(w);
            m->exec(area ? area->viewport()->mapToGlobal(pos) : w->mapToGlobal(pos));
        });
        // Pop-up shortcuts work while the owning control has focus, as in VCL.
        for (QAction *a : m->actions()) {
            if (a->shortcut().isEmpty()) continue;
            a->setShortcutContext(Qt::WidgetWithChildrenShortcut);
            w->addAction(a);
        }
    }
    pendingPopups.clear();
}

void Form::syncChecks() {
    for (auto it = menuItems.begin(); it != menuItems.end(); ++it) {
        QAction *a = it->action;
        if (a->isCheckable() && a->isChecked() != it->checked) {
            QSignalBlocker block(a);
            a->setChecked(it->checked);
        }
    }
}

bool Form::isChecked(const QAction *a) const {
    auto it = menuItems.constFind(a);
    return it != menuItems.constEnd() && it->checked;
}

void Form::setChecked(QAction *a, bool checked) {
    auto it = menuItems.find(a);
    if (it == menuItems.end()) return;
    if (checked && it->radio) {
        for (auto &other : menuItems) {
            if (other.action != a && other.radio && other.parentMenu == it->parentMenu && other.groupIndex == it->groupIndex) {
                other.checked = false;
            }
        }
    }
    it->checked = checked;
    if (!a->isCheckable()) a->setCheckable(true);
    syncChecks();
}

void Form::persistMenuItem(const MenuItemInfo &info) {
    Settings &s = Settings::instance();
    const QString name = info.action->objectName();
    if (info.checked == info.designChecked) s.remove(name);
    else s.writeBool(name, info.checked);
}

void Form::menuItemClick(QAction *a) {
    auto it = menuItems.find(a);
    if (it == menuItems.end()) return;
    menuItemSet(a, it->radio ? true : !it->checked);
}

void Form::menuItemSet(QAction *a, bool checked) {
    auto it = menuItems.find(a);
    if (it == menuItems.end()) return;
    setChecked(a, checked);
    if (it->radio) {
        for (const MenuItemInfo &other : std::as_const(menuItems)) {
            if (other.radio && other.parentMenu == it->parentMenu && other.groupIndex == it->groupIndex) persistMenuItem(other);
        }
    } else {
        persistMenuItem(*it);
    }
}

void Form::restoreMenuChecks() {
    Settings &s = Settings::instance();
    for (auto it = menuItems.begin(); it != menuItems.end(); ++it) {
        const QString name = it->action->objectName();
        if (!it->action->isCheckable() || name.isEmpty()) continue;
        it->checked = s.readBool(name, it->designChecked);
    }
    syncChecks();
}

void Form::reparent(QWidget *w, QWidget *newParent, Align align) {
    if (!w || !newParent) return;
    const QRect r = layoutOf(w->parentWidget()) ? layoutOf(w->parentWidget())->controlRect(w) : w->geometry();
    const bool visible = !w->isHidden();
    if (AlignLayout *old = layoutOf(w->parentWidget())) old->removeWidget(w);
    w->setParent(newParent);
    if (AlignLayout *l = layoutOf(newParent)) l->addControl(w, align, Anchors{}, r);
    w->setVisible(visible);
}

bool Form::isChecked(const QString &actionName) const {
    return isChecked(action(actionName));
}

void Form::setChecked(const QString &actionName, bool checked) {
    if (QAction *a = action(actionName)) setChecked(a, checked);
}

void Form::translateMenus() {
    const Lang &lang = Lang::instance();
    std::function<void(QMenu *)> walk = [&](QMenu *m) {
        for (QAction *a : m->actions()) {
            const QString name = a->objectName();
            if (a->isSeparator() || name.isEmpty()) continue;
            static const QRegularExpression zoomPercent(QStringLiteral("^N_VideoZoom_\\d+$"));
            if (!zoomPercent.match(name).hasMatch()) {
                QString text = lang.trDef(name, QString());
                if (text.isEmpty() && !name.isEmpty() && name.back().isDigit()) text = lang.trDef(name.chopped(1), QString());
                if (text.isEmpty()) text = name;
                if (name == "N_FileList_Group_With" || name == "N_VideoZoom_Other") text += " . . .";
                a->setText(text);
            }
            if (a->menu() && name != "N_View_Language" && name != "N_View_TimeZone") walk(a->menu());
        }
    };
    if (menuBar) {
        for (QAction *top : menuBar->actions()) {
            const QString name = top->objectName();
            if (!name.isEmpty()) top->setText(lang.tr(name));
            if (top->menu()) walk(top->menu());
        }
    }
    for (auto it = definitions.cbegin(); it != definitions.cend(); ++it) {
        if (it->className != "TPopupMenu") continue;
        if (auto *m = qobject_cast<QMenu *>(components.value(it.key()))) walk(m);
    }
}

void Form::bindHandlers(QObject *target, const DfmObject &def) {
    for (const auto &[key, value] : def.props) {
        if (!key.startsWith("On") || value.kind != DfmValue::Kind::Ident) continue;
        const QString handler = value.toString();
        handlers[target].insert(key, handler);
        const QByteArray sig = QMetaObject::normalizedSignature((handler + "(vcl::Event*)").toUtf8());
        if (metaObject()->indexOfMethod(sig) < 0) {
            const QString entry = handler + "  (" + def.name + "." + key + ")";
            if (!missing.contains(entry)) missing.append(entry);
        }
        connectEvent(target, key);
    }
}

void Form::bindHandler(QObject *target, const QString &event, const QString &handler) {
    const bool had = handlers.value(target).contains(event);
    handlers[target].insert(event, handler);
    if (!had) connectEvent(target, event);
}

void Form::attachPopup(QWidget *w, const QString &menuName) {
    pendingPopups.append({w, menuName});
    attachPopupMenus();
}

QAction *Form::addMenuItem(QMenu *into, const QString &name, const QString &caption, bool radio, bool checked,
                          const QString &onClick) {
    QAction *a = into->addAction(caption);
    a->setObjectName(name);
    a->setCheckable(true);
    components.insert(name, a);
    if (radio) {
        const QString key = QStringLiteral("vclRadio0");
        auto *group = into->findChild<QActionGroup *>(key, Qt::FindDirectChildrenOnly);
        if (!group) {
            group = new QActionGroup(into);
            group->setObjectName(key);
            group->setExclusionPolicy(QActionGroup::ExclusionPolicy::ExclusiveOptional);
        }
        group->addAction(a);
    }
    a->setChecked(checked);
    menuItems.insert(a, {a, into, 0, radio, checked, checked});
    connect(a, &QAction::triggered, this, [this, a] {
        syncChecks();
        fire(a, "OnClick");
    });
    bindHandler(a, "OnClick", onClick);
    return a;
}

void Form::connectEvent(QObject *target, const QString &key) {
    if (auto *b = qobject_cast<QAbstractButton *>(target); b && key == "OnClick") {
        connect(b, &QAbstractButton::clicked, this, [this, b] { fire(b, "OnClick"); });
    } else if (auto *e = qobject_cast<QLineEdit *>(target); e && key == "OnChange") {
        connect(e, &QLineEdit::textChanged, this, [this, e] { fire(e, "OnChange"); });
    } else if (auto *s = qobject_cast<QAbstractSlider *>(target); s && key == "OnChange") {
        connect(s, &QAbstractSlider::valueChanged, this, [this, s] { fire(s, "OnChange"); });
    } else if (auto *c = qobject_cast<QComboBox *>(target); c && key == "OnChange") {
        connect(c, &QComboBox::activated, this, [this, c] { fire(c, "OnChange"); });
    } else if (auto *t = qobject_cast<QTimer *>(target); t && key == "OnTimer") {
        connect(t, &QTimer::timeout, this, [this, t] { fire(t, "OnTimer"); });
    } else if (auto *w = qobject_cast<QWidget *>(target); w && w != this) {
        w->installEventFilter(this);  // mouse, key and resize events through eventFilter
        if (key.startsWith("OnMouseMove")) w->setMouseTracking(true);
    }
}

bool Form::fire(QObject *sender, const QString &eventName, Event *e) {
    const QString handler = handlers.value(sender).value(eventName);
    if (handler.isEmpty()) return false;
    Event local;
    if (!e) e = &local;
    e->sender = sender;
    e->name = eventName;
    return QMetaObject::invokeMethod(this, handler.toUtf8().constData(), Qt::DirectConnection, Q_ARG(vcl::Event *, e));
}

bool Form::eventFilter(QObject *watched, QEvent *ev) {
    // KeyPreview: key events of any control in this window go to the form first.
    if (keyPreview && (ev->type() == QEvent::KeyPress || ev->type() == QEvent::KeyRelease)) {
        auto *w = qobject_cast<QWidget *>(watched);
        if (w && w->window() == this && handlers.value(this).contains(ev->type() == QEvent::KeyPress ? "OnKeyDown" : "OnKeyUp")) {
            auto *k = static_cast<QKeyEvent *>(ev);
            Event e;
            e.key = k->key();
            e.modifiers = k->modifiers();
            fire(this, ev->type() == QEvent::KeyPress ? "OnKeyDown" : "OnKeyUp", &e);
            if (e.handled) return true;
        }
    }
    if (!handlers.contains(watched) || watched == this) return QWidget::eventFilter(watched, ev);

    auto mouse = [&](const QString &name) {
        auto *m = static_cast<QMouseEvent *>(ev);
        Event e;
        e.pos = m->position().toPoint();
        e.button = m->button();
        e.modifiers = m->modifiers();
        fire(watched, name, &e);
        return e.handled;
    };
    switch (ev->type()) {
    case QEvent::MouseButtonPress:
        watched->setProperty("vclPressed", true);
        return mouse("OnMouseDown");
    case QEvent::MouseButtonRelease: {
        const bool wasPressed = watched->property("vclPressed").toBool();
        watched->setProperty("vclPressed", false);
        const bool h = mouse("OnMouseUp");
        auto *w = qobject_cast<QWidget *>(watched);
        auto *m = static_cast<QMouseEvent *>(ev);
        if (wasPressed && w && m->button() == Qt::LeftButton && w->rect().contains(m->position().toPoint()) &&
            !qobject_cast<QAbstractButton *>(watched)) {
            fire(watched, "OnClick");
        }
        return h;
    }
    case QEvent::MouseMove:
        return mouse("OnMouseMove");
    case QEvent::MouseButtonDblClick:
        return mouse("OnDblClick");
    case QEvent::Enter:
        fire(watched, "OnMouseEnter");
        break;
    case QEvent::Leave:
        fire(watched, "OnMouseLeave");
        break;
    case QEvent::Resize:
        fire(watched, "OnResize");
        break;
    case QEvent::Wheel: {
        auto *wh = static_cast<QWheelEvent *>(ev);
        Event e;
        e.pos = wh->position().toPoint();
        e.wheelDelta = wh->angleDelta().y();
        e.modifiers = wh->modifiers();
        if (fire(watched, "OnMouseWheel", &e) && e.handled) return true;
        break;
    }
    case QEvent::KeyPress:
    case QEvent::KeyRelease: {
        auto *k = static_cast<QKeyEvent *>(ev);
        Event e;
        e.key = k->key();
        e.modifiers = k->modifiers();
        fire(watched, ev->type() == QEvent::KeyPress ? "OnKeyDown" : "OnKeyUp", &e);
        if (e.handled) return true;
        break;
    }
    default:
        break;
    }
    return QWidget::eventFilter(watched, ev);
}

void Form::closeEvent(QCloseEvent *e) {
    Event ev;
    fire(this, "OnCloseQuery", &ev);
    if (ev.handled) {  // handler refused: CanClose := False
        e->ignore();
        return;
    }
    fire(this, "OnClose");
    QWidget::closeEvent(e);
}

void Form::resizeEvent(QResizeEvent *e) {
    QWidget::resizeEvent(e);
    if (created) fire(this, "OnResize");
}

void Form::showEvent(QShowEvent *e) {
    QWidget::showEvent(e);
    fire(this, "OnShow");
}

void Form::wheelEvent(QWheelEvent *e) {
    Event ev;
    ev.pos = e->position().toPoint();
    ev.wheelDelta = e->angleDelta().y();
    ev.modifiers = e->modifiers();
    if (!fire(this, "OnMouseWheel", &ev) || !ev.handled) QWidget::wheelEvent(e);
}

void Form::mouseDoubleClickEvent(QMouseEvent *e) {
    Event ev;
    ev.pos = e->position().toPoint();
    ev.button = e->button();
    if (!fire(this, "OnDblClick", &ev)) QWidget::mouseDoubleClickEvent(e);
}

}  // namespace vcl
