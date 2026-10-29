#include "FormWhatsNew.h"

#include "i18n/Lang.h"

#include <QFile>
#include <QPlainTextEdit>

FormWhatsNew::FormWhatsNew(QWidget *parent) : vcl::Form(parent) {
    setWindowFlag(Qt::Window);
}

void FormWhatsNew::FormCreate(vcl::Event *) {
    setWindowIcon(QIcon(":/original/images/ICON_WHATSNEW.ICO"));
    // These are the notes of the original program (version 6.0.0.22), shown as they were.
    setWindowTitle(Lang::instance().tr("N_Help_WhatsNew") + " - Registrator Viewer 6.0.0.22");
}

// SF-WHATSNEW-002…004: the language's notes, else the English ones; the BOM is dropped.
void FormWhatsNew::FormShow(vcl::Event *) {
    auto *memo = find<QPlainTextEdit>("Memo");
    if (!memo) return;
    const QString lang = Lang::instance().current().toUpper();
    QFile f(":/original/lang/WHATSNEW_" + lang + ".TXT");
    if (lang == "EN" || !f.exists()) f.setFileName(":/original/lang/WHATSNEW.TXT");
    QString text;
    if (f.open(QIODevice::ReadOnly)) {
        text = QString::fromUtf8(f.readAll());
        if (text.startsWith(QChar(0xFEFF))) text.remove(0, 1);
    }
    memo->setReadOnly(true);
    memo->setLineWrapMode(QPlainTextEdit::NoWrap);
    memo->setPlainText("Registrator Viewer 6.0.0.22\n\n" + text);
}
