// The Rust file manager (rv_fm_*) seen from the UI: rows re-read when its version changes.
// Spec: file-list.md §3 A (FL-001…FL-012), §4.
#pragma once

#include "core/Group.h"

#include <QObject>
#include <QTimer>
#include <QVector>

struct RvFileManager;

struct FileRow {
    QString name, path;
    int files = 0;
    int lock = 0;           // 0 none, 1 all files read-only, 2 some
    qint64 durationMs = 0;
    int durationState = 0;  // 0 unknown, 1 estimated, 2 parser failed, 3 exact
    TDateTime finish = 0;
    double duration = 0;    // days
    qint64 size = 0;
    int width = 0, height = 0;
    double trackM = 0;
    double avgSpeed = -1;
    int gps = 0;            // 0 none, 1 points without a fix, 2 valid
    TDateTime start() const { return duration > 0 ? finish - duration : finish; }
};

struct FileRows {
    QString statusKey, statusArg;  // FL-011
    bool anyLocked = false;
    QVector<FileRow> rows;
};

class FileManager : public QObject {
    Q_OBJECT
public:
    FileManager(bool autoSearch, bool gps, QObject *parent = nullptr);
    ~FileManager() override;

    void add(const QString &path);
    void clear();
    void setSort(int column, bool ascending);
    bool idle() const;
    // Re-reads the rows now (normally done by the poll timer).
    void refresh();

    const FileRows &rows() const { return rows_; }
    Group item(int index) const;

    void remove(const QList<int> &rows);
    void setLock(const QList<int> &rows, bool lock);
    void group(const QList<int> &rows);
    void ungroup(const QList<int> &rows);
    void ungroupAll();
    void groupAuto();

signals:
    void changed();

private:
    void poll();
    RvFileManager *fm = nullptr;
    quint64 version = ~0ull;
    QTimer timer;
    FileRows rows_;
};
