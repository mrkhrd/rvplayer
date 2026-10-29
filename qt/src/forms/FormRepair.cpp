#include "FormRepair.h"

#include "i18n/Lang.h"
#include "rv_ffi.h"
#include "vcl/AlignLayout.h"

#include <QAbstractButton>
#include <QApplication>
#include <QCloseEvent>
#include <QDir>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QGroupBox>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QProcess>
#include <QProgressBar>
#include <QRadioButton>
#include <QTimer>
#include <QTimeZone>

#include <cmath>

namespace {

// SF-REPAIR-009 step 2: the reference must be a recording the app can read (CheckFile).
bool parsable(const QString &path) {
    char *raw = rv_file_info_json(QDir::toNativeSeparators(path).toUtf8().constData());
    const QJsonObject info = QJsonDocument::fromJson(raw).object();
    rv_string_free(raw);
    return info.value("duration").toDouble() > 0;
}

}  // namespace

FormRepair::FormRepair(QWidget *parent) : vcl::Form(parent) {
    setWindowFlag(Qt::Window);
}

FormRepair::~FormRepair() {
    if (job) job->cancel();
}

// SF-REPAIR-001.
void FormRepair::FormCreate(vcl::Event *) {
    const Lang &lang = Lang::instance();
    setWindowIcon(QIcon(":/original/images/ICON_REPAIR.ICO"));
    setWindowTitle(lang.tr("Repair_Title"));
    auto text = [&](const char *name, const QString &t) {
        if (auto *l = find<QLabel>(name)) l->setText(t);
        else if (auto *b = find<QAbstractButton>(name)) b->setText(t);
        else if (auto *g = find<QGroupBox>(name)) g->setTitle(t);
    };
    text("LabelBroken", lang.tr("Repair_BrokenFile") + ":");
    text("LabelRef", lang.tr("Repair_RefFile") + ":");
    text("GroupBoxMethod", lang.tr("Repair_Method"));
    text("LabelCaptionFast", lang.tr("Repair_MethodFast").replace("\\n", "\n"));
    text("ButtonRepair", lang.tr("Repair_BtnStart"));
    // The Full method (an embedded third-party program) is not offered (decisions.md Q-5).
    for (const char *n : {"RadioButtonFull", "LabelCaptionFull", "LabelResult", "EditResult", "ButtonFileResult"}) {
        if (QWidget *w = find<QWidget>(n)) w->hide();
    }
    if (auto *fast = find<QRadioButton>("RadioButtonFast")) fast->setChecked(true);
    setEnable(true);
}

// SF-REPAIR-002. With the Full method hidden the box holds only the Fast description, and the
// form is as tall as its contents.
void FormRepair::FormResize(vcl::Event *) {
    vcl::AlignLayout *l = layoutOf(this);
    if (!l) return;
    const int w = width() - 32, h = height() - 32;  // BorderWidth 16
    auto rect = [&](const char *n) { return find<QWidget>(n) ? l->controlRect(find<QWidget>(n)) : QRect(); };
    auto set = [&](const char *n, const QRect &r) {
        if (QWidget *x = find<QWidget>(n)) l->setControlRect(x, r);
    };
    for (const auto &[edit, button] : {std::pair{"EditBroken", "ButtonFileBorken"}, std::pair{"EditRef", "ButtonFileRef"}}) {
        QRect b = rect(button);
        b.moveLeft(w - b.width());
        set(button, b);
        QRect e = rect(edit);
        e.setRight(b.left() - 8);
        set(edit, e);
    }
    QRect g = rect("GroupBoxMethod");
    g.setWidth(w);
    auto *box = find<QWidget>("GroupBoxMethod");
    auto *fast = find<QLabel>("LabelCaptionFast");
    if (box && fast) {
        vcl::AlignLayout *bl = layoutOf(box);
        QRect r = bl->controlRect(fast);
        r.setWidth(w - r.left() - 8);
        r.setHeight(fast->heightForWidth(r.width()));
        bl->setControlRect(fast, r);
        g.setHeight(r.bottom() + 1 + 12);
    }
    set("GroupBoxMethod", g);
    QRect b = rect("ButtonRepair");
    b.moveLeft((w - b.width()) / 2);
    b.moveTop(g.bottom() + 1 + 16);
    set("ButtonRepair", b);
    // LabelStatus 4 px above the progress bar (alBottom).
    const int bar = rect("ProgressBar").height();
    QRect s = rect("LabelStatus");
    s.moveBottom(h - bar - 4 - 1);
    set("LabelStatus", s);
    const int need = b.bottom() + 1 + 8 + s.height() + 4 + bar + 32;
    if (minimumHeight() != need) {
        setMinimumHeight(need);
        resize(width(), need);
    }
}

// SF-REPAIR-004.
void FormRepair::setEnable(bool idle) {
    for (const char *n : {"ButtonRepair", "EditBroken", "EditRef", "ButtonFileBorken", "ButtonFileRef", "GroupBoxMethod"}) {
        if (QWidget *w = find<QWidget>(n)) w->setEnabled(idle);
    }
    for (const char *n : {"ProgressBar", "LabelStatus"}) {
        if (QWidget *w = find<QWidget>(n)) w->setVisible(!idle);
    }
    if (QTimer *t = timer("TimerConsole"); t && idle) t->stop();
    FormResize(nullptr);
}

// SF-REPAIR-003: the reference is the list's exactly-timed file nearest in time (not a rear camera's).
void FormRepair::open(const QString &broken, const QVector<GroupFile> &candidates, const GroupFile *brokenFile) {
    auto *b = find<QLineEdit>("EditBroken");
    auto *r = find<QLineEdit>("EditRef");
    if (b) b->clear();
    if (r) r->clear();
    if (!broken.isEmpty() && b) {
        b->setText(QDir::toNativeSeparators(broken));
        double best = 999999;
        QString ref;
        for (const GroupFile &f : candidates) {
            if (QFileInfo(f.path) == QFileInfo(broken) || f.durationState != 3 || f.path.contains("_R.", Qt::CaseInsensitive)) continue;
            const double d = brokenFile ? std::abs(f.start() - brokenFile->start()) + std::abs(f.finish - brokenFile->finish) : 0;
            if (d < best) {
                best = d;
                ref = f.path;
            }
        }
        if (r) r->setText(QDir::toNativeSeparators(ref));
    }
    // Paths show from their start, as in a TEdit.
    for (QLineEdit *e : {b, r}) {
        if (e) e->setCursorPosition(0);
    }
    setEnable(true);
    if (auto *fast = find<QRadioButton>("RadioButtonFast")) fast->setChecked(true);
    show();
    raise();
    activateWindow();
}

void FormRepair::EditBrokenChange(vcl::Event *) {}

// SF-REPAIR-006.
void FormRepair::ButtonFileBorkenClick(vcl::Event *e) {
    QObject *s = e ? e->sender : nullptr;
    const bool ref = s && (s->objectName() == "ButtonFileRef" || s->objectName() == "EditRef");
    auto *edit = find<QLineEdit>(ref ? "EditRef" : "EditBroken");
    if (!edit) return;
    const QString path = QFileDialog::getOpenFileName(this, QString(), edit->text(),
                                                      QStringLiteral("DashCam Files (*.mov *.mp4 *.mp4v);;Any Files (*.*)"));
    if (!path.isEmpty()) {
        edit->setText(QDir::toNativeSeparators(path));
        edit->setCursorPosition(0);
    }
}

// SF-REPAIR-005.
void FormRepair::LabelCaptionFastClick(vcl::Event *) {
    if (auto *fast = find<QRadioButton>("RadioButtonFast")) fast->setChecked(true);
}

// SF-REPAIR-009…013.
void FormRepair::ButtonRepairClick(vcl::Event *) {
    const Lang &lang = Lang::instance();
    auto *b = find<QLineEdit>("EditBroken");
    auto *r = find<QLineEdit>("EditRef");
    const QString broken = b ? QDir::fromNativeSeparators(b->text().trimmed()) : QString();
    const QString ref = r ? QDir::fromNativeSeparators(r->text().trimmed()) : QString();
    auto fail = [&](const QString &text, QLineEdit *focus) {
        QMessageBox::critical(this, lang.tr("Error"), text);
        if (focus) focus->setFocus();
    };
    if (broken.isEmpty() || !QFileInfo::exists(broken)) return fail(lang.tr("Mes_FileNotExists") + ":\r\n" + lang.tr("Repair_BrokenFile"), b);
    if (ref.isEmpty() || !QFileInfo::exists(ref) || !parsable(ref)) return fail(lang.tr("Mes_FileNotExists") + ":\r\n" + lang.tr("Repair_RefFile"), r);
    if (QFileInfo(broken) == QFileInfo(ref)) return fail(lang.tr("Repair_BrokenFile") + "   =   " + lang.tr("Repair_RefFile"), r);
    const qint64 size = QFileInfo(broken).size();
    if (size == 0) return fail(QStringLiteral("File size: 0"), b);
    if (repair::suspiciousSize(size)) {
        const QString cmd = QStringLiteral("chkdsk %1: /f").arg(broken.left(1));
        const auto answer = QMessageBox::warning(this, windowTitle(),
                                                 QDir::toNativeSeparators(broken) + "\n\n" + lang.tr("Repair_PrepareWarning").replace("%cmd%", cmd),
                                                 QMessageBox::Yes | QMessageBox::No);
        if (answer == QMessageBox::No) {
            // Run the disk check elevated, and do not repair now.
            QProcess::startDetached("powershell.exe", {"-NoProfile", "-Command",
                                                       QStringLiteral("Start-Process cmd -Verb RunAs -ArgumentList '/k %1'").arg(cmd)});
            return;
        }
        if (answer != QMessageBox::Yes) return;
    }
    if (repair::encrypted(broken)) return fail(lang.tr("Mes_FileEncrypted"), b);
    if (!repair::available()) {
        QMessageBox::information(this, windowTitle(), QStringLiteral("Repair is not available in this build yet."));
        return;
    }
    target = broken;
    created = QFileInfo(broken).fileTime(QFileDevice::FileBirthTime, QTimeZone::UTC);
    written = QFileInfo(broken).fileTime(QFileDevice::FileModificationTime, QTimeZone::UTC);
    QString error;
    job = repair::start(broken, ref, &error);
    if (!job) return fail(error, b);
    setEnable(false);
    if (QTimer *t = timer("TimerConsole")) t->start();
}

// SF-REPAIR-014/015.
void FormRepair::TimerConsoleTimer(vcl::Event *) {
    if (!job) return;
    const Lang &lang = Lang::instance();
    const repair::Status s = job->status();
    if (!s.finished && isVisible()) {
        if (auto *bar = find<QProgressBar>("ProgressBar")) bar->setValue(s.percent);
        if (auto *l = find<QLabel>("LabelStatus")) {
            l->setText(QStringLiteral("%1%    %2: %3").arg(s.percent).arg(lang.tr("Repair_FramesFound")).arg(s.frames));
        }
        return;
    }
    if (QTimer *t = timer("TimerConsole")) t->stop();
    job.reset();
    if (!isVisible()) return;
    showNormal();
    raise();
    activateWindow();
    QApplication::beep();
    QFile f(target);
    if (f.open(QIODevice::ReadWrite)) {
        f.setFileTime(created, QFileDevice::FileBirthTime);
        f.setFileTime(written, QFileDevice::FileModificationTime);
    }
    f.close();
    emit repaired(target);
    if (!s.error.isEmpty() || s.frames < 1) {
        QMessageBox::critical(this, lang.tr("Error"), s.error.isEmpty() ? lang.tr("Repair_Fail") : lang.tr("Repair_Fail") + "\n\n" + s.error);
    } else {
        QMessageBox::information(this, windowTitle(), lang.tr("Repair_Ok") + "\n\n" + lang.tr("Repair_FramesFound") + ": " + QString::number(s.frames));
    }
    setEnable(true);
}

// SF-REPAIR-014: closing stops a running repair (the engine backs out cleanly).
void FormRepair::closeEvent(QCloseEvent *e) {
    if (job) job->cancel();
    vcl::Form::closeEvent(e);
}
