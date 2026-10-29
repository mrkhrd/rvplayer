#include "GpsFiles.h"

#include "Settings.h"
#include "i18n/Lang.h"
#include "rv_ffi.h"

#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>

namespace gpsfiles {

namespace {

QJsonDocument takeJson(char *raw) {
    const QJsonDocument d = QJsonDocument::fromJson(raw);
    rv_string_free(raw);
    return d;
}

QJsonDocument call(char *(*f)(const char *), const QJsonObject &job) {
    return takeJson(f(QJsonDocument(job).toJson(QJsonDocument::Compact).constData()));
}

// The original used the Windows regional settings and its own translations.
QJsonObject common(const Settings &s) {
    const QLocale locale = QLocale::system();
    const QString time = locale.timeFormat(QLocale::LongFormat);
    QString timeSep = QStringLiteral(":");
    for (const QChar c : time) {
        if (!c.isLetter() && c != ' ') {
            timeSep = c;
            break;
        }
    }
    QJsonObject words;
    for (const char *id : {"Altitude", "Meters", "Speed", "KmetersPerH", "Azimut", "Property_Track_Length", "Details_Track_Length",
                           "Miles", "MilesPerH"}) {
        words.insert(id, Lang::instance().tr(id));
    }
    const QString templ = ::Settings::instance().readString("subtitleTemplate");
    return {{"user_shift", s.tzShift},
            {"imperial", s.british},
            {"srt_template", templ.size() >= 2 ? QJsonValue(templ) : QJsonValue()},
            {"locale", QJsonObject{{"decimal", QString(locale.decimalPoint())},
                                   {"time_sep", timeSep},
                                   {"date_format", locale.dateFormat(QLocale::ShortFormat)},
                                   {"time_format", time}}},
            {"words", words}};
}

}  // namespace

QVector<Writer> writers() {
    QVector<Writer> out;
    for (const QJsonValue &v : takeJson(rv_gps_writers_json()).array()) {
        const QJsonObject o = v.toObject();
        out.append({o.value("id").toString(), o.value("caption").toString(), o.value("ext").toString()});
    }
    return out;
}

QStringList saveFilters(const QVector<Writer> &list) {
    QStringList parts;
    for (const Writer &w : list) parts.append(QStringLiteral("%1 (*.%2)").arg(w.caption, w.ext));
    return parts;
}

QStringList openFilters() {
    QStringList all, each;
    for (const QJsonValue &v : takeJson(rv_gps_extensions_json()).array()) {
        const QString ext = v.toString();
        all.append("*." + ext);
        each.append(QStringLiteral("%1 (*.%2)").arg(ext.toUpper(), ext));
    }
    return QStringList{QStringLiteral("GPS Logs (%1)").arg(all.join(' '))} + each + QStringList{QStringLiteral("Any Files (*.*)")};
}

Result write(const QVector<Source> &sources, const QString &writer, const QString &output, const Settings &settings) {
    QJsonObject job = common(settings);
    QJsonArray list;
    for (const Source &s : sources) {
        QJsonObject o{{"path", QDir::toNativeSeparators(s.path)}};
        if (s.to > s.from && s.from > 2.0) {
            o.insert("from", s.from);
            o.insert("to", s.to);
        }
        if (s.videoSeconds > 0) o.insert("video_duration", s.videoSeconds);
        list.append(o);
    }
    job.insert("sources", list);
    if (!writer.isEmpty()) {
        job.insert("writer", writer);
        job.insert("output", QDir::toNativeSeparators(output));
    }
    const QJsonObject o = call(rv_gps_write_json, job).object();
    Result r;
    r.ok = o.value("ok").toBool();
    r.error = o.value("error").toString();
    r.hasPoints = o.value("has_points").toBool();
    r.name = o.value("name").toString();
    r.fileName = o.value("file_name").toString();
    return r;
}

QString srtPreview(const QString &templ, const Settings &settings, TDateTime time, double lat, double lng, double alt, double speed,
                   double bearing, double trackMetres) {
    QJsonObject job = common(settings);
    job.insert("srt_template", templ);
    job.insert("time", time);
    job.insert("lat", lat);
    job.insert("lng", lng);
    job.insert("alt", alt);
    job.insert("speed", speed);
    job.insert("bearing", bearing);
    job.insert("track", trackMetres);
    return call(rv_gps_srt_preview_json, job).object().value("text").toString();
}

QString srtDefaultTemplate(const Settings &settings) {
    QJsonObject job = common(settings);
    job.insert("srt_template", QJsonValue());
    return call(rv_gps_srt_preview_json, job).object().value("default_template").toString();
}

}  // namespace gpsfiles
