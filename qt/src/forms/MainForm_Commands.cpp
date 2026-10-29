// Menu commands of TFormMain that open dialogs or act on files: About, What's New, settings
// reset, snapshots, overlap time, deleting files from disk.
// Spec: shell-settings-forms.md SH-024 and §6.1–§6.2, playback-video.md PV-086,
// file-list.md FL-051…FL-053, FL-063 and FL-075.

#include "MainForm.h"

#include "core/Exif.h"
#include "core/Format.h"
#include "core/Playback.h"
#include "core/Settings.h"
#include "filelist/FileListView.h"
#include "forms/FormAbout.h"
#include "forms/FormCropping.h"
#include "forms/FormRepair.h"
#include "forms/FormWhatsNew.h"
#include "i18n/Lang.h"
#include "video/VideoView.h"

#include <QAction>
#include <QApplication>
#include <QDir>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QInputDialog>
#include <QMessageBox>
#include <QProcess>
#include <QRegularExpression>
#include <QScreen>
#include <QStandardPaths>
#include <QToolButton>

#include <cmath>

namespace {

// Opens a secondary form once and brings it back on later calls (GetFormAndCreate).
template <class F>
F *singleton(QPointer<F> &slot, const char *dfm) {
    if (!slot) {
        slot = new F;
        slot->setAttribute(Qt::WA_DeleteOnClose, false);
        if (!slot->load(QString(":/original/forms/") + dfm)) {
            delete slot;
            return nullptr;
        }
    }
    return slot;
}

// FL-053: clear the attributes, then delete; errors are ignored.
void deleteFile(const QString &path) {
    if (path.isEmpty() || !QFileInfo::exists(path)) return;
    QFile::setPermissions(path, QFile::permissions(path) | QFileDevice::WriteOwner | QFileDevice::WriteUser);
    QFile::remove(path);
}

}  // namespace

void MainForm::N_Help_AboutClick(vcl::Event *) {
    if (FormAbout *f = singleton(aboutForm, "TFORMABOUT.dfm")) {
        f->show();
        f->raise();
        f->activateWindow();
    }
}

void MainForm::N_Help_WhatsNewClick(vcl::Event *) {
    if (FormWhatsNew *f = singleton(whatsNewForm, "TFORMWHATSNEW.dfm")) {
        f->show();
        f->raise();
        f->activateWindow();
    }
}

// The brand site is gone (no brand), and the author's site has no address in the translations.
void MainForm::N_Help_Site_BrandClick(vcl::Event *) {}

void MainForm::N_Help_Site_MyClick(vcl::Event *) {
    const QString site = Lang::instance().trDef("MySite", QString());
    if (!site.isEmpty()) QProcess::startDetached("explorer.exe", {site});
}

// SH-024: confirm, wipe the settings, start again.
void MainForm::N_Settings_ClearClick(vcl::Event *) {
    const auto answer = QMessageBox::question(this, windowTitle(), Lang::instance().tr("Mes_ClearSettings") + "?\n");
    if (answer != QMessageBox::Yes) return;
    close();
    Settings::instance().clear();
    QProcess::startDetached(QApplication::applicationFilePath(), {});
    QApplication::quit();
}

// PV-086: Alt+S saves the decoded frame, Alt+W the window, as JPEG with EXIF.
void MainForm::N_Playback_Save_FrameClick(vcl::Event *e) {
    const bool window = e && e->sender && e->sender->objectName() == "N_Playback_Save_Wind";
    const double p = currentPosition();
    if (auto *play = find<QToolButton>("ButtonPlay"); play && play->isChecked()) {
        play->setChecked(false);
        ButtonPlayClick(nullptr);
    }
    QApplication::processEvents();

    QImage image;
    if (window) {
        if (isChecked("N_FullScreen")) N_FullScreenClick(nullptr);
        QApplication::processEvents();
        const QRect r = frameGeometry();
        if (QScreen *s = screen()) image = s->grabWindow(0, r.x(), r.y(), r.width(), r.height()).toImage();
    } else if (video) {
        image = video->currentFrame();
    }
    if (image.isNull()) return;

    const Group &g = playback->group();
    TDateTime dt = g.isEmpty() ? 0 : g.timeAt(p) + tzShift;
    if (dt < 2.0) dt = fmt::fromQDateTime(QDateTime::currentDateTime());
    const QDateTime when = fmt::toQDateTime(dt);
    QString name = QString(window ? "Window " : "Frame ") + when.toString("yyyy.MM.dd HH-mm-ss");
    QString videoTime;
    if (!g.isEmpty()) {
        const qint64 ms = qint64(std::lround(g.elapsedSeconds(p) * 1000));
        videoTime = QStringLiteral("%1:%2:%3.%4")
                        .arg(ms / 3600000)
                        .arg(ms / 60000 % 60, 2, 10, QChar('0'))
                        .arg(ms / 1000 % 60, 2, 10, QChar('0'))
                        .arg(ms % 1000, 3, 10, QChar('0'));
        name += " (" + QString(videoTime).replace(':', '-') + ")";
    }
    name.remove(QRegularExpression(QStringLiteral("[\\\\/:*?\"<>|]")));
    Settings &s = Settings::instance();
    const QString dir = s.readString("snapshotFolder", QStandardPaths::writableLocation(QStandardPaths::PicturesLocation));
    QString path = QFileDialog::getSaveFileName(this, QString(), QDir(dir).filePath(name + ".jpg"),
                                                QStringLiteral("Jpeg Files (*.jpg *.jpeg)"));
    if (path.isEmpty()) return;
    if (QFileInfo(path).suffix().isEmpty()) path += ".jpg";
    s.writeString("snapshotFolder", QFileInfo(path).absolutePath());

    exif::Info info;
    info.description = QString(window ? "Screenshot" : "Video frame") + " of " + windowTitle().section(' ', 0, 0) +
                       (videoTime.isEmpty() ? QString() : " " + videoTime) + " " + when.toString("yyyy-MM-dd HH:mm:ss");
    info.original = when;
    if (!g.isEmpty()) {
        const MovingValue v = moving.at(g.timeAt(p));
        if (v.gps && v.gpsGood) info.gps = exif::Info::Gps{v.lat, v.lon, v.alt};
    }
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly)) return;
    f.write(exif::jpeg(image, 96, info));
    f.setFileTime(when, QFileDevice::FileModificationTime);
    f.setFileTime(when, QFileDevice::FileBirthTime);
}

// FL-075: the overlap trim of each file's tail, in milliseconds.
void MainForm::N_View_Movie_OverlapClick(vcl::Event *) {
    stopPlayback(false);
    const Lang &lang = Lang::instance();
    Settings &s = Settings::instance();
    bool ok = false;
    const QString value = QInputDialog::getText(this, lang.tr("Mes_MovieOverlapTitle"), lang.tr("Mes_MovieOverlapText") + ":",
                                                QLineEdit::Normal, s.readString("trimTailMS", "0"), &ok);
    if (!ok) return;
    s.writeString("trimTailMS", value);  // stored as typed, like the original
    playback->setTrimTail(value.toDouble() / 1000.0);
    ListViewSelectItem(nullptr);
}

// FL-053: the files of one row, then the row itself.
void MainForm::deleteRowFromDisk(int row) {
    const Group g = fileManager->item(row);
    for (const GroupFile &f : g.files) {
        if (!f.infoPath.isEmpty() && f.infoPath != f.path) deleteFile(f.infoPath);
        deleteFile(f.path);
        if (!f.infoPath.isEmpty()) deleteFile(f.infoPath.left(f.infoPath.size() - 4) + ".srt");
    }
}

// FL-051.
void MainForm::N_FileList_DelDiskClick(vcl::Event *) {
    FileListView *list = listView();
    if (!list || !fileManager) return;
    const QList<int> rows = list->selectedRows();
    const FileRows &data = list->listModel()->fileRows();
    QStringList names;
    for (int r : rows) {
        if (isNowPlaying(r)) stopPlayback(false);
        if (r < data.rows.size()) names.append(data.rows[r].path);
    }
    if (names.isEmpty()) return;
    const Lang &lang = Lang::instance();
    const QString text = lang.tr("Mes_SureToDelFile") + "\r\n" + names.join("\r\n") + " ?\r\n\r\n" + lang.tr("Mes_NoWayToBack") + " ! ! !";
    if (QMessageBox::question(this, windowTitle(), text) == QMessageBox::Yes) {
        for (int i = int(rows.size()) - 1; i >= 0; --i) deleteRowFromDisk(rows[i]);
        lastSelectedName.clear();
        fileManager->remove(rows);
    }
    list->clearSelection();
    list->setCurrentIndex(QModelIndex());
}

// FL-052: every row without a locked file.
void MainForm::N_FileList_DelDiskAllClick(vcl::Event *) {
    FileListView *list = listView();
    if (!list || !fileManager) return;
    stopPlayback(false);
    const FileRows &data = list->listModel()->fileRows();
    QList<int> rows;
    QString listText;
    int files = 0;
    for (int r = 0; r < data.rows.size(); ++r) {
        if (data.rows[r].lock != 0) continue;
        if (rows.size() < 20) listText += "\r\n" + data.rows[r].path;
        else listText += "... ";
        rows.append(r);
        files += data.rows[r].files;
    }
    if (rows.isEmpty()) return;
    const Lang &lang = Lang::instance();
    const QString text = lang.tr("Mes_SureToDelAllFiles") + "?\r\n\r\n" + lang.tr("Mes_SureToDelAllFiles2") + ": " +
                         QString::number(files) + "\r\n" + listText + "\r\n" + lang.tr("Mes_NoWayToBack") + " ! ! !";
    QMessageBox box(QMessageBox::Warning, windowTitle(), text, QMessageBox::Yes | QMessageBox::No, this);
    if (box.exec() != QMessageBox::Yes) return;
    for (int i = int(rows.size()) - 1; i >= 0; --i) {
        deleteRowFromDisk(rows[i]);
        QApplication::processEvents();
    }
    lastSelectedName.clear();
    fileManager->remove(rows);
}

// The Cutting window: one, created on first use (FormCropping()).
FormCropping *MainForm::cropping() {
    if (!croppingForm) {
        croppingForm = new FormCropping;
        if (!croppingForm->load(":/original/forms/TFORMCROPPING.dfm")) {
            delete croppingForm;
            return nullptr;
        }
        // SF-CROPPING-024/030: the main window steps aside while exporting, then lists the result.
        connect(croppingForm, &FormCropping::exportStarted, this, [this] {
            stopPlayback(false);
            hide();
        });
        connect(croppingForm, &FormCropping::exportFinished, this, [this](const QString &output) {
            show();
            if (!output.isEmpty()) addFiles({output});
        });
    }
    croppingForm->setTimeZoneShift(tzShift);
    croppingForm->setBritish(isChecked("N_View_Measu_British"));
    return croppingForm;
}

// SF-CROPPING-035: the A-B selection of every file of the current group.
void MainForm::N_File_Save_VideoClick(vcl::Event *) {
    FormCropping *f = cropping();
    if (!f) return;
    f->showNormal();
    f->raise();
    f->activateWindow();
    const Group &g = playback->group();
    if (g.isEmpty() || selectR <= selectL) return;
    for (int k = 0; k < g.files.size(); ++k) {
        const GroupFile &file = g.files[k];
        const double p1 = g.localAt(k, selectL), p2 = g.localAt(k, selectR);
        if (p1 < 1 && p2 > 0 && p1 < p2) {
            f->addFile(file.path, std::max(0.0, p1), std::min(1.0, p2), file.start(),
                       file.infoPath.isEmpty() ? file.gpsLogPath : file.infoPath, file.fps, file.gop);
        }
    }
    if (!f->hasFileName()) {
        const QString folder = Settings::instance().readString("copyFolder", QStandardPaths::writableLocation(QStandardPaths::MoviesLocation));
        QString name = "Fragment of " + g.name;  // "Fragment of " is literal in the original too
        if (!(selectL == 0 && selectR == 1)) {
            auto clock = [&](double p) { return fmt::toQDateTime(g.timeAt(p) + tzShift).time().toString("HH-mm-ss"); };
            name += " (" + clock(selectL) + " - " + clock(selectR) + ")";
        }
        name += "." + QFileInfo(g.files.first().path).suffix();
        f->proposeFileName(QDir(folder).filePath(name));
    }
}

// SF-CROPPING-036: whole files.
void MainForm::N_FileList_SaveClick(vcl::Event *) {
    if (!playback->group().isEmpty()) {
        const double a = selectL, b = selectR;
        selectL = 0;
        selectR = 1;
        N_File_Save_VideoClick(nullptr);
        selectL = a;
        selectR = b == 1 ? 0 : b;
        FormResize(nullptr);
        return;
    }
    FormCropping *f = cropping();
    FileListView *list = listView();
    if (!f || !list || !fileManager) return;
    f->showNormal();
    f->raise();
    f->activateWindow();
    for (int r : list->selectedRows()) {
        const Group g = fileManager->item(r);
        for (const GroupFile &file : g.files) {
            f->addFile(file.path, 0, 1, file.start(), file.infoPath.isEmpty() ? file.gpsLogPath : file.infoPath, file.fps, file.gop);
        }
    }
}

// The Repair window: one, created on first use; a repaired file joins the list (SF-REPAIR-015).
FormRepair *MainForm::repairing() {
    if (!repairForm) {
        repairForm = new FormRepair;
        if (!repairForm->load(":/original/forms/TFORMREPAIR.dfm")) {
            delete repairForm;
            return nullptr;
        }
        connect(repairForm, &FormRepair::repaired, this, [this](const QString &path) { addFiles({path}); });
    }
    return repairForm;
}

// MN-011: empty fields.
void MainForm::N_File_Repair_VideoClick(vcl::Event *) {
    if (FormRepair *f = repairing()) f->open(QString(), {}, nullptr);
}

// FL-063: the current row's first file whose duration could not be read (state 2), else its
// first file; the reference is picked among all files of the list.
void MainForm::N_FileList_RepairClick(vcl::Event *) {
    FormRepair *f = repairing();
    FileListView *list = listView();
    if (!f || !list || !fileManager || !list->currentIndex().isValid()) return;
    const Group g = fileManager->item(list->currentIndex().row());
    if (g.isEmpty()) return;
    const GroupFile *broken = &g.files.first();
    for (const GroupFile &file : g.files) {
        if (file.durationState == 2) {
            broken = &file;
            break;
        }
    }
    QVector<GroupFile> all;
    for (int r = 0; r < fileManager->rows().rows.size(); ++r) all += fileManager->item(r).files;
    f->open(broken->path, all, broken);
}
