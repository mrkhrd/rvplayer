// JPEG files with the few EXIF fields the original's snapshots carry (PV-086): description,
// original date and time with sub-seconds, and the GPS position.
#pragma once

#include <QByteArray>
#include <QDateTime>
#include <QImage>
#include <QString>

#include <optional>

namespace exif {

struct Info {
    QString description;
    QDateTime original;  // local time
    struct Gps {
        double lat = 0, lon = 0, alt = 0;  // degrees, metres (0 = unknown)
    };
    std::optional<Gps> gps;
};

// The image as a JPEG of the given quality with an EXIF block for `info`.
QByteArray jpeg(const QImage &image, int quality, const Info &info);

}  // namespace exif
