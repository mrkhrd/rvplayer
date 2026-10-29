#include "FileManager.h"

#include "core/Trace.h"
#include "rv_ffi.h"

#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

#include <cstdio>

namespace {

QJsonDocument takeJson(char *raw) {
    QJsonDocument d = QJsonDocument::fromJson(raw);
    rv_string_free(raw);
    return d;
}

FileRow rowFrom(const QJsonObject &o) {
    FileRow r;
    r.name = o.value("name").toString();
    r.path = o.value("path").toString();
    r.files = o.value("files").toInt();
    r.lock = o.value("lock").toInt();
    r.durationMs = qint64(o.value("duration_ms").toDouble());
    r.durationState = o.value("duration_state").toInt();
    r.finish = o.value("finish").toDouble();
    r.duration = o.value("duration").toDouble();
    r.size = qint64(o.value("size").toDouble());
    r.width = o.value("width").toInt();
    r.height = o.value("height").toInt();
    r.trackM = o.value("track_m").toDouble();
    r.avgSpeed = o.value("avg_speed").toDouble(-1);
    r.gps = o.value("gps").toInt();
    return r;
}

std::vector<int32_t> indices(const QList<int> &rows) {
    return std::vector<int32_t>(rows.begin(), rows.end());
}

}  // namespace

FileManager::FileManager(bool autoSearch, bool gps, QObject *parent) : QObject(parent) {
    fm = rv_fm_create(autoSearch ? 1 : 0, gps ? 1 : 0);
    timer.setInterval(100);
    connect(&timer, &QTimer::timeout, this, &FileManager::poll);
    timer.start();
}

FileManager::~FileManager() {
    if (fm) rv_fm_destroy(fm);
}

void FileManager::add(const QString &path) {
    rv_fm_add(fm, QFileInfo(path).absoluteFilePath().toUtf8().constData());
}

void FileManager::clear() {
    rv_fm_clear(fm);
    refresh();
}

void FileManager::setSort(int column, bool ascending) {
    rv_fm_set_sort(fm, column, ascending ? 1 : 0);
    refresh();
}

bool FileManager::idle() const {
    return rv_fm_is_idle(fm) != 0;
}

void FileManager::poll() {
    if (rv_fm_version(fm) != version) refresh();
}

void FileManager::refresh() {
    RV_TRACE_SCOPE("FileManager::refresh");
    version = rv_fm_version(fm);
    if (TraceScope::enabled()) std::fprintf(stderr, "fm: version %llu\n", static_cast<unsigned long long>(version));
    const QJsonObject o = takeJson(rv_fm_rows_json(fm)).object();
    FileRows r;
    r.statusKey = o.value("status_key").toString();
    r.statusArg = o.value("status_arg").toString();
    r.anyLocked = o.value("any_locked").toBool();
    for (const QJsonValue &v : o.value("rows").toArray()) r.rows.append(rowFrom(v.toObject()));
    rows_ = std::move(r);
    emit changed();
}

Group FileManager::item(int index) const {
    RV_TRACE_SCOPE("FileManager::item");
    Group g;
    const QJsonObject o = takeJson(rv_fm_item_json(fm, index)).object();
    if (o.isEmpty()) return g;
    g.name = o.value("name").toString();
    g.finish = o.value("finish").toDouble();
    g.duration = o.value("duration").toDouble();
    for (const QJsonValue &v : o.value("files").toArray()) {
        const QJsonObject f = v.toObject();
        GroupFile x;
        x.path = f.value("path").toString();
        x.face = f.value("face").toString();
        x.finish = f.value("finish").toDouble();
        x.duration = f.value("duration").toDouble();
        x.pStart = f.value("p_start").toDouble();
        x.pFinish = f.value("p_finish").toDouble();
        x.durationState = f.value("duration_state").toInt();
        x.locked = f.value("lock").toInt() != 0;
        x.width = f.value("width").toInt();
        x.height = f.value("height").toInt();
        x.gop = f.value("gop").toInt();
        x.fps = f.value("fps").toDouble();
        x.size = qint64(f.value("size").toDouble());
        x.durationMs = qint64(f.value("duration_ms").toDouble());
        x.fsCreation = f.value("fs_creation").toDouble();
        x.fsWrite = f.value("fs_write").toDouble();
        x.infoPath = f.value("info_path").toString();
        x.gpsState = f.value("gps_state").toInt();
        x.gpsLogPath = f.value("gps_log_path").toString();
        g.files.append(x);
    }
    return g;
}

void FileManager::remove(const QList<int> &rows) {
    const auto v = indices(rows);
    rv_fm_remove(fm, v.data(), int32_t(v.size()));
    refresh();
}

void FileManager::setLock(const QList<int> &rows, bool lock) {
    const auto v = indices(rows);
    rv_fm_set_lock(fm, v.data(), int32_t(v.size()), lock ? 1 : 0);
    refresh();
}

void FileManager::group(const QList<int> &rows) {
    const auto v = indices(rows);
    rv_fm_group(fm, v.data(), int32_t(v.size()));
    refresh();
}

void FileManager::ungroup(const QList<int> &rows) {
    const auto v = indices(rows);
    rv_fm_ungroup(fm, v.data(), int32_t(v.size()));
    refresh();
}

void FileManager::ungroupAll() {
    rv_fm_ungroup_all(fm);
    refresh();
}

void FileManager::groupAuto() {
    rv_fm_group_auto(fm);
    refresh();
}

QStringList FileManager::logFiles() const {
    QStringList out;
    for (const QJsonValue &v : takeJson(rv_fm_log_files_json(fm)).array()) out.append(v.toString());
    return out;
}

void FileManager::shiftVideo(int row, double days) {
    rv_fm_shift_video(fm, row, days);
    refresh();
}

bool FileManager::setTrack(int row, const QString &path) {
    const bool ok = rv_fm_set_track(fm, row, QDir::toNativeSeparators(path).toUtf8().constData()) != 0;
    refresh();
    return ok;
}
