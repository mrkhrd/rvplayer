#include "FormAbout.h"

#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"
#include "vcl/Widgets.h"

#include <QApplication>
#include <QDesktopServices>
#include <QLabel>
#include <QUrl>

namespace {

const char *const kProjectUrl = "https://github.com/mrkhrd/rvplayer";
const QColor kLinkColour(0, 102, 204);

}  // namespace

FormAbout::FormAbout(QWidget *parent) : vcl::Form(parent) {
    setWindowFlags(Qt::Dialog | Qt::WindowCloseButtonHint | Qt::MSWindowsFixedSizeDialogHint);
}

// SF-ABOUT-001…003, SF-ABOUT-008; Q-29: this program above, the original and its author below.
void FormAbout::FormCreate(vcl::Event *) {
    const Lang &lang = Lang::instance();
    const bool ru = lang.current() == "ru";
    setWindowIcon(QIcon(":/original/images/ICON_ABOUT.ICO"));
    setWindowTitle(lang.tr("N_Help_About"));
    QPalette pal = palette();
    pal.setColor(QPalette::Window, lang.color("colorAboutBG"));
    setPalette(pal);
    setAutoFillBackground(true);
    // The application's icon where the DATAKAM logo was.
    if (auto *logo = find<vcl::Image>("ImageLogo")) {
        logo->setStretch(true);
        logo->setPixmap(QIcon(":/app/rvplayer.ico").pixmap(QSize(80, 80), devicePixelRatioF()));
    }
    if (auto *title = find<QLabel>("LabelTitle")) {
        QFont f = title->font();
        f.setFamily("Arial");
        title->setFont(f);
    }
    auto set = [&](const char *name, const QString &text) {
        if (auto *l = find<QLabel>(name)) {
            l->setText(text);
            l->setVisible(!text.isEmpty());
        }
    };
    // The project page on GitHub, as a link (blue, underlined, a hand over it).
    set("LabelSite", QString::fromLatin1(kProjectUrl).mid(8));  // without "https://"
    if (auto *site = find<QLabel>("LabelSite")) {
        site->setToolTip(QString::fromLatin1(kProjectUrl));
        QFont f = site->font();
        f.setUnderline(true);
        site->setFont(f);
        QPalette p = site->palette();
        p.setColor(QPalette::WindowText, kLinkColour);
        site->setPalette(p);
    }
    // The original program and its author (the translation files' own entries).
    if (vcl::AlignLayout *l = layoutOf(this)) {
        auto *original = new QLabel(ru ? QStringLiteral("На основе Registrator Viewer") : QStringLiteral("Based on Registrator Viewer"), this);
        original->setObjectName("LabelOriginal");
        l->addControl(original, vcl::Align::None, {}, QRect(0, 0, 10, 13));
    }
    set("LabelName", (ru ? QStringLiteral("автор: ") : QStringLiteral("by ")) + lang.trDef("MyName", QString()));
    set("LabelCity", lang.trDef("MyCity", QString()));
    set("LabelMail", QString());  // the original's contact no longer exists
    if (QWidget *mail = find<QWidget>("LabelMail")) mail->setToolTip(lang.tr("SendMail"));
}

// SF-ABOUT-004: everything centred, one row under the other.
void FormAbout::FormShow(vcl::Event *) {
    if (auto *t = find<QLabel>("LabelTitle")) t->setText(QStringLiteral("RV Player"));
    if (auto *v = find<QLabel>("LabelVer")) v->setText("v" + QApplication::applicationVersion());
    vcl::AlignLayout *l = layoutOf(this);
    if (!l) return;
    const int width = 300;
    // Rows with the space above each.
    const std::pair<const char *, int> rows[] = {{"ImageLogo", 4},     {"LabelTitle", 6}, {"LabelVer", 0}, {"LabelSite", 6},
                                                 {"LabelOriginal", 20}, {"LabelName", 3},  {"LabelCity", 3}};
    int y = 0;
    for (const auto &[name, gap] : rows) {
        QWidget *w = findChild<QWidget *>(name);  // LabelOriginal is not one of the form's own
        if (!w || w->isHidden()) continue;
        const QSize size = qobject_cast<vcl::Image *>(w) ? QSize(80, 80) : w->sizeHint();
        y += gap;
        l->setControlRect(w, QRect((width - size.width()) / 2, y, size.width(), size.height()));
        y += size.height();
    }
    // AutoSize with BorderWidth 10: the window ends 10 px around the rows.
    setFixedSize(width + 20, y + 4 + 20);
}

// SF-ABOUT-006/007.
void FormAbout::LabelMailClick(vcl::Event *) {
    if (auto *m = find<QLabel>("LabelMail"); m && !m->text().isEmpty()) QDesktopServices::openUrl(QUrl("mailto:" + m->text()));
}

void FormAbout::LabelSiteClick(vcl::Event *) {
    QDesktopServices::openUrl(QUrl(QString::fromLatin1(kProjectUrl)));
}

// SF-ABOUT-005: blue while hovered; the project link stays a link.
void FormAbout::LabelMailMouseEnter(vcl::Event *e) {
    if (auto *l = e ? qobject_cast<QLabel *>(e->sender) : nullptr) {
        QPalette p = l->palette();
        p.setColor(QPalette::WindowText, Qt::blue);
        l->setPalette(p);
    }
}

void FormAbout::LabelMailMouseLeave(vcl::Event *e) {
    if (auto *l = e ? qobject_cast<QLabel *>(e->sender) : nullptr) {
        QPalette p = l->palette();
        p.setColor(QPalette::WindowText, l->objectName() == "LabelSite" ? kLinkColour : QColor(Qt::black));
        l->setPalette(p);
    }
}
