#include "FormCropping.h"

#include "core/Export.h"
#include "core/Format.h"
#include "core/GpsFiles.h"
#include "core/Settings.h"
#include "forms/FormSetupSubtitleTemplate.h"
#include "i18n/Lang.h"
#include "rv_ffi.h"
#include "vcl/AlignLayout.h"

#include <QAbstractButton>
#include <QAction>
#include <QApplication>
#include <QCheckBox>
#include <QDir>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFileDialog>
#include <QFile>
#include <QFileInfo>
#include <QGroupBox>
#include <QHeaderView>
#include <QInputDialog>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QMessageBox>
#include <QMimeData>
#include <QPushButton>
#include <QToolButton>
#include <QTreeWidget>

#include <cmath>

namespace {

const char *const kDashcamFilter =
    "DashCam Files (*.mov *.mp4 *.mp4v);;"
    "Video Files (*.3gp *.asf *.avi *.mkv *.mov *.mp4 *.mpg *.mpeg *.ts *.wmv);;"
    "Any Files (*.*)";

// filterFileName: characters Windows does not allow in names.
QString filterFileName(QString s) {
    static const QString bad = QStringLiteral("*?\"<>|");
    for (QChar c : bad) s.remove(c);
    return s;
}

QString hms(double seconds) {
    return durationText(seconds);
}

}  // namespace

FormCropping::FormCropping(QWidget *parent) : vcl::Form(parent) {
    setWindowFlag(Qt::Window);
    setAcceptDrops(true);  // SF-CROPPING-009
}

QTreeWidget *FormCropping::list() const {
    return find<QTreeWidget>("ListView");
}

// SF-CROPPING-001/002.
void FormCropping::FormCreate(vcl::Event *) {
    const Lang &lang = Lang::instance();
    setWindowIcon(QIcon(":/original/images/ICON_CROPPING.ICO"));
    setWindowTitle(lang.tr("Cropping_Title"));
    if (QTreeWidget *l = list()) {
        l->setHeaderLabels({lang.tr("File"), lang.tr("Started") + " - " + lang.tr("Finished"), lang.tr("Speed"), lang.tr("Duration")});
        l->header()->setSectionResizeMode(0, QHeaderView::Stretch);
        l->header()->setStretchLastSection(false);
        l->header()->setTextElideMode(Qt::ElideRight);
        l->setColumnWidth(1, 110);
        l->setColumnWidth(2, 70);
        l->setColumnWidth(3, 65);
        for (int c = 1; c < 4; ++c) l->headerItem()->setTextAlignment(c, Qt::AlignRight | Qt::AlignVCenter);
        connect(l, &QTreeWidget::itemDoubleClicked, this, [this] { N_EditClick(nullptr); });
    }
    // Translations of the pop-up and the controls.
    translateMenus();
    if (QAction *speed = action("N_Speed")) speed->setText(lang.tr("Speed"));
    auto text = [&](const char *name, const QString &t) {
        if (auto *b = find<QAbstractButton>(name)) b->setText(t);
        else if (auto *g = find<QGroupBox>(name)) g->setTitle(t);
    };
    text("GroupBoxVideoFileTracks", lang.tr("Cropping_FileTracks"));
    text("GroupBoxAdditionalFiles", lang.tr("Cropping_AddonFiles"));
    text("CheckBoxVideo", lang.tr("Video"));
    text("CheckBoxAudio", lang.tr("Audio"));
    text("CheckBoxMeta", lang.tr("MetaInfo") + " (GPS, G-Sensor)");
    text("CheckBoxSRTSubtitles", "(.srt) - " + lang.tr("Subtitles"));
    text("ButtonSubtitle", lang.tr("SetupSubtitle_Title"));
    text("ButtonStart", lang.tr("Cropping_BtnStart"));
    text("ButtonCancel", lang.tr("Cancel"));
    // Pop-up icons; Speed and Remove Intersections use the Edit one.
    for (const char *name : {"N_Edit", "N_Up", "N_Down", "N_FileList_Add", "N_FileList_Del", "N_FileList_Clear"}) {
        if (QAction *a = action(name)) a->setIcon(QIcon(resourceBitmap(QString(name).toUpper() + ".BMP")));
    }
    for (const char *name : {"N_Speed", "N_NoIntersection"}) {
        if (QAction *a = action(name)) a->setIcon(QIcon(resourceBitmap("N_EDIT.BMP")));
    }
    // The side buttons act as their pop-up items.
    for (const auto &[button, item] : {std::pair{"ButtonAdd", "N_FileList_Add"}, std::pair{"ButtonDel", "N_FileList_Del"},
                                       std::pair{"ButtonClear", "N_FileList_Clear"}, std::pair{"ButtonUp", "N_Up"},
                                       std::pair{"ButtonDown", "N_Down"}, std::pair{"ButtonEdit", "N_Edit"}}) {
        auto *b = find<QToolButton>(button);
        QAction *a = action(item);
        if (!b || !a) continue;
        b->setText(QString());
        b->setIcon(a->icon());
        b->setToolButtonStyle(Qt::ToolButtonIconOnly);
        b->setToolTip(a->text());
        connect(b, &QToolButton::clicked, this, [this, a] {
            a->trigger();
            if (QTreeWidget *l = list()) l->setFocus();
        });
    }
    if (auto *m = find<QCheckBox>("CheckBoxVideo")) m->setChecked(true);
    if (auto *m = find<QCheckBox>("CheckBoxAudio")) m->setChecked(true);
    if (auto *m = find<QCheckBox>("CheckBoxMeta")) m->setChecked(true);
    FormClose(nullptr);  // starts empty
}

// SF-CROPPING-003.
void FormCropping::FormShow(vcl::Event *) {
    drawProgress(0, 0);
    calcTotal();
    if (auto *l = find<QLabel>("LabelCurrentAction")) l->clear();
}

// SF-CROPPING-004.
void FormCropping::FormClose(vcl::Event *) {
    auto *start = find<QPushButton>("ButtonStart");
    if (!running && (!start || start->isEnabled() || items.isEmpty())) {
        items.clear();
        if (auto *e = find<QLineEdit>("EditFileName")) e->clear();
    }
    currentRow = -1;
    progress = 0;
    refreshList();
    calcTotal();
    setEnables(true);
}

// SF-CROPPING-005.
void FormCropping::FormResize(vcl::Event *) {
    QWidget *panel = find<QWidget>("PanelBottom");
    if (!panel) return;
    vcl::AlignLayout *l = layoutOf(panel);
    const int w = panel->width();
    auto rect = [&](const char *n) { return find<QWidget>(n) ? l->controlRect(find<QWidget>(n)) : QRect(); };
    auto set = [&](const char *n, QRect r) {
        if (QWidget *x = find<QWidget>(n)) l->setControlRect(x, r);
    };
    QRect start = rect("ButtonStart"), cancel = rect("ButtonCancel");
    const int pair = start.width() + 10 + cancel.width();
    start.moveLeft((w - pair) / 2);
    cancel.moveLeft(start.right() + 1 + 10);
    set("ButtonStart", start);
    set("ButtonCancel", cancel);
    QRect sel = rect("SpeedButtonSelectFile");
    sel.moveLeft(w - sel.width());
    set("SpeedButtonSelectFile", sel);
    QRect edit = rect("EditFileName");
    edit.setLeft(0);
    edit.setRight(sel.left() - 4);
    set("EditFileName", edit);
    QRect tracks = rect("GroupBoxVideoFileTracks");
    tracks.setWidth(w / 2);
    set("GroupBoxVideoFileTracks", tracks);
    QRect addon = rect("GroupBoxAdditionalFiles");
    addon.setLeft(tracks.right() + 1 + 16);
    addon.setRight(w - 1);
    set("GroupBoxAdditionalFiles", addon);
    QRect sub = rect("ButtonSubtitle");
    sub.moveLeft(addon.left());
    sub.setWidth(std::min(300, addon.width()));
    set("ButtonSubtitle", sub);
}

// SF-CROPPING-006.
void FormCropping::setEnables(bool b) {
    for (const char *n : {"EditFileName", "ButtonStart", "CheckBoxVideo", "CheckBoxAudio", "CheckBoxMeta", "CheckBoxSRTSubtitles",
                          "SpeedButtonSelectFile", "ButtonAdd", "ButtonDel", "ButtonClear", "ButtonUp", "ButtonDown", "ButtonEdit",
                          "ButtonSubtitle"}) {
        if (QWidget *w = find<QWidget>(n)) w->setEnabled(b);
    }
    if (QTreeWidget *l = list()) l->setContextMenuPolicy(b ? Qt::CustomContextMenu : Qt::NoContextMenu);
    if (b) calcTotal();
}

// SF-CROPPING-013.
void FormCropping::refreshList() {
    QTreeWidget *l = list();
    if (!l) return;
    const QList<int> selected = selectedRows();
    l->clear();
    for (int i = 0; i < items.size(); ++i) {
        const CropItem &c = items[i];
        auto clock = [&](double p) {
            return fmt::toQDateTime(c.start + p * c.durationMs / 86400000.0 + tzShift).time().toString("H:mm:ss");
        };
        QString speed;
        if (c.speed < 1) speed = QStringLiteral("1/%1x").arg(std::lround(1 / c.speed));
        else if (c.speed == 1) speed = QStringLiteral("1x");
        else speed = QString(c.speedUpFps ? "(fps) " : "(key) ") + QString::number(std::lround(c.speed)) + "x";
        const QString duration = i == currentRow ? QStringLiteral("%1%").arg(std::lround(progress * 100)) : hms(c.outputSeconds());
        auto *item = new QTreeWidgetItem(l, {QDir::toNativeSeparators(c.path), clock(c.p1) + " - " + clock(c.p2), speed, duration});
        for (int col = 1; col < 4; ++col) item->setTextAlignment(col, Qt::AlignRight | Qt::AlignVCenter);
        item->setSelected(selected.contains(i));
    }
}

QList<int> FormCropping::selectedRows() const {
    QList<int> out;
    if (QTreeWidget *l = list()) {
        for (QTreeWidgetItem *i : l->selectedItems()) out.append(l->indexOfTopLevelItem(i));
    }
    std::sort(out.begin(), out.end());
    return out;
}

void FormCropping::selectRow(int row) {
    QTreeWidget *l = list();
    if (!l || row < 0 || row >= l->topLevelItemCount()) return;
    l->clearSelection();
    l->setCurrentItem(l->topLevelItem(row));
    l->topLevelItem(row)->setSelected(true);
    l->scrollToItem(l->topLevelItem(row));
}

// SF-CROPPING-012.
void FormCropping::calcTotal() {
    double total = 0;
    for (const CropItem &c : std::as_const(items)) total += c.outputSeconds();
    if (auto *l = find<QLabel>("LabelTimePos")) l->setText(QStringLiteral("(%1) %2").arg(items.size()).arg(hms(total)));
    if (auto *b = find<QPushButton>("ButtonStart"); b && !running) b->setEnabled(!items.isEmpty());
}

// SF-CROPPING-010/011.
bool FormCropping::addFile(const QString &path, double p1, double p2, TDateTime start, const QString &trackSource, double fps,
                           int gop) {
    const Lang &lang = Lang::instance();
    char *raw = rv_file_info_json(QDir::toNativeSeparators(path).toUtf8().constData());
    const QJsonObject info = QJsonDocument::fromJson(raw).object();
    rv_string_free(raw);
    const double duration = info.value("duration").toDouble();
    const int width = info.value("width").toInt(), height = info.value("height").toInt();
    const QString caption = windowTitle() + ". " + (action("N_FileList_Add") ? action("N_FileList_Add")->text() : QString());
    if (duration <= 0 || width <= 0) {
        QMessageBox::warning(this, caption, QDir::toNativeSeparators(path) + "\n\n" + lang.tr("Cropping_FormatUnknown"));
        if (auto *e = find<QLineEdit>("EditFileName")) e->clear();
        return false;
    }
    CropItem c;
    c.path = path;
    c.durationMs = qint64(std::lround(duration * 1000));
    c.p1 = p1;
    c.p2 = p2;
    c.start = start > 1.0 ? start
                          : fmt::fromQDateTime(QFileInfo(path).lastModified()) - duration * kSecond;
    c.frameSize = QSize(width, height);
    c.trackSource = trackSource;
    if (fps > 0) c.fps = fps;
    c.gop = gop;
    items.append(c);
    for (int i = 0; i + 1 < items.size(); ++i) {
        if (items[i].frameSize == c.frameSize) continue;
        QMessageBox::warning(this, caption,
                             QStringLiteral("%1  %2x%3\n%4  %5x%6\n\n%7")
                                 .arg(QDir::toNativeSeparators(items[i].path))
                                 .arg(items[i].frameSize.width())
                                 .arg(items[i].frameSize.height())
                                 .arg(QDir::toNativeSeparators(path))
                                 .arg(width)
                                 .arg(height)
                                 .arg(lang.tr("Cropping_FrameNotEqual")));
        if (auto *e = find<QLineEdit>("EditFileName")) e->clear();
        break;
    }
    refreshList();
    selectRow(int(items.size()) - 1);
    calcTotal();
    return true;
}

void FormCropping::proposeFileName(const QString &path) {
    if (auto *e = find<QLineEdit>("EditFileName")) e->setText(filterFileName(QDir::toNativeSeparators(path)));
}

bool FormCropping::hasFileName() const {
    auto *e = find<QLineEdit>("EditFileName");
    return e && !e->text().trimmed().isEmpty();
}

// SF-CROPPING-007.
void FormCropping::SpeedButtonSelectFileClick(vcl::Event *) {
    auto *e = find<QLineEdit>("EditFileName");
    const QString path = QFileDialog::getSaveFileName(this, QString(), e ? filterFileName(e->text()) : QString(),
                                                      QStringLiteral("MP4 and MOV files (*.mp4 *.mov);;Any Files (*.*)"));
    if (!path.isEmpty() && e) e->setText(QDir::toNativeSeparators(path));
}

// SF-CROPPING-008.
void FormCropping::N_FileList_AddClick(vcl::Event *) {
    const QStringList files = QFileDialog::getOpenFileNames(this, action("N_FileList_Add")->text(), QString(), kDashcamFilter);
    if (QTreeWidget *l = list()) l->clearSelection();
    for (const QString &f : files) addFile(f);
}

void FormCropping::dragEnterEvent(QDragEnterEvent *e) {
    if (e->mimeData()->hasUrls() && !running) e->acceptProposedAction();
}

// SF-CROPPING-009.
void FormCropping::dropEvent(QDropEvent *e) {
    if (QTreeWidget *l = list()) l->clearSelection();
    for (const QUrl &u : e->mimeData()->urls()) {
        if (u.isLocalFile() && u.toLocalFile().size() < 255) addFile(u.toLocalFile());
    }
    if (QTreeWidget *l = list()) l->setFocus();
}

// SF-CROPPING-014.
void FormCropping::N_FileList_DelClick(vcl::Event *) {
    const QList<int> rows = selectedRows();
    for (int i = int(rows.size()) - 1; i >= 0; --i) items.removeAt(rows[i]);
    refreshList();
    calcTotal();
}

// SF-CROPPING-015.
void FormCropping::N_FileList_ClearClick(vcl::Event *) {
    items.clear();
    refreshList();
    calcTotal();
}

// SF-CROPPING-016: a selected row moves past an unselected neighbour, with its selection.
void FormCropping::moveSelected(int step) {
    QVector<bool> sel(items.size(), false);
    for (int r : selectedRows()) sel[r] = true;
    const int n = int(items.size());
    if (step < 0) {
        for (int i = 1; i < n; ++i) {
            if (sel[i] && !sel[i - 1]) {
                items.swapItemsAt(i, i - 1);
                std::swap(sel[i], sel[i - 1]);
            }
        }
    } else {
        for (int i = n - 2; i >= 0; --i) {
            if (sel[i] && !sel[i + 1]) {
                items.swapItemsAt(i, i + 1);
                std::swap(sel[i], sel[i + 1]);
            }
        }
    }
    refreshList();
    if (QTreeWidget *l = list()) {
        l->clearSelection();
        for (int i = 0; i < n; ++i) l->topLevelItem(i)->setSelected(sel[i]);
    }
}

void FormCropping::N_UpClick(vcl::Event *) {
    moveSelected(-1);
}

void FormCropping::N_DownClick(vcl::Event *) {
    moveSelected(1);
}

// SF-CROPPING-017.
void FormCropping::N_EditClick(vcl::Event *) {
    const QList<int> rows = selectedRows();
    if (rows.isEmpty() || running) return;
    FormCroppingItem editor(this);
    if (!editor.load(":/original/forms/TFORMCROPPINGITEM.dfm")) return;
    editor.edit(items[rows.first()], tzShift);
    refreshList();
    calcTotal();
}

// SF-CROPPING-018.
void FormCropping::N_SpeedClick(vcl::Event *) {
    const QList<int> rows = selectedRows();
    if (rows.isEmpty()) return;
    const Lang &lang = Lang::instance();
    bool ok = false;
    const QString text = QInputDialog::getText(this, lang.tr("N_Edit"), lang.tr("Speed"), QLineEdit::Normal,
                                               FormCroppingItem::speedText(items[rows.first()].speed), &ok);
    if (!ok) return;
    double speed;
    if (text.startsWith("1/")) {
        const int n = text.mid(2).toInt();
        speed = 1.0 / std::max(1, n > 0 ? n : 1);
    } else {
        const int n = text.toInt();
        speed = std::max(1, n > 0 ? n : 1);
    }
    for (int r : rows) {
        items[r].speed = speed;
        items[r].speedUpFps = false;
    }
    refreshList();
    calcTotal();
}

// SF-CROPPING-019.
void FormCropping::N_NoIntersectionClick(vcl::Event *) {
    for (int i = int(items.size()) - 2; i >= 0; --i) {
        CropItem &c = items[i];
        const CropItem &next = items[i + 1];
        const double dur = std::max<double>(1, c.durationMs) / 86400000.0;
        const TDateTime nextStart = next.start + next.p1 * next.durationMs / 86400000.0;
        const TDateTime end = c.start + c.p2 * dur;
        if (nextStart < end) {
            c.p2 = (nextStart - c.start) / dur;
            if (c.p2 <= c.p1) items.removeAt(i);
        }
    }
    refreshList();
    calcTotal();
}

// SF-CROPPING-020.
void FormCropping::PopupMenuCropListPopup(vcl::Event *) {
    const bool any = !selectedRows().isEmpty();
    for (const char *n : {"N_Edit", "N_Speed", "N_Up", "N_Down", "N_FileList_Del"}) {
        if (QAction *a = action(n)) a->setEnabled(any);
    }
}

// SF-CROPPING-021: the subtitle template window, owned by this one, modeless.
void FormCropping::ButtonSubtitleClick(vcl::Event *) {
    if (!subtitleForm) {
        subtitleForm = new FormSetupSubtitleTemplate(this);
        if (!subtitleForm->load(":/original/forms/TFORMSETUPSUBTITLETEMPLATE.dfm")) {
            delete subtitleForm;
            return;
        }
    }
    subtitleForm->setSettings({tzShift, british});
    subtitleForm->show();
    subtitleForm->raise();
    subtitleForm->activateWindow();
}

// SF-CROPPING-022: hiding the window is the cancel signal for a running export.
void FormCropping::ButtonCancelClick(vcl::Event *) {
    hide();
    if (!running) close();
}

// SF-CROPPING-031.
void FormCropping::drawProgress(double seconds, double fraction) {
    progress = fraction;
    double total = 0;
    for (const CropItem &c : std::as_const(items)) total += c.outputSeconds();
    if (seconds >= 0 && running) {
        if (auto *l = find<QLabel>("LabelTimePos")) {
            l->setText(QStringLiteral("%1% - %2").arg(std::lround(std::min(1.0, total > 0 ? seconds / total : 0) * 100)).arg(hms(seconds)));
        }
    }
}

// SF-CROPPING-023…030.
void FormCropping::ButtonStartClick(vcl::Event *) {
    const Lang &lang = Lang::instance();
    auto *edit = find<QLineEdit>("EditFileName");
    const QString output = edit ? QDir::fromNativeSeparators(edit->text().trimmed()) : QString();
    if (output.isEmpty()) {
        QMessageBox::critical(this, lang.tr("Error"), lang.tr("Repair_FullOutputFile"));
        if (edit) edit->setFocus();
        return;
    }
    Settings::instance().writeString("copyFolder", QFileInfo(output).absolutePath());
    if (QFileInfo::exists(output) &&
        QMessageBox::question(this, windowTitle(), lang.tr("Mes_FileExists") + "\n" + QDir::toNativeSeparators(output) + "\n\n" +
                                                       lang.tr("Mes_SureToReplace") + " ?") != QMessageBox::Yes) {
        return;
    }
    QDir().mkpath(QFileInfo(output).absolutePath());
    if (!QFileInfo(QFileInfo(output).absolutePath()).isDir()) {
        QMessageBox::critical(this, lang.tr("Error"), lang.tr("Mes_DirectoryNotExists") + "\n" + QFileInfo(output).absolutePath());
        return;
    }
    if (!exporting::available()) {
        QMessageBox::information(this, windowTitle(), QStringLiteral("Video export is not available in this build yet."));
        return;
    }

    running = true;
    setEnables(false);
    emit exportStarted();
    exporting::Options options;
    options.video = find<QCheckBox>("CheckBoxVideo") && find<QCheckBox>("CheckBoxVideo")->isChecked();
    options.audio = find<QCheckBox>("CheckBoxAudio") && find<QCheckBox>("CheckBoxAudio")->isChecked();
    options.meta = find<QCheckBox>("CheckBoxMeta") && find<QCheckBox>("CheckBoxMeta")->isChecked();
    Settings &s = Settings::instance();
    options.trimHeadMs = s.readString("trimHeadMS", "0").toInt();
    options.trimTailMs = s.readString("trimTailMS", "0").toInt();
    exporting::Stage lastStage = exporting::Stage::Finishing;
    const exporting::Result r = exporting::run(items, options, output, [&](const exporting::Progress &p) {
        if (p.stage != lastStage) {
            lastStage = p.stage;
            static const char *keys[] = {"Cropping_Open", "Cropping_MetaInfo", "Cropping_MetaInfo", "Cropping_MetaInfo",
                                         "Cropping_Copping", "Cropping_Finish"};
            static const char *suffix[] = {"", " (1)", " (2)", " (3)", "", ""};
            if (auto *l = find<QLabel>("LabelCurrentAction")) {
                l->setText(lang.tr(keys[int(p.stage)]) + suffix[int(p.stage)] + " . . .");
            }
        }
        if (p.item != currentRow && p.item >= 0) selectRow(p.item);
        currentRow = p.item;
        drawProgress(p.outputSeconds, p.fraction);
        return isVisible();  // hiding the window cancels
    });
    running = false;
    currentRow = -1;
    // SF-CROPPING-030 ".srt": subtitles from the track inside the output, so they follow its
    // frames, beside it as "<name>.srt" with the output's times.
    if (r.ok && find<QCheckBox>("CheckBoxSRTSubtitles") && find<QCheckBox>("CheckBoxSRTSubtitles")->isChecked()) {
        const QString srt = output.left(output.lastIndexOf('.') + 1) + "srt";
        gpsfiles::write({{output, 0, 0, r.seconds}}, QStringLiteral("srt"), srt, {tzShift, british});
    }
    if (!r.problems.isEmpty()) QMessageBox::critical(this, lang.tr("Error"), r.problems.join("\n"));
    if (!r.ok && !r.cancelled && !r.error.isEmpty()) QMessageBox::critical(this, lang.tr("Error"), r.error);
    if (r.ok && r.first > 2.0 && r.last > 2.0) {
        QFile f(output);
        if (f.open(QIODevice::ReadWrite)) {
            f.setFileTime(fmt::toQDateTime(r.first), QFileDevice::FileBirthTime);
            f.setFileTime(fmt::toQDateTime(r.last), QFileDevice::FileModificationTime);
        }
    }
    if (auto *l = find<QLabel>("LabelCurrentAction")) l->clear();
    if (!isVisible()) {  // cancelled
        QFile::remove(output);
        show();
    }
    setEnables(true);
    refreshList();
    raise();
    activateWindow();
    QApplication::beep();
    emit exportFinished(r.ok ? output : QString());
}
