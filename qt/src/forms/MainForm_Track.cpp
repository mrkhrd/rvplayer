// GPS track commands of TFormMain: the GPS log converter, exporting the selection's track,
// opening it in Google Earth, attaching a track to a row and moving a row's videos against
// their GPS.
// Spec: file-list.md FL-064, FL-070…FL-074; shell-settings-forms.md MN-012;
// graphs-map-layout.md §3.13.

#include "MainForm.h"

#include "core/GpsFiles.h"
#include "core/Playback.h"
#include "core/Settings.h"
#include "filelist/FileListView.h"
#include "forms/FormCroppingItem.h"
#include "forms/FormGPSLog.h"
#include "i18n/Lang.h"

#include <QAction>
#include <QDesktopServices>
#include <QDir>
#include <QFileDialog>
#include <QFileInfo>
#include <QInputDialog>
#include <QMessageBox>
#include <QStandardPaths>
#include <QUrl>

// The converter: one, created on first use (FormGPSLog()).
FormGPSLog *MainForm::gpsLog() {
    if (!gpsLogForm) {
        gpsLogForm = new FormGPSLog;
        if (!gpsLogForm->load(":/original/forms/TFORMGPSLOG.dfm")) {
            delete gpsLogForm;
            return nullptr;
        }
        connect(gpsLogForm, &FormGPSLog::previewFiles, this, &MainForm::previewTrackFiles);
        connect(gpsLogForm, &FormGPSLog::logsDeleted, this, [this] {
            if (fileManager) fileManager->refresh();
        });
    }
    return gpsLogForm;
}

// MN-012 (L3969–L3971): all logs found, then playback stops.
void MainForm::N_File_Convert_GPSClick(vcl::Event *) {
    FormGPSLog *f = gpsLog();
    if (!f) return;
    f->popup(fileManager ? fileManager->logFiles() : QStringList(), this, isChecked("N_View_Measu_British"), tzShift);
    stopPlayback(false);
}

// FL-064 (hidden in this version): the logs of the selected rows ticked and selected.
void MainForm::N_FileList_Track_GPSLogClick(vcl::Event *) {
    FormGPSLog *f = gpsLog();
    FileListView *list = listView();
    if (!f || !list || !fileManager) return;
    f->popup(fileManager->logFiles(), this, isChecked("N_View_Measu_British"), tzShift);
    QStringList paths;
    for (int r : list->selectedRows()) {
        for (const GroupFile &file : fileManager->item(r).files) {
            if (!file.gpsLogPath.isEmpty()) paths.append(file.gpsLogPath);
        }
    }
    f->markOnly(paths);
}

// FL-073 (L5231–L5239).
void MainForm::N_FileList_Track_SetClick(vcl::Event *) {
    FileListView *list = listView();
    if (!list || !fileManager || !list->currentIndex().isValid()) return;
    const int row = list->currentIndex().row();
    stopPlayback(false);
    const QStringList filters = gpsfiles::openFilters();
    QString selected = filters.value(0);
    const QString path = QFileDialog::getOpenFileName(this, action("N_FileList_Track_Set")->text(), QString(), filters.join(";;"), &selected);
    if (path.isEmpty()) return;
    if (!fileManager->setTrack(row, path)) {
        QMessageBox::warning(this, windowTitle(), QDir::toNativeSeparators(path) + "\n\n" + Lang::instance().tr("Mes_NoGPSInfo"));
        return;
    }
    ListViewSelectItem(nullptr);
}

// FL-074 (L5246–L5264): "[-]H:MM:SS"; anything unreadable counts as 0 and changes nothing.
void MainForm::N_FileList_Track_MoveClick(vcl::Event *) {
    FileListView *list = listView();
    if (!list || !fileManager || !list->currentIndex().isValid()) return;
    const int row = list->currentIndex().row();
    bool ok = false;
    QString text = QInputDialog::getText(this, action("N_FileList_Track_Move")->text(), QString(), QLineEdit::Normal, durationText(0), &ok);
    if (!ok) return;
    text = text.trimmed();
    const bool minus = text.startsWith('-');
    if (minus) text.remove(0, 1);
    double seconds = parseDuration(text);
    if (minus) seconds = -seconds;
    if (seconds == 0) return;
    stopPlayback(false);
    fileManager->shiftVideo(row, seconds * kSecond);
    ListViewSelectItem(nullptr);
}

// FL-070: the item now playing, when it is the only selected row, with the GPS it plays with
// (logs included); otherwise each selected file's own GPS info. Each file contributes its own
// time window; a video's own track is read whole.
QVector<gpsfiles::Source> MainForm::selectionGps() const {
    QVector<gpsfiles::Source> out;
    FileListView *list = listView();
    if (!list || !fileManager) return out;
    const QList<int> rows = list->selectedRows();
    const bool playing = rows.size() == 1 && isNowPlaying(rows.first());
    for (int r : rows) {
        const Group g = playing ? playback->group() : fileManager->item(r);
        for (const GroupFile &f : g.files) {
            const QString source = !f.infoPath.isEmpty() ? f.infoPath : (playing ? f.gpsLogPath : QString());
            if (source.isEmpty()) continue;
            gpsfiles::Source s{source};
            if (QFileInfo(source) == QFileInfo(f.path)) {
                s.videoSeconds = f.durationMs / 1000.0;
            } else {
                s.from = f.start();
                s.to = f.finish;
            }
            out.append(s);
        }
    }
    return out;
}

// FL-071 (L3703–L3723).
void MainForm::N_FileList_Track_ExportClick(vcl::Event *) {
    const Lang &lang = Lang::instance();
    const gpsfiles::Settings settings{tzShift, isChecked("N_View_Measu_British")};
    const QVector<gpsfiles::Source> sources = selectionGps();
    const gpsfiles::Result info = gpsfiles::write(sources, QString(), QString(), settings);
    if (!info.hasPoints) {
        QMessageBox::information(this, windowTitle(), lang.tr("Mes_NoGPSInfo"));
        return;
    }
    const QVector<gpsfiles::Writer> writers = gpsfiles::writers();
    const QStringList filters = gpsfiles::saveFilters(writers);
    QString selected = filters.value(0);
    const QString folder = Settings::instance().readString("copyFolder");
    QString path = QFileDialog::getSaveFileName(this, action("N_FileList_Track_Export")->text(), QDir(folder).filePath(info.fileName),
                                                filters.join(";;"), &selected);
    if (path.isEmpty()) return;
    const int k = std::max(0, int(filters.indexOf(selected)));
    if (k >= writers.size()) return;
    // The chosen format's extension is added unless the name already ends with it.
    if (!path.endsWith("." + writers[k].ext, Qt::CaseInsensitive)) path += "." + writers[k].ext;
    const gpsfiles::Result r = gpsfiles::write(sources, writers[k].id, path, settings);
    if (!r.ok) QMessageBox::critical(this, lang.tr("Error"), QDir::toNativeSeparators(path) + "\n\n" + r.error);
}

// FL-072 (L3729–L3744): a KML in the temporary folder, opened with its program.
void MainForm::N_FileList_Track_GoogleClick(vcl::Event *) {
    const gpsfiles::Settings settings{tzShift, isChecked("N_View_Measu_British")};
    const QVector<gpsfiles::Source> sources = selectionGps();
    const gpsfiles::Result info = gpsfiles::write(sources, QString(), QString(), settings);
    if (!info.hasPoints) {
        QMessageBox::information(this, windowTitle(), Lang::instance().tr("Mes_NoGPSInfo"));
        return;
    }
    QString temp = QStandardPaths::writableLocation(QStandardPaths::TempLocation);
    if (temp.isEmpty()) temp = QStringLiteral("C:/Windows/Temp");
    const QString path = QDir(temp).filePath(info.fileName + ".kml");
    const gpsfiles::Result r = gpsfiles::write(sources, QStringLiteral("kml"), path, settings);
    if (!r.ok) {
        QMessageBox::critical(this, Lang::instance().tr("Error"), r.error);
        return;
    }
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}
