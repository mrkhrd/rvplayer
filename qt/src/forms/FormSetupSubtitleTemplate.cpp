#include "FormSetupSubtitleTemplate.h"

#include "core/Format.h"
#include "core/Settings.h"
#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"

#include <QCheckBox>
#include <QDateTime>
#include <QLabel>
#include <QPlainTextEdit>
#include <QRandomGenerator>
#include <QTextDocument>
#include <QTextOption>

namespace {

// The setting keeps Windows line breaks, the editor Qt's.
QString toEditor(QString s) {
    return s.replace("\r\n", "\n");
}

QString fromEditor(QString s) {
    return s.replace("\n", "\r\n");
}

}  // namespace

FormSetupSubtitleTemplate::FormSetupSubtitleTemplate(QWidget *parent) : vcl::Form(parent) {
    setWindowFlags(Qt::Tool);  // bsSizeToolWin
}

// The form has no OnCreate; it is translated and set up when shown (DoTranslate).
void FormSetupSubtitleTemplate::translate() {
    const Lang &lang = Lang::instance();
    setWindowTitle(lang.tr("SetupSubtitle_Title"));
    if (auto *c = find<QCheckBox>("CheckBoxDefault")) c->setText(lang.tr("Default"));
    if (auto *m = find<QPlainTextEdit>("MemoTemplate")) {
        QTextOption o = m->document()->defaultTextOption();
        o.setAlignment(Qt::AlignHCenter);  // Alignment = taCenter
        m->document()->setDefaultTextOption(o);
    }
    if (auto *l = find<QLabel>("LabelPreview")) l->setAlignment(Qt::AlignHCenter | Qt::AlignTop);
}

// L49–L51: the example takes half of the space above the bottom panel.
void FormSetupSubtitleTemplate::FormResize(vcl::Event *) {
    vcl::AlignLayout *l = layoutOf(this);
    QWidget *preview = find<QWidget>("LabelPreview");
    QWidget *bottom = find<QWidget>("PanelBottom");
    if (!l || !preview || !bottom) return;
    l->setControlSize(preview, QSize(l->controlRect(preview).width(), (height() - l->controlRect(bottom).height()) / 2));
}

// L56–L61.
void FormSetupSubtitleTemplate::FormShow(vcl::Event *) {
    translate();
    const QString s = Settings::instance().readString("subtitleTemplate");
    if (auto *c = find<QCheckBox>("CheckBoxDefault")) c->setChecked(s.size() < 2);
    if (auto *m = find<QPlainTextEdit>("MemoTemplate")) m->setPlainText(toEditor(s));
    CheckBoxDefaultClick(nullptr);
}

// L64–L70.
void FormSetupSubtitleTemplate::ButtonOkClick(vcl::Event *) {
    auto *c = find<QCheckBox>("CheckBoxDefault");
    auto *m = find<QPlainTextEdit>("MemoTemplate");
    if (c && c->isChecked()) Settings::instance().remove("subtitleTemplate");
    else if (m) Settings::instance().writeString("subtitleTemplate", fromEditor(m->toPlainText()));
    close();
}

// L73–L81: "Default" shows the built-in template, read-only.
void FormSetupSubtitleTemplate::CheckBoxDefaultClick(vcl::Event *) {
    auto *c = find<QCheckBox>("CheckBoxDefault");
    auto *m = find<QPlainTextEdit>("MemoTemplate");
    if (!c || !m) return;
    if (c->isChecked()) m->setPlainText(toEditor(gpsfiles::srtDefaultTemplate(settings)));
    m->setEnabled(!c->isChecked());
    generate();
}

void FormSetupSubtitleTemplate::MemoTemplateKeyUp(vcl::Event *) {
    generate();
}

// Generate L86–L97: now, a random place, altitude below 6000 m, bearing, speed below 200 km/h
// and a trip below 10 km.
void FormSetupSubtitleTemplate::generate() {
    auto *m = find<QPlainTextEdit>("MemoTemplate");
    auto *l = find<QLabel>("LabelPreview");
    if (!m || !l) return;
    QRandomGenerator *r = QRandomGenerator::global();
    const double lng = r->bounded(-180000000, 180000000) / 1e6;
    const double lat = r->bounded(-90000000, 90000000) / 1e6;
    l->setText(gpsfiles::srtPreview(fromEditor(m->toPlainText()), settings, fmt::fromQDateTime(QDateTime::currentDateTime()), lat, lng,
                                    r->bounded(6000), r->bounded(360), r->bounded(200), r->bounded(10000)));
}
