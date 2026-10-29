#include "FormAbout.h"

#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"

#include <QApplication>
#include <QDesktopServices>
#include <QLabel>
#include <QUrl>

FormAbout::FormAbout(QWidget *parent) : vcl::Form(parent) {
    setWindowFlags(Qt::Dialog | Qt::WindowCloseButtonHint | Qt::MSWindowsFixedSizeDialogHint);
}

// SF-ABOUT-001…003, SF-ABOUT-008.
void FormAbout::FormCreate(vcl::Event *) {
    const Lang &lang = Lang::instance();
    setWindowIcon(QIcon(":/original/images/ICON_ABOUT.ICO"));
    setWindowTitle(lang.tr("N_Help_About"));
    QPalette pal = palette();
    pal.setColor(QPalette::Window, lang.color("colorAboutBG"));
    setPalette(pal);
    setAutoFillBackground(true);
    if (auto *title = find<QLabel>("LabelTitle")) {
        QFont f = title->font();
        f.setFamily("Arial");
        title->setFont(f);
    }
    if (QWidget *logo = find<QWidget>("ImageLogo")) logo->hide();  // the DATAKAM logo
    auto set = [&](const char *name, const QString &text) {
        if (auto *l = find<QLabel>(name)) {
            l->setText(text);
            l->setVisible(!text.isEmpty());
        }
    };
    set("LabelName", lang.trDef("MyName", QString()));
    set("LabelCity", lang.trDef("MyCity", QString()));
    set("LabelMail", lang.trDef("MyEMail", QString()));
    set("LabelSite", lang.trDef("MySite", QString()));
    if (QWidget *mail = find<QWidget>("LabelMail")) mail->setToolTip(lang.tr("SendMail"));
    if (QWidget *site = find<QWidget>("LabelSite")) site->setToolTip(lang.tr("N_Help_Site"));
    // SF-ABOUT-002: city above the mail, the name below it.
    vcl::AlignLayout *l = layoutOf(this);
    QWidget *name = find<QWidget>("LabelName");
    QWidget *city = find<QWidget>("LabelCity");
    if (l && name && city) {
        QRect rn = l->controlRect(name), rc = l->controlRect(city);
        const int top = rn.top();
        rn.moveTop(rc.top());
        rc.moveTop(top);
        l->setControlRect(name, rn);
        l->setControlRect(city, rc);
    }
}

// SF-ABOUT-004: everything centred horizontally.
void FormAbout::FormShow(vcl::Event *) {
    if (auto *t = find<QLabel>("LabelTitle")) t->setText(QStringLiteral("Registrator Viewer"));
    if (auto *v = find<QLabel>("LabelVer")) v->setText("v" + QApplication::applicationVersion());
    vcl::AlignLayout *l = layoutOf(this);
    if (!l) return;
    const int width = 300;
    // Without the logo the text starts at the top instead of under it.
    const int lift = find<QWidget>("LabelTitle") ? l->controlRect(find<QWidget>("LabelTitle")).top() : 0;
    for (const char *n : {"LabelTitle", "LabelVer", "LabelName", "LabelMail", "LabelCity", "LabelSite"}) {
        if (auto *w = find<QLabel>(n)) {
            QRect r = l->controlRect(w);
            r.setWidth(w->sizeHint().width());
            r.moveLeft((width - r.width()) / 2);
            if (!aboutLifted) r.translate(0, -lift);
            l->setControlRect(w, r);
        }
    }
    aboutLifted = true;
    // AutoSize with BorderWidth 10: the window ends 10 px around the labels.
    int bottom = 0;
    for (const char *n : {"LabelTitle", "LabelVer", "LabelName", "LabelMail", "LabelCity", "LabelSite"}) {
        if (auto *w = find<QLabel>(n); w && !w->isHidden()) bottom = std::max(bottom, l->controlRect(w).bottom() + 1);
    }
    setFixedSize(width + 20, bottom + 20);
}

// SF-ABOUT-006/007.
void FormAbout::LabelMailClick(vcl::Event *) {
    if (auto *m = find<QLabel>("LabelMail"); m && !m->text().isEmpty()) QDesktopServices::openUrl(QUrl("mailto:" + m->text()));
}

void FormAbout::LabelSiteClick(vcl::Event *) {
    if (auto *s = find<QLabel>("LabelSite"); s && !s->text().isEmpty()) QDesktopServices::openUrl(QUrl(s->text()));
}

// SF-ABOUT-005: blue while hovered.
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
        p.setColor(QPalette::WindowText, Qt::black);
        l->setPalette(p);
    }
}
