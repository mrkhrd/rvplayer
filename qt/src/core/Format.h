// Display strings of the file list and the window caption (FM: BinaryToStr).
// Spec: file-list.md §4.8 and FL-090.
#pragma once

#include "Group.h"

#include <QDateTime>
#include <QString>

namespace fmt {

// Date and time of an item start: "today 18:39", "yesterday 07:05" or "<short date> 18:39".
QString dateTime(TDateTime start, double tzShift);
// "1:13", "1:02:03", "~0:38" (estimated) or "?" (unknown).
QString duration(qint64 ms, int durationState);
// MiB with a back-quote before the last three digits ("1`234"); empty when negative.
QString size(qint64 bytes);
// "2.0km", "0.65km", "123km" (FL-090 unit appended); empty when there is no track.
QString trip(double metres, bool british);
// "102", "61.7"; empty when unknown.
QString speed(double kmh, bool british);
// "2304x1296"; empty when unknown.
QString resolution(int w, int h);

// TDateTime of a QDateTime and back (local time).
TDateTime fromQDateTime(const QDateTime &dt);
QDateTime toQDateTime(TDateTime t);

}  // namespace fmt
