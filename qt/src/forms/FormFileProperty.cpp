#include "FormFileProperty.h"

#include "core/Format.h"
#include "draw/Bitmaps.h"
#include "i18n/Lang.h"
#include "rv_ffi.h"

#include <QAbstractButton>
#include <QApplication>
#include <QClipboard>
#include <QDir>
#include <QFileInfo>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>
#include <QRegularExpression>
#include <QTableWidget>

#include <cmath>

namespace {

const QString kIndent = QStringLiteral("    ");

QJsonObject fileInfo(const QString &path) {
    char *raw = rv_file_info_json(path.toUtf8().constData());
    const QJsonObject o = QJsonDocument::fromJson(raw).object();
    rv_string_free(raw);
    return o;
}

}  // namespace

FormFileProperty::FormFileProperty(QWidget *parent) : vcl::Form(parent) {
    setAttribute(Qt::WA_DeleteOnClose);  // SF-FILEPROPERTY-010
    setWindowFlag(Qt::Window);           // own taskbar button
}

QTableWidget *FormFileProperty::grid() const {
    return find<QTableWidget>("StringGrid");
}

// SF-FILEPROPERTY-001.
void FormFileProperty::FormCreate(vcl::Event *) {
    setWindowIcon(QIcon(":/original/images/ICON_PROPERTY.ICO"));
    if (QTableWidget *g = grid()) {
        g->setColumnCount(2);
        g->setRowCount(0);
        g->horizontalHeader()->hide();
        g->verticalHeader()->hide();
        g->verticalHeader()->setDefaultSectionSize(16);
        g->setFrameShape(QFrame::NoFrame);
        g->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        g->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);
        g->setWordWrap(false);
    }
    if (auto *copy = find<QWidget>("ButtonCopy")) {
        static_cast<QAbstractButton *>(copy)->setText(Lang::instance().tr("Property_CopyAll"));
    }
}

// SF-FILEPROPERTY-002.
void FormFileProperty::FormResize(vcl::Event *) {
    if (QTableWidget *g = grid()) {
        g->setColumnWidth(1, std::max(10, g->viewport()->width() - 1 - g->columnWidth(0)));
    }
}

void FormFileProperty::FormClose(vcl::Event *) {}

// SF-FILEPROPERTY-003: column 0 is fixed (grey), and widens to the longest label.
void FormFileProperty::add(const QString &name, const QString &value, bool translate) {
    QTableWidget *g = grid();
    if (!g) return;
    QString label = name;
    if (translate) {
        // The name may carry an indent or "[k] " prefix in front of the translation key.
        const qsizetype keyStart = name.lastIndexOf(QRegularExpression(QStringLiteral("[ \\]]"))) + 1;
        const QString key = name.mid(keyStart);
        if (!key.isEmpty()) label = name.left(keyStart) + Lang::instance().tr(key);
    }
    const int row = g->rowCount();
    g->insertRow(row);
    auto *l = new QTableWidgetItem(label);
    l->setBackground(palette().color(QPalette::Button));
    l->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable);
    g->setItem(row, 0, l);
    g->setItem(row, 1, new QTableWidgetItem(value));  // editable, so the text can be selected
    labelWidth = std::max(labelWidth, g->fontMetrics().horizontalAdvance(label) + 4 + 8);
    g->setColumnWidth(0, labelWidth);
    FormResize(nullptr);
}

QString FormFileProperty::dateTime(TDateTime t) const {
    const QDateTime dt = fmt::toQDateTime(t + tzShift);
    return QLocale::system().toString(dt.date(), QLocale::ShortFormat) + " " + dt.time().toString("HH:mm:ss");
}

// SF-FILEPROPERTY-008.
void FormFileProperty::addStartFinish(TDateTime start, TDateTime finish, int durationState, const QString &indent) {
    if (start > 0) add(indent + "Started", dateTime(start));
    if (finish > 0) add(indent + "Finished", dateTime(finish));
    if (start > 0) {
        const double secs = (finish - start) * 86400.0;
        const qint64 ms = qint64(std::lround(secs * 1000));
        QString d = draw::timeToStrSmall(secs * kSecond, true) + QStringLiteral(".%1").arg(ms % 1000, 3, 10, QChar('0'));
        if (durationState != 3) d = "~" + d;
        add(indent + "Duration", d);
    }
}

// SF-FILEPROPERTY-006/007.
void FormFileProperty::addFile(const GroupFile &f, const QString &prefix) {
    const Lang &lang = Lang::instance();
    const QString in = prefix + kIndent;
    add(in + "File", QDir::toNativeSeparators(f.path));
    // Media information lines (without names).
    if (f.width > 0) {
        QString video = QStringLiteral("Video: %1x%2").arg(f.width).arg(f.height);
        if (f.fps > 0) video += QStringLiteral(", %1 fps").arg(f.fps, 0, 'f', 2);
        if (f.durationMs > 0 && f.size > 0) video += QStringLiteral(", %1 kbps").arg(f.size * 8 / f.durationMs);
        if (f.gop > 0) video += QStringLiteral(", GOP %1").arg(f.gop);
        add(QString(), video, false);
    }
    add(in + "Size", QStringLiteral("%1 %2   (%3 %4)")
                         .arg(f.size)
                         .arg(lang.tr("byte"))
                         .arg(f.size / 1024.0 / 1024.0, 0, 'f', 1)
                         .arg(lang.tr("Mb")));
    if (f.locked) add(in + "Locked", lang.tr("Locked_true"));
    addStartFinish(f.start(), f.finish, f.durationState, in);

    if (!f.infoPath.isEmpty()) {
        add(in + "MetaInfo", QString());
        const QString in2 = in + kIndent;
        const QJsonObject info = fileInfo(f.infoPath);
        add(in2 + "File", QDir::toNativeSeparators(f.infoPath));
        if (const QString format = info.value("format").toString(); !format.isEmpty()) add(in2 + "DataFormat", format);
        else if (const QString camera = info.value("camera").toString(); !camera.isEmpty()) add(in2 + "DataFormat", camera);
        const int gps = int(info.value("gps").toArray().size());
        const int g = int(info.value("gsensor").toArray().size());
        if (gps > 0) add(in2 + "GPS " + lang.tr("points"), QString::number(gps), false);
        if (g > 0) add(in2 + "G-Sensor " + lang.tr("points"), QString::number(g), false);
        if (f.gpsState == 2) {
            const double m = info.value("distance_km").toDouble() * 1000;
            const double kmh = info.value("avg_speed_kmh").toDouble();
            if (british) {
                add(in2 + "Property_Track_Length", QStringLiteral("%1 %2").arg(m / 1000 / 1.609344, 0, 'f', 3).arg(lang.tr("Miles")));
                add(in2 + "AvgSpeed", QStringLiteral("%1 %2").arg(kmh / 1.609344, 0, 'f', 1).arg(lang.tr("MilesPerH")));
            } else {
                add(in2 + "Property_Track_Length", QStringLiteral("%1 %2").arg(std::lround(m)).arg(lang.tr("Meters")));
                add(in2 + "AvgSpeed", QStringLiteral("%1 %2").arg(kmh, 0, 'f', 1).arg(lang.tr("KmetersPerH")));
            }
        }
        // The track's own time span (camera clock of the first fix to the last).
        const QDateTime start = QDateTime::fromString(info.value("start").toString(), Qt::ISODateWithMs);
        const QJsonArray pts = info.value("gps").toArray();
        if (start.isValid() && !pts.isEmpty()) {
            const TDateTime t0 = fmt::fromQDateTime(start);
            addStartFinish(t0 + pts.first().toArray()[0].toDouble() * kSecond,
                           t0 + pts.last().toArray()[0].toDouble() * kSecond, 3, in2);
        }
    }
    if (!f.gpsLogPath.isEmpty()) {
        add(in + Lang::instance().tr("GPS Log"), QString(), false);  // a key with a space
        add(in + kIndent + "File", QDir::toNativeSeparators(f.gpsLogPath));
    }
}

// SF-FILEPROPERTY-004/005.
void FormFileProperty::popup(const FileRow &row, const Group &group, const QString &durationText, double shift, bool brit) {
    tzShift = shift;
    british = brit;
    setWindowTitle(Lang::instance().tr("Property_Title") + " " + row.name);
    add("Name", row.name);
    add("Duration", durationText);
    add("DateTime", dateTime(row.start()));
    if (group.files.size() == 1) {
        addFile(group.files.first(), QString());
    } else {
        add("Grouped", QString::number(group.files.size()));
        // The group's combined block.
        add(kIndent + "File", row.path);
        add(kIndent + "Size", QStringLiteral("%1 %2   (%3 %4)")
                                  .arg(row.size)
                                  .arg(Lang::instance().tr("byte"))
                                  .arg(row.size / 1024.0 / 1024.0, 0, 'f', 1)
                                  .arg(Lang::instance().tr("Mb")));
        if (row.lock == 1) add(kIndent + "Locked", Lang::instance().tr("Locked_true"));
        if (row.lock == 2) add(kIndent + "Locked", Lang::instance().tr("Locked_partial"));
        addStartFinish(row.start(), row.finish, row.durationState, kIndent);
        for (int k = 0; k < group.files.size(); ++k) {
            add(QString(), QString(), false);
            addFile(group.files[k], QStringLiteral("[%1] ").arg(k + 1));
        }
    }
    show();
}

// SF-FILEPROPERTY-009: TAB between cells, CRLF after each row.
void FormFileProperty::ButtonCopyClick(vcl::Event *) {
    QTableWidget *g = grid();
    if (!g) return;
    QString text;
    for (int r = 0; r < g->rowCount(); ++r) {
        const auto cell = [&](int c) { return g->item(r, c) ? g->item(r, c)->text() : QString(); };
        text += cell(0) + "\t" + cell(1) + "\r\n";
    }
    QApplication::clipboard()->setText(text);
}
