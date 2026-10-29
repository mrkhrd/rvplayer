// View → Coordinates and the coordinate commands of TFormMain: flips and shifts applied to the
// loaded tracks, speed from coordinates, the marker command (M) and copying the point (C).
// Spec: graphs-map-layout.md GM-101, GM-104, GM-180…GM-186.

#include "MainForm.h"

#include "core/Format.h"
#include "core/Playback.h"
#include "core/Settings.h"
#include "i18n/Lang.h"

#include <QAction>
#include <QApplication>
#include <QClipboard>
#include <QDesktopServices>
#include <QInputDialog>
#include <QLocale>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QProcess>
#include <QRegularExpression>
#include <QTimer>
#include <QToolButton>
#include <QUrl>

#include <cmath>

#ifdef Q_OS_WIN
#define NOMINMAX
#include <windows.h>
#endif

namespace {

void beep() {
#ifdef Q_OS_WIN
    MessageBeep(MB_OK);
#else
    QApplication::beep();
#endif
}

const char *const kVariablesHelp =
    "{x}, {y}, {h} - coordinates\n{z} - map zoom\n{s} - speed\n{a} - direction\n{p} - track position\n"
    "{date}, {ye}, {mo}, {da} - date\n{time}, {ho}, {mi}, {se} - time";

// Degrees typed with "," or ".".
double parseDegrees(QString s) {
    return s.replace(',', '.').trimmed().toDouble();
}

}  // namespace

// GM-101 and GM-104: the corrections the next track load applies.
MovingInfo::Options MainForm::trackOptions() const {
    MovingInfo::Options o;
    Settings &s = Settings::instance();
    o.flipX = isChecked("N_View_Coord_FlipX");
    o.flipY = isChecked("N_View_Coord_FlipY");
    o.shiftX = parseDegrees(s.readString("gm_shiftX", "0"));
    o.shiftY = parseDegrees(s.readString("gm_shiftY", "0"));
    o.speedFromCoordinates = isChecked("N_View_Coord_Speed");
    return o;
}

// The current row again, so new corrections apply.
void MainForm::reloadCurrentItem() {
    stopPlayback(false);
    ListViewSelectItem(nullptr);
}

// GM-180 (FlipX, FlipY and Coord_Speed).
void MainForm::N_View_Coord_FlipXClick(vcl::Event *e) {
    auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (!a) return;
    menuItemClick(a);
    reloadCurrentItem();
}

// GM-181.
void MainForm::N_View_Coord_ShiftXClick(vcl::Event *e) {
    auto *a = e ? qobject_cast<QAction *>(e->sender) : nullptr;
    if (!a) return;
    const QString key = a->objectName() == "N_View_Coord_ShiftY" ? "gm_shiftY" : "gm_shiftX";
    Settings &s = Settings::instance();
    bool ok = false;
    const QString current = QString::number(parseDegrees(s.readString(key, "0")), 'f', 6);
    const QString value = QInputDialog::getText(this, "Fix Coordinates", a->text().remove('&'), QLineEdit::Normal, current, &ok);
    if (!ok) return;
    s.writeString(key, QString::number(parseDegrees(value), 'f', 6));
    reloadCurrentItem();
}

// GM-182: placeholders replaced by the values at the current instant.
QString MainForm::fillCoordinatesTemplate(QString t, const MovingValue &v) const {
    const Group &g = playback->group();
    const QDateTime when = fmt::toQDateTime(g.timeAt(lastPosition) + tzShift);
    const bool british = isChecked("N_View_Measu_British");
    auto r = [&](const char *key, const QString &value) {
        t.replace(QString::fromLatin1(key), value, Qt::CaseInsensitive);
    };
    auto two = [](int n) { return QStringLiteral("%1").arg(n, 2, 10, QChar('0')); };
    r("{ye}", QString::number(when.date().year()));
    r("{mo}", two(when.date().month()));
    r("{da}", two(when.date().day()));
    r("{ho}", two(when.time().hour()));
    r("{mi}", two(when.time().minute()));
    r("{se}", two(when.time().second()));
    r("{ms}", QStringLiteral("%1").arg(when.time().msec(), 3, 10, QChar('0')));
    r("{date}", QLocale::system().toString(when.date(), QLocale::ShortFormat));
    r("{time}", QLocale::system().toString(when.time(), "HH:mm:ss"));
    r("{x}", QString::number(v.lon, 'f', 6));
    r("{y}", QString::number(v.lat, 'f', 6));
    r("{a}", QString::number(std::lround(std::max(0.0, v.course))));
    r("{s}", QString::number(std::lround(british ? v.speed / 1.609344 : v.speed)));
    r("{p}", british ? QString::number(v.track / 1000 / 1.609344, 'f', 3) : QString::number(std::lround(v.track)));
    r("{h}", QString::number(std::lround(british ? v.alt / 0.3048 : v.alt)));
    r("{z}", QString::number(viewport.zoom));
    t.replace("\\r", "\r\n");
    t.replace("\\n", "\r\n");
    return t;
}

// GM-183.
void MainForm::N_View_MarkerURLSetupClick(vcl::Event *) {
    Settings &s = Settings::instance();
    const QString help = QStringLiteral(
        "Examples:\n  notepad.exe \"{z}_{x}_{y}.txt\"\n  http://www.google.com/maps/@{y},{x},{z}z\n"
        "Don't open, only download:\n  @http://localhost:8111/add_node?lon={x}&&lat={y}\n\n") + kVariablesHelp;
    bool ok = false;
    const QString value = QInputDialog::getText(this, Lang::instance().tr("Mes_MarkerURLSetupTitle"), help, QLineEdit::Normal,
                                                s.readString("markerURL", "http://www.openstreetmap.org/?mlat={y}&mlon={x}&zoom={z}"), &ok);
    if (ok) s.writeString("markerURL", value);
}

// GM-184 (M).
void MainForm::N_View_MarkerURLOpenClick(vcl::Event *) {
    beep();
    const Group &g = playback->group();
    const MovingValue v = g.isEmpty() ? MovingValue() : moving.at(g.timeAt(currentPosition()));
    if (!v.gps) {
        QTimer::singleShot(200, this, [] { beep(); });
        return;
    }
    if (auto *play = find<QToolButton>("ButtonPlay"); play && play->isChecked()) {
        play->setChecked(false);
        ButtonPlayClick(nullptr);
    }
    const QString cmd = fillCoordinatesTemplate(
        Settings::instance().readString("markerURL", "http://www.openstreetmap.org/?mlat={y}&mlon={x}&zoom={z}"), v);
    if (cmd.startsWith("@http", Qt::CaseInsensitive)) {
        // A silent request: the answer is not used.
        auto *net = new QNetworkAccessManager(this);
        QNetworkRequest request(QUrl(cmd.mid(1)));
        request.setTransferTimeout(5000);
        QNetworkReply *reply = net->get(request);
        connect(reply, &QNetworkReply::finished, net, [net, reply] {
            reply->deleteLater();
            net->deleteLater();
        });
    } else if (cmd.startsWith("http", Qt::CaseInsensitive)) {
        QDesktopServices::openUrl(QUrl(cmd));
    } else {
        // The program is the leading quoted string, or the text up to the first space.
        QString program, args;
        if (cmd.startsWith('"')) {
            const qsizetype end = cmd.indexOf('"', 1);
            program = cmd.mid(1, end < 0 ? -1 : end - 1);
            args = end < 0 ? QString() : cmd.mid(end + 1).trimmed();
        } else {
            program = cmd.section(' ', 0, 0);
            args = cmd.section(' ', 1).trimmed();
        }
        QProcess p;
        p.setProgram(program);
        p.setNativeArguments(args);
        p.startDetached();
    }
}

// GM-185.
QString MainForm::copyTemplateDefault() const {
    return isChecked("N_View_Measu_British")
               ? QStringLiteral("{y}, {x}\\n{date} {time}\\nspeed: {s}mph, direction: {a}°, track: {p}mile")
               : QStringLiteral("{y}, {x}\\n{date} {time}\\nspeed: {s}km/h, direction: {a}°, track: {p}m");
}

void MainForm::N_View_Coord_CopySetupClick(vcl::Event *) {
    Settings &s = Settings::instance();
    bool ok = false;
    const QString value = QInputDialog::getText(this, Lang::instance().tr("Mes_CopyCoordSetupTitle"),
                                                QString(kVariablesHelp) + "\n \\n - new line", QLineEdit::Normal,
                                                s.readString("coordCopyTemplate", copyTemplateDefault()), &ok);
    if (ok) s.writeString("coordCopyTemplate", value);
}

// GM-186 (C).
void MainForm::N_View_Coord_CopyClick(vcl::Event *) {
    beep();
    const Group &g = playback->group();
    const MovingValue v = g.isEmpty() ? MovingValue() : moving.at(g.timeAt(currentPosition()));
    if (!v.gps) {
        QTimer::singleShot(200, this, [] { beep(); });
        return;
    }
    QApplication::clipboard()->setText(
        fillCoordinatesTemplate(Settings::instance().readString("coordCopyTemplate", copyTemplateDefault()), v));
}
