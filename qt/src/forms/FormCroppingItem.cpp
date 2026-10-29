#include "FormCroppingItem.h"

#include "core/Format.h"
#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"

#include <QAbstractButton>
#include <QDir>
#include <QGroupBox>
#include <QLabel>
#include <QLineEdit>
#include <QLocale>
#include <QRadioButton>
#include <QSignalBlocker>
#include <QSlider>

#include <cmath>

QString durationText(double seconds) {
    const qint64 s = qint64(std::lround(std::max(0.0, seconds)));
    return QStringLiteral("%1:%2:%3").arg(s / 3600).arg(s / 60 % 60, 2, 10, QChar('0')).arg(s % 60, 2, 10, QChar('0'));
}

double parseDuration(const QString &text, bool *ok) {
    const QStringList parts = text.trimmed().split(':');
    double total = 0;
    bool good = !parts.isEmpty() && parts.size() <= 3;
    for (const QString &p : parts) {
        bool n = false;
        const double v = QString(p).replace(',', '.').toDouble(&n);
        good = good && n;
        total = total * 60 + v;
    }
    if (ok) *ok = good;
    return good ? total : 0;
}

FormCroppingItem::FormCroppingItem(QWidget *parent) : vcl::Form(parent) {
    setWindowFlags(Qt::Dialog | Qt::WindowCloseButtonHint);
}

void FormCroppingItem::FormCreate(vcl::Event *) {
    const Lang &lang = Lang::instance();
    setWindowTitle(lang.tr("Cropping_Title") + ". " + lang.tr("N_Edit"));
    auto label = [&](const char *name, const QString &text) {
        if (auto *l = find<QLabel>(name)) l->setText(text);
    };
    label("LabelDuration", lang.tr("Duration") + ":");
    label("LabelStart", lang.tr("Started") + ":");
    label("LabelFinish", lang.tr("Finished") + ":");
    label("LabelSpeed", lang.tr("Speed") + ":");
    if (auto *b = find<QAbstractButton>("ButtonCancel")) b->setText(lang.tr("Cancel"));
    if (auto *g = find<QGroupBox>("SpeedUpMethod")) g->setTitle(lang.tr("Cropping_SpeedUpMethod"));
    for (const char *n : {"SUM_FPS_Caption", "SUM_KeyFrames_Caption"}) {
        if (auto *l = find<QLabel>(n)) l->setWordWrap(true);
    }
}

// SF-CROPPINGITEM-005.
QString FormCroppingItem::speedText(double speed) {
    if (speed < 1) return QStringLiteral("1/%1").arg(std::lround(1 / speed));
    return QString::number(std::lround(speed));
}

// SF-CROPPINGITEM-006.
double FormCroppingItem::parseSpeed(const QString &text) {
    bool ok = false;
    if (text.startsWith("1/")) {
        const int n = text.mid(2).trimmed().toInt(&ok);
        return ok && n > 0 ? 1.0 / n : -1;
    }
    const int n = text.trimmed().toInt(&ok);
    return ok ? n : -1;
}

// SF-CROPPINGITEM-002: speed presets by the key-frame interval.
void FormCroppingItem::buildPresets() {
    presets = {1.0 / 8, 1.0 / 4, 1.0 / 2, 1};
    int key = item->gop;
    switch (item->gop) {
    case 8: presets += {4, 8, 16, 32, 64, 128, 512, 1024}; break;
    case 15: presets += {2, 3, 8, 15, 30, 60, 120, 480, 1020}; break;
    case 32: presets += {2, 3, 4, 16, 32, 64, 128, 256, 1024}; break;
    case 0: presets += {2, 3, 4, 8, 15, 30, 60, 120}; break;
    default: {
        const int n = item->gop;
        presets += {double(n / 2), double(n), double(2 * n), double(4 * n), double(8 * n), double(16 * n)};
        key = n;
    }
    }
    if (auto *l = find<QLabel>("SUM_KeyFrames_Caption")) {
        l->setText(Lang::instance().tr("Cropping_KeyFrameOnly").replace("%key%", QString::number(key)).replace("\\n", "\n"));
    }
    if (auto *t = find<QSlider>("TrackBarSpeed")) t->setRange(0, int(presets.size()) - 1);
}

void FormCroppingItem::layoutControls() {
    vcl::AlignLayout *l = layoutOf(this);
    if (!l) return;
    auto rect = [&](const char *n) { return find<QWidget>(n) ? l->controlRect(find<QWidget>(n)) : QRect(); };
    auto set = [&](const char *n, const QRect &r) {
        if (QWidget *w = find<QWidget>(n)) l->setControlRect(w, r);
    };
    // A longer (translated) caption pushes its row to the right, so it is never cut off.
    int widest = 0;
    for (const char *n : {"LabelStart", "LabelFinish", "LabelSpeed"}) {
        if (QWidget *w = find<QWidget>(n)) widest = std::max(widest, w->sizeHint().width());
    }
    const int shift = std::max(0, widest + 4 - std::min(rect("EditTime1").left(), rect("TrackBarSpeed").left()));
    if (shift > 0 && !shifted) {
        for (const char *n : {"EditTime1", "EditTime2", "LabelDateTime1", "LabelDateTime2", "EditP1", "EditP2", "TrackBarSpeed", "EditSpeed"}) {
            set(n, rect(n).translated(shift, 0));
        }
        shifted = true;
    }
    // Captions end 4 px before their edits.
    for (const auto &[caption, edit] : {std::pair{"LabelStart", "EditTime1"}, std::pair{"LabelFinish", "EditTime2"},
                                        std::pair{"LabelSpeed", "TrackBarSpeed"}}) {
        QWidget *c = find<QWidget>(caption);
        if (!c) continue;
        QRect r = rect(caption);
        r.setWidth(c->sizeHint().width());
        r.moveRight(rect(edit).left() - 4 - 1);
        set(caption, r);
    }
    for (const char *n : {"LabelFileName", "LabelDuration"}) set(n, rect(n).translated(-rect(n).left(), 0));
    if (QWidget *d = find<QWidget>("LabelDuration")) {
        QRect v = rect("LabelDurationValue");
        v.moveLeft(d->sizeHint().width() + 4);
        set("LabelDurationValue", v);
    }
    const int right = rect("EditP1").right();
    // OK and Cancel centred; the method box spans the width with its texts stacked by height.
    QRect ok = rect("ButtonOk"), cancel = rect("ButtonCancel");
    const int pair = ok.width() + 10 + cancel.width();
    ok.moveLeft((right + 1 - pair) / 2);
    cancel.moveLeft(ok.right() + 1 + 10);
    set("ButtonOk", ok);
    set("ButtonCancel", cancel);
    if (auto *box = find<QWidget>("SpeedUpMethod")) {
        vcl::AlignLayout *bl = layoutOf(box);
        QRect b = rect("SpeedUpMethod");
        b.setLeft(0);
        b.setRight(right);
        const int textLeft = 28, textWidth = b.width() - textLeft - 8;
        int y = 22;
        for (const auto &[radio, caption] : {std::pair{"SUM_FPS", "SUM_FPS_Caption"}, std::pair{"SUM_KeyFrames", "SUM_KeyFrames_Caption"}}) {
            auto *r = find<QWidget>(radio);
            auto *c = find<QLabel>(caption);
            if (!r || !c) continue;
            const int h = c->heightForWidth(textWidth);
            bl->setControlRect(r, QRect(5, y, 18, 18));
            bl->setControlRect(c, QRect(textLeft, y, textWidth, h));
            y += std::max(h, 18) + 10;
        }
        b.setHeight(y);
        set("SpeedUpMethod", b);
    }
    // The file path is shortened in the middle to the width of the rows below it.
    if (auto *file = find<QLabel>("LabelFileName")) {
        file->setProperty("vclAutoSize", false);
        file->setText(file->fontMetrics().elidedText(file->property("fullText").toString(), Qt::ElideMiddle, right + 1));
        set("LabelFileName", QRect(0, rect("LabelFileName").top(), right + 1, file->sizeHint().height()));
    }
    // AutoSize with BorderWidth 10.
    QRect all;
    for (QObject *o : children()) {
        auto *w = qobject_cast<QWidget *>(o);
        if (w && !w->isHidden() && !w->isWindow()) all |= l->controlRect(w);
    }
    setFixedSize(all.right() + 1 + 20, all.bottom() + 1 + 20);
}

// SF-CROPPINGITEM-003.
bool FormCroppingItem::edit(CropItem &it, double shift) {
    item = &it;
    tzShift = shift;
    buildPresets();
    if (auto *l = find<QLabel>("LabelFileName")) l->setProperty("fullText", QDir::toNativeSeparators(it.path));
    if (auto *l = find<QLabel>("LabelDurationValue")) l->setText(durationText(it.durationMs / 1000.0));
    layoutControls();
    if (auto *e = find<QLineEdit>("EditP1")) e->setText(QString::number(it.p1, 'f', 6));
    if (auto *e = find<QLineEdit>("EditP2")) e->setText(QString::number(it.p2, 'f', 6));
    if (auto *e = find<QLineEdit>("EditSpeed")) e->setText(speedText(it.speed));
    if (auto *r = find<QRadioButton>(it.speedUpFps ? "SUM_FPS" : "SUM_KeyFrames")) r->setChecked(true);
    EditSpeedChange(nullptr);
    if (auto *r = find<QRadioButton>(it.speedUpFps ? "SUM_FPS" : "SUM_KeyFrames")) r->setChecked(true);
    layoutControls();
    return showModal() == mrOk;
}

// SF-CROPPINGITEM-004.
void FormCroppingItem::ButtonOkClick(vcl::Event *) {
    if (!item) return;
    auto value = [&](const char *name) {
        auto *e = find<QLineEdit>(name);
        return e ? QString(e->text()).replace(',', '.').toDouble() : 0.0;
    };
    item->p1 = std::clamp(value("EditP1"), 0.0, 1.0);
    item->p2 = std::clamp(value("EditP2"), 0.0, 1.0);
    const double speed = parseSpeed(find<QLineEdit>("EditSpeed") ? find<QLineEdit>("EditSpeed")->text() : QString());
    item->speed = speed > 0 ? speed : 1;
    auto *fps = find<QRadioButton>("SUM_FPS");
    item->speedUpFps = fps && fps->isChecked();
    setModalResult(mrOk);
}

void FormCroppingItem::ButtonCancelClick(vcl::Event *) {
    setModalResult(mrCancel);
}

// SF-CROPPINGITEM-009: a fraction gives the time edit and the absolute date and time.
void FormCroppingItem::pChanged(int which) {
    if (!item) return;
    auto *p = find<QLineEdit>(which == 1 ? "EditP1" : "EditP2");
    if (!p) return;
    const double seconds = QString(p->text()).replace(',', '.').toDouble() * item->durationMs / 1000.0;
    if (auto *l = find<QLabel>(which == 1 ? "LabelDateTime1" : "LabelDateTime2")) {
        const QDateTime dt = fmt::toQDateTime(item->start + seconds * kSecond + tzShift);
        l->setText(QLocale::system().toString(dt.date(), QLocale::ShortFormat) + "  " + dt.time().toString("HH:mm:ss"));
        l->adjustSize();
    }
    if (changing) return;
    changing = true;
    if (auto *t = find<QLineEdit>(which == 1 ? "EditTime1" : "EditTime2")) t->setText(durationText(seconds));
    changing = false;
}

// SF-CROPPINGITEM-010: a time gives the fraction.
void FormCroppingItem::timeChanged(int which) {
    if (!item || changing || item->durationMs <= 0) return;
    auto *t = find<QLineEdit>(which == 1 ? "EditTime1" : "EditTime2");
    if (!t) return;
    bool ok = false;
    double seconds = parseDuration(t->text(), &ok);
    if (!ok) seconds = which == 1 ? 0 : item->durationMs / 1000.0;
    changing = true;
    if (auto *p = find<QLineEdit>(which == 1 ? "EditP1" : "EditP2")) p->setText(QString::number(seconds * 1000 / item->durationMs, 'f', 6));
    changing = false;
}

void FormCroppingItem::EditP1Change(vcl::Event *) {
    pChanged(1);
}

void FormCroppingItem::EditP2Change(vcl::Event *) {
    pChanged(2);
}

void FormCroppingItem::EditTime1Change(vcl::Event *) {
    timeChanged(1);
}

void FormCroppingItem::EditTime2Change(vcl::Event *) {
    timeChanged(2);
}

// SF-CROPPINGITEM-007.
void FormCroppingItem::TrackBarSpeedChange(vcl::Event *) {
    auto *t = find<QSlider>("TrackBarSpeed");
    auto *e = find<QLineEdit>("EditSpeed");
    if (!t || !e || t->value() < 0 || t->value() >= presets.size()) return;
    e->setText(speedText(presets[t->value()]));
}

// SF-CROPPINGITEM-008.
void FormCroppingItem::EditSpeedChange(vcl::Event *) {
    auto *e = find<QLineEdit>("EditSpeed");
    if (!e || !item) return;
    const double speed = parseSpeed(e->text());
    if (auto *t = find<QSlider>("TrackBarSpeed")) {
        for (int i = 0; i < presets.size(); ++i) {
            if (std::abs(presets[i] - speed) < 1e-9) {
                const QSignalBlocker block(t);
                t->setValue(i);
            }
        }
    }
    auto *box = find<QWidget>("SpeedUpMethod");
    if (!box) return;
    box->setVisible(speed > 1);
    if (speed > 1) {
        if (auto *l = find<QLabel>("SUM_FPS_Caption")) {
            l->setText(Lang::instance()
                           .tr("Cropping_BigFPS")
                           .replace("%fps%", QString::number(std::lround(speed * item->fps)))
                           .replace("\\n", "\n"));
        }
        if (auto *k = find<QRadioButton>("SUM_KeyFrames")) k->setChecked(true);
    }
    layoutControls();
}

// SF-CROPPINGITEM-011.
void FormCroppingItem::SUM_FPS_CaptionClick(vcl::Event *) {
    if (auto *r = find<QRadioButton>("SUM_FPS")) r->setChecked(true);
}

void FormCroppingItem::SUM_KeyFrames_CaptionClick(vcl::Event *) {
    if (auto *r = find<QRadioButton>("SUM_KeyFrames")) r->setChecked(true);
}
