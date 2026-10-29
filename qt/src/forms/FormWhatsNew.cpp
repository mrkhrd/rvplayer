#include "FormWhatsNew.h"

#include "i18n/Lang.h"

#include <QApplication>
#include <QFile>
#include <QPlainTextEdit>

namespace {

// A text resource without its BOM; empty if missing.
QString readText(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return {};
    QString text = QString::fromUtf8(f.readAll());
    if (text.startsWith(QChar(0xFEFF))) text.remove(0, 1);
    return text;
}

}  // namespace

FormWhatsNew::FormWhatsNew(QWidget *parent) : vcl::Form(parent) {
    setWindowFlag(Qt::Window);
}

void FormWhatsNew::FormCreate(vcl::Event *) {
    setWindowIcon(QIcon(":/original/images/ICON_WHATSNEW.ICO"));
    setWindowTitle(Lang::instance().tr("N_Help_WhatsNew") + " - RV Player " + QApplication::applicationVersion());
}

// SF-WHATSNEW-002…004: the language's notes, else the English ones; the BOM is dropped. Q-30: this
// program's notes come first, then the original's (version 6.0.0.22) as they were.
void FormWhatsNew::FormShow(vcl::Event *) {
    auto *memo = find<QPlainTextEdit>("Memo");
    if (!memo) return;
    const QString lang = Lang::instance().current();
    QString ours = readText(":/app/whatsnew_" + lang + ".txt");
    if (ours.isEmpty()) ours = readText(":/app/whatsnew_en.txt");
    QString original = lang == "en" ? QString() : readText(":/original/lang/WHATSNEW_" + lang.toUpper() + ".TXT");
    if (original.isEmpty()) original = readText(":/original/lang/WHATSNEW.TXT");
    memo->setReadOnly(true);
    memo->setLineWrapMode(QPlainTextEdit::NoWrap);
    memo->setPlainText(ours.trimmed() + "\n\n\nRegistrator Viewer 6.0.0.22\n\n" + original);
}
