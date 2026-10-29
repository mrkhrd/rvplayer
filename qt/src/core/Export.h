// Export of video fragments (the engine behind the Cutting window), run on a worker thread.
// Spec: shell-settings-forms.md SF-CROPPING-025…030.
#pragma once

#include "forms/FormCroppingItem.h"

#include <QStringList>

#include <functional>

namespace exporting {

struct Options {
    bool video = true, audio = true, meta = true;
    int trimHeadMs = 0, trimTailMs = 0;  // FL-075 overlap trims
};

enum class Stage { Opening, Meta1, Meta2, Meta3, Copying, Finishing };

struct Progress {
    Stage stage = Stage::Opening;
    int item = -1;             // item being processed, -1 = none
    double outputSeconds = 0;  // position on the output timeline
    double fraction = 0;
};

struct Result {
    bool ok = false;
    bool cancelled = false;
    QString error;
    QStringList problems;      // "Cannot open file …" / "Different format …" (SF-CROPPING-025)
    TDateTime first = 0, last = 0;  // wall-clock span of the output (file timestamps)
    double seconds = 0;             // length of the output
};

// Runs the export; `progress` is called on the calling (UI) thread, which keeps processing
// events meanwhile. Returning false from it cancels.
Result run(const QVector<CropItem> &items, const Options &options, const QString &output,
           const std::function<bool(const Progress &)> &progress);

// Whether the export engine is present in this build.
bool available();

}  // namespace exporting
