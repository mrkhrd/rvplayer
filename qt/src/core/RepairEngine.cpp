#include "RepairEngine.h"

#include "rv_ffi.h"

#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>

namespace repair {

namespace {

QByteArray arg(const QString &path) {
    return QDir::toNativeSeparators(path).toUtf8();
}

// A job of the Rust engine (crates/rv-repair).
class CoreJob : public Job {
public:
    explicit CoreJob(RvRepair *job) : job(job) {}
    ~CoreJob() override { rv_repair_free(job); }

    Status status() const override {
        char *raw = rv_repair_status_json(job);
        const QJsonObject o = QJsonDocument::fromJson(raw).object();
        rv_string_free(raw);
        Status s;
        s.percent = o.value("percent").toInt();
        s.frames = qint64(o.value("frames").toDouble());
        s.finished = o.value("finished").toBool();
        s.error = o.value("error").toString();
        return s;
    }
    void cancel() override { rv_repair_cancel(job); }

private:
    RvRepair *job;
};

}  // namespace

bool available() {
    return true;
}

std::unique_ptr<Job> start(const QString &broken, const QString &reference, QString *error) {
    RvRepair *job = rv_repair_start(arg(broken).constData(), arg(reference).constData());
    if (!job) {
        if (error) *error = QStringLiteral("Cannot start the repair.");
        return nullptr;
    }
    return std::make_unique<CoreJob>(job);
}

bool encrypted(const QString &path) {
    return rv_repair_is_encrypted(arg(path).constData()) != 0;
}

}  // namespace repair
