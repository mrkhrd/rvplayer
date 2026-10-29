#include "FormGPSLog.h"

#include "core/GpsFiles.h"
#include "core/MovingInfo.h"
#include "forms/FormCroppingItem.h"
#include "i18n/Lang.h"

#include <QAction>
#include <QApplication>
#include <QCheckBox>
#include <QDir>
#include <QDragEnterEvent>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QHeaderView>
#include <QListWidget>
#include <QMessageBox>
#include <QMimeData>
#include <QProcess>
#include <QPushButton>
#include <QTimer>
#include <QToolButton>
#include <QTreeWidget>

namespace {

// The row's file is its caption (TListItem.Caption).
QString pathOf(const QTreeWidgetItem *it) {
    return it->data(0, Qt::UserRole).toString();
}

}  // namespace

FormGPSLog::FormGPSLog(QWidget *parent) : vcl::Form(parent) {
    setWindowFlag(Qt::Window);
    setAcceptDrops(true);  // FormCreate L84: DragAcceptFiles
}

QTreeWidget *FormGPSLog::list() const {
    return find<QTreeWidget>("ListViewFiles");
}

// FormCreate L83–L90 and the translation of the form (shell-settings-forms.md §4.3).
void FormGPSLog::FormCreate(vcl::Event *) {
    const Lang &lang = Lang::instance();
    setWindowIcon(QIcon(":/original/images/ICON_GPS_CONV.ICO"));
    setWindowTitle(lang.tr("GPSLog_Title"));
    if (auto *b = find<QPushButton>("ButtonConvert")) b->setText(lang.tr("GPSLog_Convert"));
    if (auto *c = find<QCheckBox>("CheckBoxAll")) c->setText(lang.tr("GPSLog_SelectAll"));
    if (auto *c = find<QCheckBox>("CheckBoxShowPath")) c->setText(lang.tr("GPSLog_ShowTrack"));
    if (auto *c = find<QCheckBox>("CheckBoxJoin")) c->setText(lang.tr("GPSLog_JoinToOneFile"));
    for (const char *n : {"N_FileList_ShowExplorer", "N_FileList_Add", "N_FileList_Del", "N_FileList_Clear", "N_FileList_DelDisk",
                          "N_FileList_DelDiskLogs"}) {
        if (QAction *a = action(n)) a->setText(lang.tr(n));
    }
    if (QTreeWidget *l = list()) {
        l->header()->setStretchLastSection(false);
        // Ticking a row redraws the preview like selecting one (OnClick).
        connect(l, &QTreeWidget::itemChanged, this, [this] { ListViewFilesClick(nullptr); });
        connect(l, &QTreeWidget::itemDoubleClicked, this, [this] { N_FileList_ShowExplorerClick(nullptr); });
    }
    statsTimer = new QTimer(this);
    connect(statsTimer, &QTimer::timeout, this, &FormGPSLog::fillNext);
}

// FormShow L101–L106 (Btn): the icon buttons run the list menu's items.
void FormGPSLog::FormShow(vcl::Event *) {
    for (const auto &[button, item] : {std::pair{"ButtonAdd", "N_FileList_Add"}, std::pair{"ButtonDel", "N_FileList_Del"},
                                       std::pair{"ButtonClear", "N_FileList_Clear"}, std::pair{"ButtonDelDisk", "N_FileList_DelDisk"}}) {
        auto *b = find<QToolButton>(button);
        QAction *a = action(item);
        if (!b || !a || b->property("bound").toBool()) continue;
        b->setText(QString());
        b->setIcon(a->icon());
        b->setToolButtonStyle(Qt::ToolButtonIconOnly);
        b->setToolTip(a->text());
        b->setProperty("bound", true);
        connect(b, &QToolButton::clicked, a, &QAction::trigger);
    }
}

// FormClose L146–L148.
void FormGPSLog::FormClose(vcl::Event *) {
    if (statsTimer) statsTimer->stop();
}

void FormGPSLog::popup(const QStringList &files, QWidget *owner, bool british_, double tzShift_) {
    const Lang &lang = Lang::instance();
    british = british_;
    tzShift = tzShift_;
    QTreeWidget *l = list();
    if (!l) return;
    l->setHeaderLabels({lang.tr("File"), lang.tr("Points"), lang.tr("FileList_Col_Duration"),
                        lang.tr("FileList_Col_Track") + ", " + lang.tr(british ? "Miles" : "Kmeters")});
    for (int c = 1; c < 4; ++c) l->headerItem()->setTextAlignment(c, Qt::AlignRight | Qt::AlignVCenter);
    if (l->topLevelItemCount() == 0) {
        const QSignalBlocker block(l);
        for (const QString &f : files) addFile(f)->setCheckState(0, Qt::Unchecked);
    }
    // L120–L125: "<caption> - .<ext>", the first one ticked.
    if (auto *c = find<QListWidget>("CheckListBoxConverters"); c && c->count() == 0) {
        for (const gpsfiles::Writer &w : gpsfiles::writers()) {
            auto *it = new QListWidgetItem(w.caption + " - ." + w.ext, c);
            it->setData(Qt::UserRole, w.id);
            it->setFlags(it->flags() | Qt::ItemIsUserCheckable);
            it->setCheckState(c->count() == 1 ? Qt::Checked : Qt::Unchecked);
        }
    }
    // L128–L131: opened next to the main window, no taller than its client area.
    if (!isVisible() && owner) {
        move(owner->pos() + QPoint(30, 70));
        resize(width(), std::min(height(), owner->height() - 10));
    }
    l->sortItems(0, Qt::AscendingOrder);
    show();
    raise();
    activateWindow();
    statsTimer->start(500);  // L136–L140
    ListViewFilesClick(nullptr);
}

void FormGPSLog::markOnly(const QStringList &paths) {
    QTreeWidget *l = list();
    if (!l) return;
    const QSignalBlocker block(l);
    l->clearSelection();
    QTreeWidgetItem *last = nullptr;
    for (int i = 0; i < l->topLevelItemCount(); ++i) {
        QTreeWidgetItem *it = l->topLevelItem(i);
        const bool on = paths.contains(pathOf(it), Qt::CaseInsensitive);
        it->setCheckState(0, on ? Qt::Checked : Qt::Unchecked);
        it->setSelected(on);
        if (on) last = it;
    }
    if (last) l->scrollToItem(last);
    ListViewFilesClick(nullptr);
}

QStringList FormGPSLog::checkedFiles() const {
    QStringList out;
    if (QTreeWidget *l = list()) {
        for (int i = 0; i < l->topLevelItemCount(); ++i) {
            if (l->topLevelItem(i)->checkState(0) == Qt::Checked) out.append(pathOf(l->topLevelItem(i)));
        }
    }
    return out;
}

QTreeWidgetItem *FormGPSLog::addFile(const QString &path) {
    auto *it = new QTreeWidgetItem(list());
    it->setText(0, QDir::toNativeSeparators(path));
    it->setData(0, Qt::UserRole, path);
    it->setFlags(it->flags() | Qt::ItemIsUserCheckable);
    it->setCheckState(0, Qt::Checked);
    for (int c = 1; c < 4; ++c) it->setTextAlignment(c, Qt::AlignRight | Qt::AlignVCenter);
    return it;
}

// AfterAdd L172–L179.
void FormGPSLog::afterAdd(QTreeWidgetItem *it) {
    QTreeWidget *l = list();
    l->sortItems(0, Qt::AscendingOrder);
    if (it) {
        l->setCurrentItem(it);
        it->setSelected(true);
        l->scrollToItem(it);
    }
    l->setFocus();
    statsTimer->start(1);
}

// OnTimer L306–L329: one row without statistics per tick, then a check every second.
void FormGPSLog::fillNext() {
    statsTimer->setInterval(1000);
    QTreeWidget *l = list();
    if (!isVisible() || !l) return;
    const double k = british ? 0.001 / 1.609344 : 0.001;
    for (int i = 0; i < l->topLevelItemCount(); ++i) {
        QTreeWidgetItem *it = l->topLevelItem(i);
        if (!it->text(1).isEmpty()) continue;
        MovingInfo mi;
        mi.loadSource(pathOf(it));
        const QSignalBlocker block(l);
        if (mi.gpsCount() == 0 && !mi.hasGSensor()) {
            it->setText(1, Lang::instance().tr("Error"));
        } else {
            it->setText(1, QString::number(mi.gpsCount()));
            it->setText(2, durationText(mi.span() * 86400.0));
            it->setText(3, QString::number(mi.trackLength() * k, 'f', 1));
        }
        statsTimer->setInterval(1);
        break;
    }
}

// L218–L226.
void FormGPSLog::CheckBoxAllClick(vcl::Event *) {
    QTreeWidget *l = list();
    auto *all = find<QCheckBox>("CheckBoxAll");
    if (!l || !all) return;
    {
        const QSignalBlocker block(l);
        for (int i = 0; i < l->topLevelItemCount(); ++i) {
            l->topLevelItem(i)->setCheckState(0, all->isChecked() ? Qt::Checked : Qt::Unchecked);
            l->topLevelItem(i)->setSelected(false);
        }
    }
    ListViewFilesClick(nullptr);
}

void FormGPSLog::ListViewFilesClick(vcl::Event *e) {
    ListViewFilesSelectItem(e);
}

// GM-191 (L339–L352): the preview follows after 100 ms for one row, 500 ms for more.
void FormGPSLog::ListViewFilesSelectItem(vcl::Event *) {
    QTimer *t = timer("TimerDrawRoute");
    QTreeWidget *l = list();
    if (!t || !l) return;
    t->stop();
    int n = 0;
    for (int i = 0; i < l->topLevelItemCount(); ++i) {
        const QTreeWidgetItem *it = l->topLevelItem(i);
        if (it->isSelected() || it->checkState(0) == Qt::Checked) ++n;
    }
    t->setInterval(n < 2 ? 100 : 500);
    if (auto *c = find<QCheckBox>("CheckBoxShowPath"); c && c->isChecked()) t->start();
}

// GM-191 (L360–L382).
void FormGPSLog::TimerDrawRouteTimer(vcl::Event *) {
    if (QTimer *t = timer("TimerDrawRoute")) t->stop();
    QTreeWidget *l = list();
    if (!l) return;
    QStringList paths;
    QList<bool> selected;
    for (int i = 0; i < l->topLevelItemCount(); ++i) {
        const QTreeWidgetItem *it = l->topLevelItem(i);
        if (it->isSelected() || it->checkState(0) == Qt::Checked) {
            paths.append(pathOf(it));
            selected.append(it->isSelected());
        }
    }
    if (paths.size() == 1) selected[0] = false;  // a single file is red
    QApplication::setOverrideCursor(Qt::BusyCursor);
    emit previewFiles(paths, selected);
    QApplication::restoreOverrideCursor();
}

// GM-192 (L235–L280).
void FormGPSLog::ButtonConvertClick(vcl::Event *) {
    const Lang &lang = Lang::instance();
    QTreeWidget *l = list();
    if (!l) return;
    if (checkedFiles().isEmpty()) {
        QMessageBox::information(this, windowTitle(), lang.tr("GPSLog_SelectToProcess"));
        return;
    }
    const bool join = find<QCheckBox>("CheckBoxJoin") && find<QCheckBox>("CheckBoxJoin")->isChecked();
    for (const char *n : {"ButtonConvert", "CheckBoxAll"}) {
        if (QWidget *w = find<QWidget>(n)) w->setEnabled(false);
    }
    QVector<gpsfiles::Source> joined;
    QString dir;
    for (int i = 0; i < l->topLevelItemCount(); ++i) {
        QTreeWidgetItem *it = l->topLevelItem(i);
        if (it->checkState(0) != Qt::Checked) continue;
        l->scrollToItem(it);
        const QString fn = pathOf(it);
        QApplication::processEvents();
        if (!join) {
            convert({{fn}}, fn);
        } else {
            joined.append({fn});
            if (dir.isEmpty()) dir = QFileInfo(fn).absolutePath();
        }
        const QSignalBlocker block(l);
        it->setCheckState(0, Qt::Unchecked);
    }
    // L272–L275: one file named after the merged track, shown in Explorer.
    if (join && !dir.isEmpty()) {
        const gpsfiles::Result info = gpsfiles::write(joined, QString(), QString(), {tzShift, british});
        if (info.hasPoints) {
            if (const QString written = convert(joined, QDir(dir).filePath(info.fileName + ".")); !written.isEmpty()) {
                QProcess::startDetached("explorer.exe", {"/select,", QDir::toNativeSeparators(written)});
            }
        }
    }
    for (const char *n : {"CheckBoxAll", "ButtonConvert"}) {
        if (QWidget *w = find<QWidget>(n)) w->setEnabled(true);
    }
}

// Convert L291–L299: one file per ticked format, the extension replaced (ChangeExt). Returns
// the last file written. A source without GPS data writes nothing (TMovingInfo was nil).
QString FormGPSLog::convert(const QVector<gpsfiles::Source> &sources, QString path) {
    auto *c = find<QListWidget>("CheckListBoxConverters");
    if (!c) return {};
    QString written;
    const QVector<gpsfiles::Writer> writers = gpsfiles::writers();
    for (int i = 0; i < c->count() && i < writers.size(); ++i) {
        if (c->item(i)->checkState() != Qt::Checked) continue;
        const int dot = path.lastIndexOf('.');
        path = (dot > path.lastIndexOf('/') && dot > path.lastIndexOf('\\') ? path.left(dot) : path) + "." + writers[i].ext;
        const gpsfiles::Result r = gpsfiles::write(sources, writers[i].id, path, {tzShift, british});
        if (!r.hasPoints) return written;
        if (!r.ok) {
            QMessageBox::critical(this, Lang::instance().tr("Error"), QDir::toNativeSeparators(path) + "\n\n" + r.error);
            return written;
        }
        written = path;
    }
    return written;
}

// L154–L169.
void FormGPSLog::N_FileList_AddClick(vcl::Event *) {
    QTreeWidget *l = list();
    if (!l) return;
    l->clearSelection();
    const QStringList filters = gpsfiles::openFilters();
    QString selected = filters.value(0);
    const QStringList files = QFileDialog::getOpenFileNames(this, action("N_FileList_Add") ? action("N_FileList_Add")->text() : QString(),
                                                            QString(), filters.join(";;"), &selected);
    QTreeWidgetItem *it = nullptr;
    {
        const QSignalBlocker block(l);
        for (const QString &f : files) it = addFile(f);
    }
    afterAdd(it);
}

void FormGPSLog::dragEnterEvent(QDragEnterEvent *e) {
    if (e->mimeData()->hasUrls()) e->acceptProposedAction();
}

// WMDropFiles L187–L213: dropped files are ticked and selected.
void FormGPSLog::dropEvent(QDropEvent *e) {
    QTreeWidget *l = list();
    if (!l) return;
    l->clearSelection();
    QTreeWidgetItem *it = nullptr;
    {
        const QSignalBlocker block(l);
        for (const QUrl &u : e->mimeData()->urls()) {
            const QString f = u.toLocalFile();
            if (f.isEmpty() || f.size() >= 255 || !QFileInfo(f).isFile()) continue;
            it = addFile(f);
            it->setSelected(true);
        }
    }
    afterAdd(it);
}

// L400–L404.
void FormGPSLog::N_FileList_DelClick(vcl::Event *) {
    if (QTreeWidget *l = list()) qDeleteAll(l->selectedItems());
}

// L444–L446.
void FormGPSLog::N_FileList_ClearClick(vcl::Event *) {
    if (QTreeWidget *l = list()) l->clear();
}

// DelIt L427–L431: the log and its conversions (same name, each export extension).
void FormGPSLog::deleteWithConversions(const QString &path) {
    const QString base = path.left(path.size() - 3);
    for (const gpsfiles::Writer &w : gpsfiles::writers()) QFile::remove(base + w.ext);
    QFile::remove(path);
}

// L387–L395.
void FormGPSLog::N_FileList_DelDiskClick(vcl::Event *) {
    QTreeWidget *l = list();
    QTreeWidgetItem *it = l ? l->currentItem() : nullptr;
    if (!it) return;
    const QString fn = pathOf(it);
    const QString text = Lang::instance().tr("Mes_SureToDelFile") + "\r\n" + QDir::toNativeSeparators(fn) + " ?";
    if (QMessageBox::question(this, windowTitle(), text) != QMessageBox::Yes) return;
    deleteWithConversions(fn);
    delete it;
}

// L409–L418.
void FormGPSLog::N_FileList_DelDiskLogsClick(vcl::Event *) {
    QTreeWidget *l = list();
    if (!l) return;
    if (QMessageBox::question(this, windowTitle(), Lang::instance().tr("Mes_SureToDelAllGPSLogs") + "?") != QMessageBox::Yes) return;
    for (int i = 0; i < l->topLevelItemCount(); ++i) deleteWithConversions(pathOf(l->topLevelItem(i)));
    l->clear();
    emit logsDeleted();
    close();
}

// L436–L441.
void FormGPSLog::N_FileList_ShowExplorerClick(vcl::Event *) {
    QTreeWidget *l = list();
    if (QTreeWidgetItem *it = l ? l->currentItem() : nullptr) {
        QProcess::startDetached("explorer.exe", {"/select,", QDir::toNativeSeparators(pathOf(it))});
    }
}

// L452–L464: Space ticks or unticks all selected rows like the first of them.
void FormGPSLog::N_SpaceClick(vcl::Event *) {
    QTreeWidget *l = list();
    if (!l) return;
    const QList<QTreeWidgetItem *> sel = l->selectedItems();
    if (sel.isEmpty()) return;
    const Qt::CheckState v = sel.first()->checkState(0) == Qt::Checked ? Qt::Unchecked : Qt::Checked;
    {
        const QSignalBlocker block(l);
        for (QTreeWidgetItem *it : sel) it->setCheckState(0, v);
    }
    ListViewFilesClick(nullptr);
}
