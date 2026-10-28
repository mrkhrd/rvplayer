// File list of TFormMain: the list view, its toolbar, the file manager and the list commands.
// Spec: file-list.md.

#include "MainForm.h"

#include "core/Playback.h"
#include "core/Settings.h"
#include "filelist/FileListView.h"
#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"
#include "vcl/Widgets.h"

#include <QAction>
#include <QApplication>
#include <QDir>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QElapsedTimer>
#include <QFileDialog>
#include <QFrame>
#include <QHeaderView>
#include <QMenu>
#include <QMimeData>
#include <QProcess>
#include <QSignalBlocker>
#include <QTimer>
#include <QToolButton>

#include <cmath>

using vcl::Align;

namespace {

// MakeDisabledImage (L780–L798) with Faktor 2: grey (r + 2g + b) / 4 pulled halfway to white,
// dithered with white on every other pixel. The dither is averaged here, which looks the same at
// normal size and stays clean when the image is scaled up.
QPixmap disabledImage(const QPixmap &src) {
    QImage img = src.toImage().convertToFormat(QImage::Format_ARGB32);
    for (int y = 0; y < img.height(); ++y) {
        auto *line = reinterpret_cast<QRgb *>(img.scanLine(y));
        for (int x = 0; x < img.width(); ++x) {
            const QRgb c = line[x];
            const int grey = (qRed(c) + 2 * qGreen(c) + qBlue(c)) / 4;
            const int v = 255 - int(std::lround((255 - grey) / 2.0));
            const int d = (v + 255) / 2;
            line[x] = qRgba(d, d, d, qAlpha(c));
        }
    }
    QPixmap out = QPixmap::fromImage(img);
    out.setDevicePixelRatio(src.devicePixelRatio());
    return out;
}

// Column ID → caption key (DoTranslate L4674–L4685).
const char *const kColumnKeys[] = {"FileList_Col_Name", "FileList_Col_Duration", "FileList_Col_DateTime",
                                   "FileList_Col_Track", "KmetersPerH", "FileList_Col_FilePath",
                                   "Mb", "FileList_Col_PictSize"};
const int kDefaultWidths[] = {136, 47, 97, 56, 40, 1, 45, 64};  // FL-022 (FormCreate L929–L936)
constexpr int kColumns = int(std::size(kDefaultWidths));

}  // namespace

FileListView *MainForm::listView() const {
    return find<FileListView>("ListView");
}

// CreateFileListMenuButtons (L805–L857): one flat 20×20 button per item of the list's pop-up
// menu, with bevels for the separators.
void MainForm::createFileListMenuButtons() {
    auto *panel = find<vcl::Panel>("PanelFileListMenu");
    QMenu *popup = menu("PopupMenuListView");
    if (!panel || !popup) return;
    const int h = controlSize(panel).height();
    int x = 0;
    for (QAction *a : popup->actions()) {
        const QString name = a->objectName();
        if (name == "N_FileList_DelDiskAll" || name == "N_FileList_Open" || name == "N_SFL1") continue;
        if (a->isSeparator()) {
            auto *bevel = new QFrame(panel);
            bevel->setFrameShape(QFrame::VLine);
            bevel->setFrameShadow(QFrame::Sunken);
            layoutOf(panel)->addControl(bevel, Align::None, vcl::Anchors{}, QRect(x + 1, 2, 2, h - 4));
            x += 4;
            continue;
        }
        auto *b = new QToolButton(panel);
        b->setObjectName("FileListButton_" + name);
        b->setAutoRaise(true);
        b->setFocusPolicy(Qt::NoFocus);
        const QPixmap glyph = a->icon().pixmap(QSize(16, 16), devicePixelRatioF());
        QIcon icon;
        icon.addPixmap(glyph, QIcon::Normal);
        icon.addPixmap(disabledImage(glyph), QIcon::Disabled);
        b->setIcon(icon);
        b->setIconSize(QSize(16, 16));
        const int w = a->isVisible() ? h : 0;
        layoutOf(panel)->addControl(b, Align::None, vcl::Anchors{}, QRect(x, 0, w, h));
        if (w == 0) b->hide();
        x += w;
        connect(b, &QToolButton::clicked, this, [this, a] {
            a->trigger();  // OnFileListMenuButtonClick L5592–L5599
            updateFileListMenu();
        });
        fileListButtons.append({b, a});
    }
}

void MainForm::setupListView() {
    FileListView *list = listView();
    if (!list) return;
    QHeaderView *header = list->header();
    header->setStretchLastSection(false);
    header->setSectionsMovable(true);  // FullDrag
    header->setHighlightSections(false);
    header->setMinimumSectionSize(1);
    header->setSortIndicatorShown(false);
    header->setSectionsClickable(true);
    QPalette pal = list->palette();
    pal.setColor(QPalette::Base, Lang::instance().color("colorListBG"));  // FormCreate L945
    pal.setColor(QPalette::Text, Lang::instance().color("colorListText"));
    list->setPalette(pal);

    // FL-023: saved widths (deleted when equal to the default) and display order.
    Settings &s = Settings::instance();
    for (int i = 0; i < kColumns; ++i) {
        int w = s.readInt(QStringLiteral("col%1_w").arg(i), -1);
        if (w == kDefaultWidths[i]) s.remove(QStringLiteral("col%1_w").arg(i));
        list->setColumnWidth(i, w >= 0 ? w : kDefaultWidths[i]);
    }
    for (int p = 0; p < kColumns; ++p) {
        const int id = s.readInt(QStringLiteral("col%1_i").arg(p), p);
        if (id >= 0 && id < kColumns) header->moveSection(header->visualIndex(id), p);
    }

    connect(list->selectionModel(), &QItemSelectionModel::selectionChanged, this, [this] { ListViewSelectItem(nullptr); });
    connect(list, &QTreeView::clicked, this, [this] { ListViewClick(nullptr); });
    connect(list, &QTreeView::doubleClicked, this, [this] { ListViewDblClick(nullptr); });
    connect(header, &QHeaderView::sectionClicked, this, [this](int logical) {
        vcl::Event e;
        e.key = logical;
        ListViewColumnClick(&e);
    });
    // FL-024: end of a column resize or a drag of a column.
    connect(header, &QHeaderView::sectionResized, this, [this] { ListViewColumnDragged(nullptr); });
    connect(header, &QHeaderView::sectionMoved, this, [this] { ListViewColumnDragged(nullptr); });
    // FL-007: files dropped anywhere on the window.
    setAcceptDrops(true);
    updateFileListMenu();
}

// TR-024: captions by column ID, km/h or mph by the measurement units.
void MainForm::translateListView() {
    FileListView *list = listView();
    if (!list) return;
    const Lang &lang = Lang::instance();
    const bool british = isChecked("N_View_Measu_British");
    QStringList headers;
    for (int i = 0; i < kColumns; ++i) {
        headers.append(lang.tr((i == 4 && british) ? QStringLiteral("MilesPerH") : QString::fromLatin1(kColumnKeys[i])));
    }
    list->listModel()->setHeaders(headers);
    list->listModel()->setBritish(british);
    list->viewport()->update();
    for (const auto &[button, a] : std::as_const(fileListButtons)) button->setToolTip(a->text());
}

void MainForm::PopupMenuListViewPopup(vcl::Event *) {
    updateFileListMenu();
}

// FL-040…FL-047 (PopupMenuListViewPopup L3519–L3565).
void MainForm::updateFileListMenu() {
    QMenu *popup = menu("PopupMenuListView");
    FileListView *list = listView();
    if (!popup || !list) return;
    const bool r = list->currentIndex().isValid();
    const QList<int> selected = list->selectedRows();
    for (QAction *a : popup->actions()) {
        if (!a->isSeparator()) a->setEnabled(r);
    }
    auto set = [this](const char *name, bool on) {
        if (QAction *a = action(name)) a->setEnabled(on);
    };
    for (const char *name : {"N_FileList_Add", "N_FileList_DelDiskAll", "N_FileList_Refresh", "N_FileList_Clear"}) {
        set(name, true);
    }
    if (r) {
        const FileRows &rows = list->listModel()->fileRows();
        bool lockable = false, unlockable = false, ungroupable = false, gps = false;
        for (int i : selected) {
            if (i >= rows.rows.size()) continue;
            const FileRow &row = rows.rows[i];
            lockable |= row.lock != 1;
            unlockable |= row.lock != 0;
            ungroupable |= row.files > 1;
            gps |= row.gps > 0;
        }
        set("N_FileList_SetLock", lockable);
        set("N_FileList_UnsetLock", unlockable);
        set("N_FileList_Ungroup", ungroupable);
        set("N_FileList_Track_GPSLog", false);
        set("N_FileList_Track_Export", gps);
        set("N_FileList_Track_Google", gps);
        set("N_FileList_DelDisk", !unlockable);
    }
    set("N_FileList_Open", selected.size() == 1);
    set("N_FileList_Track_Set", selected.size() == 1);
    set("N_FileList_Track_Move", selected.size() == 1);
    const bool any = list->listModel()->rowCount() > 0;
    for (const char *name : {"N_FileList_Group_With", "N_FileList_Group_Auto", "N_FileList_Ungroup_All"}) set(name, any);
    for (const auto &[button, a] : std::as_const(fileListButtons)) button->setEnabled(a->isEnabled());
}

void MainForm::clearListSelection() {
    if (FileListView *list = listView()) list->clearSelection();
}

// --- File manager -----------------------------------------------------------------------------

void MainForm::createFileManager(bool autoSearch) {
    delete fileManager;
    fileManager = new FileManager(autoSearch, profile() != Profile::Video, this);
    Settings &s = Settings::instance();
    fileManager->setSort(s.readInt("col_sort", 2), s.readBool("col_sort_dur", true));
    connect(fileManager, &FileManager::changed, this, &MainForm::onFileManagerChanges);
    onFileManagerChanges();
}

// FL-012.
void MainForm::onFileManagerChanges() {
    FileListView *list = listView();
    if (!list || !fileManager) return;
    const QList<int> before = list->selectedRows();
    const int current = list->currentIndex().row();
    {
        QSignalBlocker block(list->selectionModel());
        list->listModel()->setRows(fileManager->rows());
        // The selection follows the remembered item (Q-18), else the same rows stay selected.
        const FileRows &rows = list->listModel()->fileRows();
        int found = -1;
        if (!lastSelectedName.isEmpty()) {
            for (int i = 0; i < rows.rows.size(); ++i) {
                if (rows.rows[i].name == lastSelectedName) {
                    found = i;
                    break;
                }
            }
        }
        if (found >= 0) {
            list->selectRow(found);
        } else if (!lastSelectedName.isEmpty()) {
            lastSelectedName.clear();
        } else {
            for (int r : before) {
                if (r < list->listModel()->rowCount()) {
                    list->selectionModel()->select(list->listModel()->index(r, 0),
                                                   QItemSelectionModel::Select | QItemSelectionModel::Rows);
                }
            }
            if (current >= 0 && current < list->listModel()->rowCount()) {
                list->selectionModel()->setCurrentIndex(list->listModel()->index(current, 0), QItemSelectionModel::NoUpdate);
            }
        }
    }
    list->viewport()->update();
    updateFileListMenu();
}

// FL-002 (F5, Ctrl+R).
void MainForm::N_FileList_RefreshClick(vcl::Event *) {
    createFileManager(true);
}

// FL-009 (Ctrl+N).
void MainForm::N_FileList_ClearClick(vcl::Event *) {
    if (fileManager) fileManager->clear();
}

// FL-006 (Ctrl+O, Insert).
void MainForm::N_File_OpenClick(vcl::Event *) {
    const QString filter = QStringLiteral(
        "DashCam Files (*.mov *.mp4 *.mp4v);;"
        "Video Files (*.3gp *.asf *.avi *.avs *.cme *.cof *.csa *.divx *.flv *.m1v *.m2v *.mls *.mov *.mp4 *.mpe *.mpeg "
        "*.mpg *.ogm *.mkv *.xfl *.rm *.rmvb *.viv *.vob *.wmv *.xqt *.vp3 *.vp4 *.vp5 *.vp6 *.vp7);;"
        "Any Files (*.*)");
    const QStringList files = QFileDialog::getOpenFileNames(this, action("N_File_Open")->text(), QString(), filter);
    if (!files.isEmpty()) addFiles(files);
}

// FL-008.
void MainForm::addFiles(const QStringList &paths) {
    if (!fileManager) createFileManager(false);
    QApplication::setOverrideCursor(Qt::WaitCursor);
    for (const QString &p : paths) {
        if (!p.isEmpty()) fileManager->add(p);
    }
    QApplication::restoreOverrideCursor();
    if (FileListView *list = listView(); list && isVisible()) list->setFocus();
}

void MainForm::dragEnterEvent(QDragEnterEvent *e) {
    if (e->mimeData()->hasUrls()) e->acceptProposedAction();
}

// FL-007.
void MainForm::dropEvent(QDropEvent *e) {
    QStringList paths;
    for (const QUrl &u : e->mimeData()->urls()) {
        if (u.isLocalFile()) paths.append(u.toLocalFile());
    }
    addFiles(paths);
}

// FL-001: command-line files are added to a manager without auto-search and the first row
// plays; without them the list is refreshed.
void MainForm::TimerOnStartTimer(vcl::Event *e) {
    if (auto *t = e ? qobject_cast<QTimer *>(e->sender) : nullptr) t->stop();
    const QStringList params = QApplication::arguments().mid(1);
    if (!params.isEmpty()) {
        createFileManager(false);
        for (const QString &p : params) fileManager->add(p);
        QElapsedTimer waited;
        waited.start();
        while (!fileManager->idle() && waited.elapsed() < 10000) {
            QApplication::processEvents(QEventLoop::AllEvents, 50);
        }
        fileManager->refresh();
        if (FileListView *list = listView(); list && list->listModel()->rowCount() > 0 && !list->listModel()->hasStatus()) {
            list->selectRow(0);
            ListViewDblClick(nullptr);
            return;
        }
    }
    if (!fileManager || fileManager->rows().rows.isEmpty()) createFileManager(true);
}

// --- Selection and activation -----------------------------------------------------------------

// FL-034.
bool MainForm::isNowPlaying(int row) const {
    FileListView *list = listView();
    const FileRows &rows = list->listModel()->fileRows();
    if (row < 0 || row >= rows.rows.size() || playback->group().isEmpty()) return false;
    QStringList paths;
    for (const GroupFile &f : playback->group().files) paths.append(f.path);
    return rows.rows[row].path == paths.join(", ");
}

// FL-031 / PV-010.
void MainForm::ListViewSelectItem(vcl::Event *) {
    FileListView *list = listView();
    if (!list || !fileManager) return;
    const QList<int> selected = list->selectedRows();
    if (selected.size() != 1) {
        // OnWmUser3: a multiple (or empty) selection stops playback.
        QTimer::singleShot(0, this, [this] {
            if (listView() && listView()->selectedRows().size() != 1) {
                lastSelectedName.clear();
                if (!playback->group().isEmpty()) stopPlayback(false);
            }
            updateFileListMenu();
        });
        return;
    }
    const int idx = selected.first();
    const FileRows &rows = list->listModel()->fileRows();
    if (list->listModel()->hasStatus() && idx == 0) return;  // the status row is not an item
    if (idx >= rows.rows.size()) return;
    lastSelectedName = rows.rows[idx].name;
    auto *play = find<QToolButton>("ButtonPlay");
    if (!isNowPlaying(idx) || (play && !play->isEnabled())) {
        QApplication::setOverrideCursor(Qt::BusyCursor);
        const Group g = fileManager->item(idx);
        loadGroup(g);
        // PV-051: GOP sizes in multiples of 15 get 3x/15x/60x presets.
        const bool gop15 = !g.isEmpty() && g.files[0].gop > 0 && g.files[0].gop % 15 == 0;
        const int tags[] = {2, 14, 59}, tags64[] = {3, 15, 63};
        for (int i = 0; i < 3 && 3 + i < speedButtons.size(); ++i) {
            setSpeedButtonTag(speedButtons[3 + i], gop15 ? tags[i] : tags64[i]);
        }
        QApplication::restoreOverrideCursor();
    }
    updateFileListMenu();
}

// FL-032 / PV-011.
void MainForm::ListViewClick(vcl::Event *) {
    const Qt::KeyboardModifiers mods = QApplication::keyboardModifiers();
    auto *play = find<QToolButton>("ButtonPlay");
    if (mods & (Qt::ControlModifier | Qt::ShiftModifier)) {
        ListViewSelectItem(nullptr);
        return;
    }
    if (play && play->isEnabled() && !playback->active()) {
        play->setChecked(false);
        ButtonPlayClick(nullptr);
    }
}

// FL-035 / PV-012.
void MainForm::ListViewDblClick(vcl::Event *) {
    auto *play = find<QToolButton>("ButtonPlay");
    if (!play) return;
    if (!play->isEnabled()) {
        stopPlayback(false);
        ListViewSelectItem(nullptr);
    }
    if (!play->isEnabled()) return;
    play->setChecked(true);
    ButtonPlayClick(nullptr);
}

// FL-025: sort by the clicked column; again toggles the direction.
void MainForm::ListViewColumnClick(vcl::Event *e) {
    if (!e || !fileManager) return;
    Settings &s = Settings::instance();
    const int column = e->key;
    const int current = s.readInt("col_sort", 2);
    const bool ascending = column == current ? !s.readBool("col_sort_dur", true) : true;
    s.writeInt("col_sort", column);
    s.writeBool("col_sort_dur", ascending);
    fileManager->setSort(column, ascending);
}

// FL-024.
void MainForm::ListViewColumnDragged(vcl::Event *) {
    FileListView *list = listView();
    if (!list || !isVisible()) return;
    Settings &s = Settings::instance();
    QHeaderView *h = list->header();
    for (int p = 0; p < kColumns; ++p) {
        const int id = h->logicalIndex(p);
        const int w = std::max(1, h->sectionSize(id));
        if (w == kDefaultWidths[id]) s.remove(QStringLiteral("col%1_w").arg(id));
        else s.writeInt(QStringLiteral("col%1_w").arg(id), w);
        if (id == p) s.remove(QStringLiteral("col%1_i").arg(p));
        else s.writeInt(QStringLiteral("col%1_i").arg(p), id);
    }
}

// --- Commands ---------------------------------------------------------------------------------

// FL-050.
void MainForm::N_FileList_DelClick(vcl::Event *) {
    FileListView *list = listView();
    if (!list || !fileManager) return;
    const QList<int> rows = list->selectedRows();
    for (int r : rows) {
        if (isNowPlaying(r)) stopPlayback(false);
    }
    lastSelectedName.clear();
    fileManager->remove(rows);
}

void MainForm::N_FileList_SetLockClick(vcl::Event *e) {
    FileListView *list = listView();
    if (!list || !fileManager || !e) return;
    const bool lock = e->sender && e->sender->objectName() == "N_FileList_SetLock";
    fileManager->setLock(list->selectedRows(), lock);
}

// FL-056.
void MainForm::N_FileList_UngroupClick(vcl::Event *) {
    if (FileListView *list = listView(); list && fileManager) fileManager->ungroup(list->selectedRows());
}

// FL-058.
void MainForm::N_FileList_Ungroup_AllClick(vcl::Event *) {
    if (fileManager) fileManager->ungroupAll();
}

void MainForm::N_FileList_Group_AutoClick(vcl::Event *) {
    if (fileManager) fileManager->groupAuto();
}

// Show the (first) file of the row in Explorer.
void MainForm::N_FileList_ShowExplorerClick(vcl::Event *) {
    FileListView *list = listView();
    if (!list || !fileManager) return;
    const QList<int> rows = list->selectedRows();
    if (rows.isEmpty()) return;
    const Group g = fileManager->item(rows.first());
    if (g.isEmpty()) return;
    QProcess::startDetached("explorer.exe", {"/select,", QDir::toNativeSeparators(g.files.first().path)});
}
