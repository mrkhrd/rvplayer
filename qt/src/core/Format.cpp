#include "Format.h"

#include "draw/Bitmaps.h"
#include "i18n/Lang.h"

#include <QDateTime>
#include <QLocale>

#include <cmath>

namespace fmt {

namespace {

const QDate kEpoch(1899, 12, 30);

}  // namespace

TDateTime fromQDateTime(const QDateTime &dt) {
    const QDateTime local = dt.toLocalTime();
    return kEpoch.daysTo(local.date()) + local.time().msecsSinceStartOfDay() / 86400000.0;
}

QDateTime toQDateTime(TDateTime t) {
    const double day = std::floor(t);
    const int ms = int(std::lround((t - day) * 86400000.0));
    return QDateTime(kEpoch.addDays(qint64(day)), QTime(0, 0)).addMSecs(ms);
}

QString dateTime(TDateTime start, double tzShift) {
    const TDateTime t = start + tzShift;
    if (t <= 2.0) return {};
    int m = int(std::lround((t - std::floor(t)) * 1440));
    const QString time = QStringLiteral(" %1:%2").arg(m / 60, 2, 10, QChar('0')).arg(m % 60, 2, 10, QChar('0'));
    const qint64 day = qint64(std::floor(t));
    const qint64 today = kEpoch.daysTo(QDate::currentDate());
    QString date;
    if (day == today) date = Lang::instance().tr("today");
    else if (day == today - 1) date = Lang::instance().tr("yesterday");
    else date = QLocale::system().toString(kEpoch.addDays(day), QLocale::ShortFormat);
    return date + time;
}

QString duration(qint64 ms, int durationState) {
    if (ms <= 0 || durationState == 0) return QStringLiteral("?");
    const QString s = draw::timeToStrSmall(ms / 86400000.0, true);
    return durationState == 1 ? "~" + s : s;
}

QString size(qint64 bytes) {
    if (bytes < 0) return {};
    QString s = QString::number(std::llround(bytes / 1024.0 / 1024.0));
    if (s.size() > 3) s.insert(s.size() - 3, QChar('`'));
    return s;
}

QString trip(double metres, bool british) {
    double km = metres / 1000;
    if (british) km /= 1.609344;
    QString s;
    if (km > 100) s = QString::number(qint64(std::lround(km)));
    else if (km > 2) s = QString::number(km, 'f', 1);
    else if (km > 0) s = QString::number(km, 'f', 2);
    else return {};
    const Lang &l = Lang::instance();
    return s + (british ? l.tr("Miles").left(2) : l.tr("Kmeters"));
}

QString speed(double kmh, bool british) {
    double v = british ? kmh / 1.609344 : kmh;
    if (v >= 100) return QString::number(qint64(std::lround(v)));
    if (v > 0) return QString::number(v, 'f', 1);
    return {};
}

QString resolution(int w, int h) {
    if (w == 0 && h == 0) return {};
    return QStringLiteral("%1x%2").arg(w).arg(h);
}

}  // namespace fmt
