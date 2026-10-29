#include "FormCopy.h"

#include "core/Settings.h"
#include "draw/Bitmaps.h"
#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"

#include <QAbstractButton>
#include <QApplication>
#include <QDir>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QLabel>
#include <QMessageBox>
#include <QProcess>
#include <QProgressBar>
#include <QTimeZone>
#include <QTimer>

#include <cmath>

namespace {

constexpr qint64 kBlock = 1 << 20;  // SF-COPY-004: 1 MiB copy buffer

QString hms(qint64 seconds) {
    return QStringLiteral("%1:%2:%3")
        .arg(seconds / 3600)
        .arg(seconds / 60 % 60, 2, 10, QChar('0'))
        .arg(seconds % 60, 2, 10, QChar('0'));
}

// UnitUtilsFile FileIsSame: equal size and last-write time.
bool sameFile(const QFileInfo &a, const QFileInfo &b) {
    return a.size() == b.size() && a.lastModified(QTimeZone::UTC) == b.lastModified(QTimeZone::UTC);
}

}  // namespace

// --- Worker -----------------------------------------------------------------------------------

CopyWorker::CopyWorker(QStringList files, QString dest, QObject *parent)
    : QThread(parent), sources(std::move(files)), folder(QDir(dest).absolutePath()) {
    for (const QString &f : std::as_const(sources)) totalBytes += QFileInfo(f).size();
    QDir().mkpath(folder);
}

CopyWorker::~CopyWorker() {
    stop();
    wait();
}

QString CopyWorker::currentSource() const {
    const int i = current.load();
    return i < sources.size() ? sources[i] : QString();
}

QString CopyWorker::lastDestination() const {
    QMutexLocker lock(&mutex);
    return lastDest;
}

// SF-COPY-005 and SF-COPY-007.
void CopyWorker::run() {
    QList<QPair<QString, QString>> written;  // source, copy
    for (int i = 0; i < sources.size() && !stopping; ++i) {
        current = i;
        const QString to = QDir(folder).filePath(QFileInfo(sources[i]).fileName());
        if (QFileInfo::exists(to)) {
            if (sameFile(QFileInfo(sources[i]), QFileInfo(to))) continue;
            bool replace = false;
            emit askReplace(to, &replace);  // blocking, answered on the UI thread
            if (!replace) continue;
        }
        copyOne(sources[i], to);
        written.append({sources[i], to});
    }
    current = int(sources.size());
    if (stopping) return;
    msleep(500);
    // The original timestamps go onto the copies once more (writing may have changed them).
    for (const auto &[from, to] : std::as_const(written)) {
        const QFileInfo src(from);
        QFile f(to);
        if (f.open(QIODevice::ReadWrite)) {
            // In UTC: through local time, historical daylight-saving rules can shift them by an hour.
            f.setFileTime(src.fileTime(QFileDevice::FileModificationTime, QTimeZone::UTC), QFileDevice::FileModificationTime);
            f.setFileTime(src.fileTime(QFileDevice::FileBirthTime, QTimeZone::UTC), QFileDevice::FileBirthTime);
        }
    }
}

// SF-COPY-006: block copy with a smoothed throughput.
void CopyWorker::copyOne(const QString &from, const QString &to) {
    QFile in(from);
    if (!in.open(QIODevice::ReadOnly)) return;
    if (QFileInfo::exists(to)) QFile::setPermissions(to, QFile::permissions(to) | QFileDevice::WriteOwner | QFileDevice::WriteUser);
    QFile out(to);
    if (!out.open(QIODevice::WriteOnly | QIODevice::Truncate)) return;
    {
        QMutexLocker lock(&mutex);
        lastDest = to;
    }
    out.resize(in.size());
    QByteArray buffer(kBlock, Qt::Uninitialized);
    QElapsedTimer interval;
    interval.start();
    qint64 intervalBytes = 0;
    while (!stopping) {
        while (paused && !stopping) msleep(10);
        const qint64 n = in.read(buffer.data(), kBlock);
        if (n <= 0) break;
        out.write(buffer.constData(), n);
        copiedBytes += n;
        intervalBytes += n;
        if (interval.elapsed() > 4000) {
            const double sample = intervalBytes / (interval.elapsed() / 1000.0);
            const double old = bytesPerSecond.load();
            bytesPerSecond = old == 0 ? sample : sample >= old ? 0.5 * old + 0.5 * sample : 0.2 * old + 0.8 * sample;
            interval.restart();
            intervalBytes = 0;
        }
    }
    out.close();
    out.open(QIODevice::ReadWrite);
    out.setFileTime(QFileInfo(from).fileTime(QFileDevice::FileModificationTime, QTimeZone::UTC), QFileDevice::FileModificationTime);
}

// --- Window -----------------------------------------------------------------------------------

FormCopy::FormCopy(QWidget *parent) : vcl::Form(parent) {
    setAttribute(Qt::WA_DeleteOnClose);  // SF-COPY-011
    setWindowFlags(Qt::Window | Qt::WindowMinimizeButtonHint | Qt::WindowCloseButtonHint);
}

FormCopy::~FormCopy() {
    delete worker;
}

void FormCopy::setText(const char *label, const QString &text) {
    if (auto *l = find<QLabel>(label)) {
        l->setProperty("fullText", text);
        l->setText(text);
        l->adjustSize();
    }
}

// SF-COPY-001.
void FormCopy::FormCreate(vcl::Event *) {
    const Lang &lang = Lang::instance();
    setWindowIcon(QIcon(":/original/images/ICON_COPY.ICO"));
    setWindowTitle(lang.tr("Copying_Title"));
    setText("LabelFileCaption", lang.tr("File") + ":");
    setText("LabelSizeCaption", lang.tr("Size") + ":");
    setText("LabelSpeedCaption", lang.tr("Speed") + ":");
    setText("LabelTimeCaption", lang.tr("Time") + ":");
    setText("LabelRemainCaption", lang.tr("Remains") + ":");
    for (const char *n : {"LabelFile", "LabelSize", "LabelSpeed", "LabelTime", "LabelRemain"}) setText(n, QString());
    for (const auto &[button, key] : {std::pair{"ButtonOpenFolder", "OpenFolder"}, std::pair{"ButtonPause", "Pause"}}) {
        if (auto *b = find<QAbstractButton>(button)) {
            b->setText(lang.tr(key));
            QFont f = b->font();
            f.setUnderline(true);
            b->setFont(f);
        }
    }
    // Only the width can change: the height fits the labels and the progress bar.
    if (auto *bar = find<QProgressBar>("ProgressBar")) {
        const int h = layoutOf(this)->controlRect(bar).bottom() + 1 + 8;
        setMinimumHeight(h);
        setMaximumHeight(h);
        resize(width(), h);
    }
    setMinimumWidth(350);
}

// SF-COPY-002.
void FormCopy::FormResize(vcl::Event *) {
    vcl::AlignLayout *l = layoutOf(this);
    if (!l) return;
    const int w = width();
    auto place = [&](const char *name, auto adjust) {
        if (QWidget *x = find<QWidget>(name)) {
            QRect r = l->controlRect(x);
            adjust(r);
            l->setControlRect(x, r);
        }
    };
    place("ButtonOpenFolder", [&](QRect &r) { r.moveLeft(w - r.width() - 1); });
    place("LabelSpeed", [&](QRect &r) { r.moveLeft(w - 100); });
    place("LabelRemain", [&](QRect &r) { r.moveLeft(w - 100); });
    for (const auto &[caption, value] : {std::pair{"LabelFileCaption", "LabelFile"}, std::pair{"LabelSizeCaption", "LabelSize"},
                                         std::pair{"LabelSpeedCaption", "LabelSpeed"}, std::pair{"LabelTimeCaption", "LabelTime"},
                                         std::pair{"LabelRemainCaption", "LabelRemain"}}) {
        QWidget *c = find<QWidget>(caption);
        QWidget *v = find<QWidget>(value);
        if (!c || !v) continue;
        QRect r = l->controlRect(c);
        r.setWidth(c->sizeHint().width());  // labels size to their text (AutoSize)
        r.moveRight(l->controlRect(v).left() - 3 - 1);
        l->setControlRect(c, r);
    }
    // A long path is shortened in the middle so it ends before Open Folder.
    if (auto *file = find<QLabel>("LabelFile"); file && find<QWidget>("ButtonOpenFolder")) {
        const int room = l->controlRect(find<QWidget>("ButtonOpenFolder")).left() - 4 - l->controlRect(file).left();
        const QString full = file->property("fullText").toString();
        file->setText(file->fontMetrics().elidedText(full, Qt::ElideMiddle, std::max(room, 40)));
    }
    place("ProgressBar", [&](QRect &r) {
        r.setLeft(8);
        r.setWidth(w - 16);
        r.moveBottom(height() - 8 - 1);
    });
}

// SF-COPY-003 (with the system folder picker in place of TFormSelectFolder).
bool FormCopy::popup(const QStringList &files, const QString &displayName, const QString &given) {
    Settings &s = Settings::instance();
    const QString folder = !given.isEmpty() ? given
                                            : QFileDialog::getExistingDirectory(parentWidget(),
                                                                                Lang::instance().tr("SelectFolderFor") + " " + displayName,
                                                                                s.readString("copyFolder", "C:\\"));
    if (folder.isEmpty()) return false;
    s.writeString("copyFolder", folder);
    worker = new CopyWorker(files, folder);
    connect(worker, &CopyWorker::askReplace, this, [this](const QString &dest, bool *replace) {
        const Lang &lang = Lang::instance();
        *replace = QMessageBox::question(this, windowTitle(),
                                         lang.tr("Mes_FileExists") + "\n" + QDir::toNativeSeparators(dest) + "\n\n" +
                                             lang.tr("Mes_SureToReplace") + " ?") == QMessageBox::Yes;
    }, Qt::BlockingQueuedConnection);
    started = QDateTime::currentDateTime();
    worker->start();
    setWindowTitle(windowTitle() + " " + displayName);
    TimerTimer(nullptr);
    show();
    if (QTimer *t = timer("Timer")) t->start();
    return true;
}

// SF-COPY-008.
void FormCopy::TimerTimer(vcl::Event *) {
    if (!worker) return;
    const Lang &lang = Lang::instance();
    const qint64 mb = 1024 * 1024;
    auto *bar = find<QProgressBar>("ProgressBar");
    if (worker->isFinished() || worker->index() >= worker->count()) {
        if (QTimer *t = timer("Timer")) t->stop();
        if (!finished) {
            finished = true;
            setText("LabelFile", QDir::toNativeSeparators(worker->lastDestination()));
            setText("LabelSize", QStringLiteral("%1 %2").arg(worker->copied() / mb).arg(lang.tr("Mb")));
            if (bar) bar->setValue(100);
            for (const char *n : {"LabelSpeedCaption", "LabelSpeed", "LabelRemainCaption", "LabelRemain", "ButtonPause"}) {
                if (QWidget *w = find<QWidget>(n)) w->hide();
            }
            QApplication::beep();
            FormResize(nullptr);
        }
        return;
    }
    const qint64 total = std::max<qint64>(worker->total(), 1);
    const int percent = int(std::clamp<qint64>(std::llround(worker->copied() * 100.0 / total), 0, 100));
    setText("LabelFile", QStringLiteral("%1 / %2  -  %3")
                             .arg(worker->index() + 1)
                             .arg(worker->count())
                             .arg(QDir::toNativeSeparators(worker->currentSource())));
    setText("LabelSize", QStringLiteral("%1%  -  %2 %3 / %4 %3").arg(percent).arg(worker->copied() / mb).arg(lang.tr("Mb")).arg(total / mb));
    if (bar) bar->setValue(percent);
    if (worker->speed() > 0) {
        setText("LabelSpeed", QStringLiteral("%1 %2").arg(worker->speed() / mb, 0, 'f', 1).arg(lang.tr("Mbytepsec")));
        setText("LabelRemain", hms(qint64(std::max<qint64>(worker->total() - worker->copied(), 0) / worker->speed())));
    } else {
        setText("LabelSpeed", QString());
        setText("LabelRemain", QString());
    }
    if (pausedAt.isNull()) setText("LabelTime", hms(started.secsTo(QDateTime::currentDateTime())));
    FormResize(nullptr);
}

// SF-COPY-009.
void FormCopy::ButtonPauseClick(vcl::Event *) {
    auto *b = find<QAbstractButton>("ButtonPause");
    if (!b || !worker) return;
    const bool pause = b->isChecked();
    worker->setPaused(pause);
    if (pause) {
        pausedAt = QDateTime::currentDateTime();
    } else if (!pausedAt.isNull()) {
        started = started.addMSecs(pausedAt.msecsTo(QDateTime::currentDateTime()));
        pausedAt = QDateTime();
    }
}

// SF-COPY-010 (without the original's 3-second UI freeze: the copy resumes after 3 s by timer).
void FormCopy::ButtonOpenFolderClick(vcl::Event *) {
    if (!worker) return;
    auto *pause = find<QAbstractButton>("ButtonPause");
    const bool wasPaused = pause && pause->isChecked();
    if (!wasPaused) worker->setPaused(true);
    const QString target = worker->lastDestination();
    QProcess::startDetached("explorer.exe", {"/select,", QDir::toNativeSeparators(target)});
    if (!wasPaused) QTimer::singleShot(3000, this, [this] {
        if (worker) worker->setPaused(false);
    });
}

// SF-COPY-011: closing cancels; a partly written file stays.
void FormCopy::FormClose(vcl::Event *) {
    if (QTimer *t = timer("Timer")) t->stop();
    if (worker) worker->stop();
}
