// The Fast repair engine (header rebuild of an unfinished recording, in place), run in the
// background. Spec: shell-settings-forms.md SF-REPAIR-010/011/013/014/017.
#pragma once

#include <QString>

#include <memory>

namespace repair {

struct Status {
    int percent = 0;
    qint64 frames = 0;
    bool finished = false;
    QString error;
};

class Job {
public:
    virtual ~Job() = default;
    virtual Status status() const = 0;
    virtual void cancel() = 0;
};

// Starts repairing `broken` with `reference` (a good file from the same camera); null with
// `error` set when it cannot start.
std::unique_ptr<Job> start(const QString &broken, const QString &reference, QString *error);

// SF-REPAIR-010: a size that is an exact multiple of 32 MiB hints at a file system that did
// not record the real size.
inline bool suspiciousSize(qint64 bytes) {
    return bytes > 0 && bytes % (32ll * 1024 * 1024) == 0;
}

// SF-REPAIR-011.
bool encrypted(const QString &path);

bool available();

}  // namespace repair
