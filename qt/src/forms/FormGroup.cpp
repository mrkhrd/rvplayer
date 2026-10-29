#include "FormGroup.h"

#include "filelist/FileListView.h"
#include "i18n/Lang.h"
#include "vcl/AlignLayout.h"

#include <QHeaderView>
#include <QPushButton>
#include <QTimer>
#include <QTreeWidget>

FormGroup::FormGroup(QWidget *parent) : vcl::Form(parent) {
    setWindowFlag(Qt::Dialog);
}

QTreeWidget *FormGroup::list() const {
    return find<QTreeWidget>("ListView");
}

void FormGroup::FormCreate(vcl::Event *) {
    const Lang &lang = Lang::instance();
    setWindowIcon(QIcon(":/original/images/ICON_GROUP.ICO"));
    setWindowTitle(lang.tr("Grouping_Title"));
    if (auto *b = find<QPushButton>("ButtonGroup")) b->setText(lang.tr("Grouping_Btn"));
    if (auto *b = find<QPushButton>("ButtonClose")) b->setText(lang.tr("Cancel"));
    if (QTreeWidget *l = list()) {
        l->setRootIsDecorated(false);
        l->setUniformRowHeights(true);
        l->setSelectionBehavior(QAbstractItemView::SelectRows);
        l->setSelectionMode(QAbstractItemView::ExtendedSelection);
        l->header()->setStretchLastSection(false);
        // Ticking a row is a change the preview follows, like selecting one.
        connect(l, &QTreeWidget::itemChanged, this, [this] { restartPreview(); });
    }
}

// Popup L56–L89: the same columns (8 px wider), texts and icons as the main list.
void FormGroup::setRows(const FileListView *main) {
    QTreeWidget *l = list();
    if (!l || !main) return;
    const FileListModel *m = main->listModel();
    QHeaderView *mh = main->header();
    QStringList headers;
    for (int c = 0; c < FileListModel::kColumns; ++c) headers.append(m->headerData(c, Qt::Horizontal, Qt::DisplayRole).toString());
    l->clear();
    l->setColumnCount(FileListModel::kColumns);
    l->setHeaderLabels(headers);
    static const char *iconFiles[] = {"FILELIST_ICON_KEY.BMP", "FILELIST_ICON_KEY_PARTIAL.BMP",
                                      "FILELIST_ICON_VIDEO_WARNING.BMP", "FILELIST_ICON_GPS_FOUND.BMP"};
    const QSignalBlocker block(l);
    const QList<int> selected = main->selectedRows();
    const int current = main->currentIndex().row();
    for (int r = 0; r < m->fileRows().rows.size(); ++r) {
        auto *item = new QTreeWidgetItem(l);
        for (int c = 0; c < FileListModel::kColumns; ++c) {
            item->setText(c, m->text(r, c));
            const QVariant align = m->headerData(c, Qt::Horizontal, Qt::TextAlignmentRole);
            item->setTextAlignment(c, Qt::Alignment(align.toInt()));
            if (const int icon = m->icon(r, c); icon >= 0) item->setIcon(c, QIcon(resourceBitmap(iconFiles[icon])));
        }
        item->setFlags(item->flags() | Qt::ItemIsUserCheckable);
        item->setCheckState(0, selected.contains(r) || r == current ? Qt::Checked : Qt::Unchecked);
    }
    // 8 px wider than in the main list; a column hidden there (1 px, File Location) stays hidden.
    for (int c = 0; c < FileListModel::kColumns; ++c) l->setColumnWidth(c, mh->sectionSize(c) < 8 ? 1 : mh->sectionSize(c) + 8);
    // The column order of the main list.
    for (int v = 0; v < FileListModel::kColumns; ++v) l->header()->moveSection(l->header()->visualIndex(mh->logicalIndex(v)), v);
    if (current >= 0 && current < l->topLevelItemCount()) {
        l->setCurrentItem(l->topLevelItem(current));
        l->scrollToItem(l->topLevelItem(current));
    }
}

QList<int> FormGroup::tickedRows() const {
    QList<int> out;
    if (QTreeWidget *l = list()) {
        for (int r = 0; r < l->topLevelItemCount(); ++r) {
            if (l->topLevelItem(r)->checkState(0) == Qt::Checked) out.append(r);
        }
    }
    return out;
}

void FormGroup::FormShow(vcl::Event *) {
    if (QTreeWidget *l = list()) l->setFocus();
    restartPreview();
}

void FormGroup::FormResize(vcl::Event *) {
    // Cancel stays at the right edge of the bottom panel.
    if (auto *close = find<QWidget>("ButtonClose"); close && close->parentWidget()) {
        vcl::AlignLayout *l = layoutOf(close->parentWidget());
        QRect r = l->controlRect(close);
        r.moveRight(close->parentWidget()->width() - 9);
        l->setControlRect(close, r);
    }
}

// FormKeyDown L188–L203: Space flips the ticks of the selected rows to the opposite of the first one.
void FormGroup::FormKeyDown(vcl::Event *e) {
    QTreeWidget *l = list();
    if (!e || !l || e->key != Qt::Key_Space) return;
    const QList<QTreeWidgetItem *> sel = l->selectedItems();
    if (sel.isEmpty()) return;
    const Qt::CheckState to = sel.first()->checkState(0) == Qt::Checked ? Qt::Unchecked : Qt::Checked;
    for (QTreeWidgetItem *i : sel) i->setCheckState(0, to);
    e->handled = true;
}

// ButtonGroupClick L95–L103.
void FormGroup::ButtonGroupClick(vcl::Event *) {
    setModalResult(mrOk);
}

void FormGroup::ButtonCloseClick(vcl::Event *) {
    setModalResult(mrCancel);
}

void FormGroup::ListViewClick(vcl::Event *) {
    restartPreview();
}

void FormGroup::ListViewSelectItem(vcl::Event *) {
    restartPreview();
}

// ListViewSelectItem L126–L138: the preview follows quickly while one row is ticked, slower with more.
void FormGroup::restartPreview() {
    if (QTimer *t = timer("TimerDrawRoute")) {
        t->stop();
        t->setInterval(tickedRows().size() < 2 ? 100 : 1000);
        t->start();
    }
}

// TimerDrawRouteTimer L155–L182.
void FormGroup::TimerDrawRouteTimer(vcl::Event *e) {
    if (auto *t = e ? qobject_cast<QTimer *>(e->sender) : nullptr) t->stop();
    QSet<int> highlighted;
    if (QTreeWidget *l = list()) {
        for (QTreeWidgetItem *i : l->selectedItems()) highlighted.insert(l->indexOfTopLevelItem(i));
    }
    emit previewRoutes(tickedRows(), highlighted);
}
