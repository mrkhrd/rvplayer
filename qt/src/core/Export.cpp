#include "Export.h"

#include "rv_ffi.h"

#include <QDir>
#include <QEventLoop>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTimer>

#include <algorithm>

namespace exporting {

bool available() {
    return true;
}

Result run(const QVector<CropItem> &items, const Options &options, const QString &output,
           const std::function<bool(const Progress &)> &progress) {
    QJsonArray list;
    for (const CropItem &c : items) {
        QJsonObject o{{"path", QDir::toNativeSeparators(c.path)},
                      {"p1", c.p1},
                      {"p2", c.p2},
                      {"speed", c.speed},
                      {"speed_up_fps", c.speedUpFps},
                      {"start", c.start > 2.0 ? QJsonValue(c.start) : QJsonValue()}};
        if (!c.trackSource.isEmpty()) o.insert("track", QDir::toNativeSeparators(c.trackSource));
        list.append(o);
    }
    const QJsonObject job{{"output", QDir::toNativeSeparators(output)},
                          {"video", options.video},
                          {"audio", options.audio},
                          {"meta", options.meta},
                          {"trim_head_ms", std::max(0, options.trimHeadMs)},
                          {"trim_tail_ms", std::max(0, options.trimTailMs)},
                          {"items", list}};
    Result r;
    RvExport *h = rv_export_start(QJsonDocument(job).toJson(QJsonDocument::Compact).constData());
    if (!h) {
        r.error = QStringLiteral("Cannot start the export.");
        return r;
    }
    // The UI keeps running while the core works on its own thread; progress is polled.
    QJsonObject s;
    bool cancelled = false;
    for (;;) {
        char *raw = rv_export_status_json(h);
        s = QJsonDocument::fromJson(raw).object();
        rv_string_free(raw);
        if (s.value("finished").toBool()) break;
        Progress p;
        p.stage = Stage(std::clamp(s.value("stage").toInt(), 0, int(Stage::Finishing)));
        p.item = s.value("item").toInt(-1);
        p.outputSeconds = s.value("output_seconds").toDouble();
        p.fraction = s.value("fraction").toDouble();
        if (!cancelled && !progress(p)) {
            cancelled = true;
            rv_export_cancel(h);
        }
        QEventLoop wait;
        QTimer::singleShot(30, &wait, &QEventLoop::quit);
        wait.exec();
    }
    rv_export_free(h);
    r.ok = s.value("ok").toBool();
    r.cancelled = s.value("cancelled").toBool();
    r.error = s.value("error").toString();
    for (const QJsonValue &v : s.value("problems").toArray()) r.problems.append(v.toString());
    r.first = s.value("t_min").toDouble();
    r.last = s.value("t_max").toDouble();
    r.seconds = s.value("duration").toDouble();
    return r;
}

}  // namespace exporting
