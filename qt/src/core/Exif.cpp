#include "Exif.h"

#include <QBuffer>
#include <QImageWriter>

#include <algorithm>
#include <cmath>
#include <vector>

namespace exif {

namespace {

// Little-endian TIFF structure: IFD entries point into a data area that follows them.
class Tiff {
public:
    struct Entry {
        quint16 tag, type;
        quint32 count;
        QByteArray data;  // the value, inline when it fits in 4 bytes
    };

    static Entry ascii(quint16 tag, const QString &s) {
        QByteArray d = s.toUtf8();
        d.append('\0');
        return {tag, 2, quint32(d.size()), d};
    }
    static Entry byte(quint16 tag, QByteArray bytes) { return {tag, 1, quint32(bytes.size()), bytes}; }
    static Entry rationals(quint16 tag, const std::vector<std::pair<quint32, quint32>> &values) {
        QByteArray d;
        for (const auto &[num, den] : values) d += u32(num) + u32(den);
        return {tag, 5, quint32(values.size()), d};
    }
    static Entry longValue(quint16 tag, quint32 v) { return {tag, 4, 1, u32(v)}; }

    static QByteArray u16(quint16 v) { return QByteArray(reinterpret_cast<const char *>(&v), 2); }
    static QByteArray u32(quint32 v) { return QByteArray(reinterpret_cast<const char *>(&v), 4); }

    // One IFD at `offset` (from the TIFF header); returns its bytes including its data area.
    static QByteArray ifd(std::vector<Entry> entries, quint32 offset) {
        std::sort(entries.begin(), entries.end(), [](const Entry &a, const Entry &b) { return a.tag < b.tag; });
        QByteArray head = u16(quint16(entries.size()));
        QByteArray data;
        const quint32 dataStart = offset + 2 + quint32(entries.size()) * 12 + 4;
        for (const Entry &e : entries) {
            head += u16(e.tag) + u16(e.type) + u32(e.count);
            if (e.data.size() <= 4) {
                head += e.data + QByteArray(4 - e.data.size(), '\0');
            } else {
                head += u32(dataStart + quint32(data.size()));
                data += e.data;
                if (data.size() % 2) data.append('\0');
            }
        }
        return head + u32(0) + data;  // no next IFD
    }
};

// Degrees as degrees/minutes/seconds rationals.
std::vector<std::pair<quint32, quint32>> dms(double v) {
    v = std::abs(v);
    const quint32 d = quint32(v);
    const double m = (v - d) * 60;
    const quint32 mi = quint32(m);
    const quint32 s100 = quint32(std::lround((m - mi) * 60 * 100));
    return {{d, 1}, {mi, 1}, {s100, 100}};
}

QByteArray exifBlock(const Info &info) {
    // Sizes are only known once built, so the sub-IFDs are laid out after IFD0 in two passes.
    const QString when = info.original.toString("yyyy:MM:dd HH:mm:ss");
    const QString sub = QString::number(info.original.time().msec()).rightJustified(3, '0');
    std::vector<Tiff::Entry> ifd0 = {Tiff::ascii(0x010E, info.description), Tiff::ascii(0x0132, when),
                                     Tiff::longValue(0x8769, 0)};
    if (info.gps) ifd0.push_back(Tiff::longValue(0x8825, 0));
    const std::vector<Tiff::Entry> exifIfd = {Tiff::ascii(0x9003, when), Tiff::ascii(0x9291, sub)};
    std::vector<Tiff::Entry> gpsIfd;
    if (info.gps) {
        const Info::Gps &g = *info.gps;
        gpsIfd = {Tiff::byte(0x0000, QByteArray("\x02\x02\x00\x00", 4)), Tiff::ascii(0x0001, g.lat < 0 ? "S" : "N"),
                  Tiff::rationals(0x0002, dms(g.lat)), Tiff::ascii(0x0003, g.lon < 0 ? "W" : "E"),
                  Tiff::rationals(0x0004, dms(g.lon))};
        if (g.alt != 0) {
            gpsIfd.push_back(Tiff::byte(0x0005, QByteArray(1, g.alt < 0 ? '\x01' : '\x00')));
            gpsIfd.push_back(Tiff::rationals(0x0006, {{quint32(std::lround(std::abs(g.alt) * 10)), 10}}));
        }
    }
    const quint32 ifd0Offset = 8;
    QByteArray first = Tiff::ifd(ifd0, ifd0Offset);
    const quint32 exifOffset = ifd0Offset + quint32(first.size());
    const QByteArray exifBytes = Tiff::ifd(exifIfd, exifOffset);
    const quint32 gpsOffset = exifOffset + quint32(exifBytes.size());
    for (Tiff::Entry &e : ifd0) {
        if (e.tag == 0x8769) e.data = Tiff::u32(exifOffset);
        if (e.tag == 0x8825) e.data = Tiff::u32(gpsOffset);
    }
    first = Tiff::ifd(ifd0, ifd0Offset);
    QByteArray tiff = QByteArray("II*\0", 4) + Tiff::u32(ifd0Offset) + first + exifBytes;
    if (info.gps) tiff += Tiff::ifd(gpsIfd, gpsOffset);
    return QByteArray("Exif\0\0", 6) + tiff;
}

}  // namespace

QByteArray jpeg(const QImage &image, int quality, const Info &info) {
    QByteArray out;
    QBuffer buffer(&out);
    buffer.open(QIODevice::WriteOnly);
    QImageWriter writer(&buffer, "jpg");
    writer.setQuality(quality);
    if (!writer.write(image.convertToFormat(QImage::Format_RGB888))) return {};
    // APP1 goes after SOI and the JFIF APP0 segment, if any.
    const QByteArray payload = exifBlock(info);
    const int length = int(payload.size()) + 2;
    QByteArray app1("\xFF\xE1", 2);
    app1.append(char(length >> 8)).append(char(length & 0xFF)).append(payload);
    int at = 2;
    if (out.size() > 4 && quint8(out[2]) == 0xFF && quint8(out[3]) == 0xE0) {
        at = 4 + ((quint8(out[4]) << 8) | quint8(out[5]));
    }
    out.insert(at, app1);
    return out;
}

}  // namespace exif
