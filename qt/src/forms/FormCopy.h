// TFormCopy ("Copying"): copies the files of the selected list entries (videos, their GPS files
// and GPS logs) to a folder, with progress, speed, remaining time, pause and "open folder".
// Spec: shell-settings-forms.md §6.4 (SF-COPY-001…012), file-list.md FL-061.
#pragma once

#include "vcl/Form.h"

#include <QDateTime>
#include <QElapsedTimer>
#include <QMutex>
#include <QThread>

#include <atomic>

// The copy worker (TFormCopyThread).
class CopyWorker : public QThread {
    Q_OBJECT
public:
    CopyWorker(QStringList files, QString folder, QObject *parent = nullptr);
    ~CopyWorker() override;

    void stop() { stopping = true; }
    void setPaused(bool p) { paused = p; }

    // Progress, read by the window's timer.
    int index() const { return current.load(); }
    int count() const { return int(sources.size()); }
    qint64 copied() const { return copiedBytes.load(); }
    qint64 total() const { return totalBytes; }
    double speed() const { return bytesPerSecond.load(); }
    QString currentSource() const;
    QString lastDestination() const;

signals:
    // SF-COPY-005: asked on the UI thread; the answer comes back through `replace`.
    void askReplace(const QString &destination, bool *replace);

protected:
    void run() override;

private:
    void copyOne(const QString &from, const QString &to);

    QStringList sources;
    QString folder;
    qint64 totalBytes = 0;
    std::atomic<int> current{0};
    std::atomic<qint64> copiedBytes{0};
    std::atomic<double> bytesPerSecond{0};
    std::atomic<bool> stopping{false}, paused{false};
    mutable QMutex mutex;
    QString lastDest;
};

class FormCopy : public vcl::Form {
    Q_OBJECT
public:
    explicit FormCopy(QWidget *parent = nullptr);
    ~FormCopy() override;

    // SF-COPY-003: asks for the folder (unless one is given) and starts; false when cancelled.
    bool popup(const QStringList &files, const QString &displayName, const QString &folder = {});
    bool isFinished() const { return finished; }

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormResize(vcl::Event *e);
    Q_INVOKABLE void FormClose(vcl::Event *e);
    Q_INVOKABLE void TimerTimer(vcl::Event *e);
    Q_INVOKABLE void ButtonPauseClick(vcl::Event *e);
    Q_INVOKABLE void ButtonOpenFolderClick(vcl::Event *e);

private:
    void setText(const char *label, const QString &text);
    CopyWorker *worker = nullptr;
    QDateTime started;
    QDateTime pausedAt;
    bool finished = false;
};
