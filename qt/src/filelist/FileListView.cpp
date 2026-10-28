#include "FileListView.h"

#include "core/Format.h"
#include "i18n/Lang.h"
#include "vcl/Form.h"

#include <QHeaderView>
#include <QPainter>
#include <QStyledItemDelegate>

namespace {

constexpr int kRowHeight = 17;
const bool kRightAligned[] = {false, true, true, true, true, false, true, true};

class RowHeightDelegate : public QStyledItemDelegate {
public:
    using QStyledItemDelegate::QStyledItemDelegate;
    QSize sizeHint(const QStyleOptionViewItem &, const QModelIndex &) const override { return {0, kRowHeight}; }
};

// Delphi Font.Height > 0 is the character cell height (em plus internal leading), so the em
// size is the largest one whose line height fits that cell.
QFont listFont(int cellHeight) {
    QFont f(Lang::instance().trDef("LangFont", "Arial"));
    int px = cellHeight;
    f.setPixelSize(px);
    while (px > 1 && QFontMetrics(f).height() > cellHeight) f.setPixelSize(--px);
    return f;
}

}  // namespace

// --- Model ------------------------------------------------------------------------------------

void FileListModel::setRows(const FileRows &rows) {
    beginResetModel();
    rows_ = rows;
    endResetModel();
}

void FileListModel::setHeaders(const QStringList &h) {
    headers = h;
    emit headerDataChanged(Qt::Horizontal, 0, kColumns - 1);
}

int FileListModel::rowCount(const QModelIndex &parent) const {
    if (parent.isValid()) return 0;
    const int n = int(rows_.rows.size());
    return hasStatus() ? std::max(1, n) : n;  // FL-011: the status needs a row
}

int FileListModel::columnCount(const QModelIndex &parent) const {
    return parent.isValid() ? 0 : kColumns;
}

QString FileListModel::text(int row, int column) const {
    if (row < 0 || row >= rows_.rows.size()) return {};
    const FileRow &r = rows_.rows[row];
    switch (column) {
    case 0: return r.name;
    case 1: return fmt::duration(r.durationMs, r.durationState);
    case 2: return fmt::dateTime(r.start(), tzShift);
    case 3: return r.gps == 2 || r.trackM > 0 ? fmt::trip(r.trackM, british) : QString();
    case 4: return r.avgSpeed > 0 ? fmt::speed(r.avgSpeed, british) : QString();
    case 5: return r.path;
    case 6: return fmt::size(r.size);
    case 7: return fmt::resolution(r.width, r.height);
    default: return {};
    }
}

int FileListModel::icon(int row, int column) const {
    if (row < 0 || row >= rows_.rows.size()) return -1;
    const FileRow &r = rows_.rows[row];
    if (column == 0) return r.lock == 1 ? 0 : r.lock == 2 ? 1 : -1;
    if (column == 1) return r.durationState == 2 ? 2 : -1;
    if (column == 3) return r.gps == 2 ? 3 : -1;
    return -1;
}

QString FileListModel::statusText() const {
    QString s = Lang::instance().tr(rows_.statusKey);
    if (rows_.statusKey == "Preview") s += " " + rows_.statusArg + "%";
    else if (!rows_.statusArg.isEmpty()) s += " - " + rows_.statusArg;
    return ". . . " + s + " . . .";
}

QVariant FileListModel::data(const QModelIndex &index, int role) const {
    if (role == Qt::DisplayRole) return text(index.row(), index.column());
    return {};
}

QVariant FileListModel::headerData(int section, Qt::Orientation o, int role) const {
    if (o != Qt::Horizontal) return {};
    if (role == Qt::DisplayRole) return section < headers.size() ? headers[section] : QString();
    if (role == Qt::TextAlignmentRole) {
        return int((section < kColumns && kRightAligned[section] ? Qt::AlignRight : Qt::AlignLeft) | Qt::AlignVCenter);
    }
    return {};
}

// --- View -------------------------------------------------------------------------------------

FileListView::FileListView(QWidget *parent) : QTreeView(parent), model_(new FileListModel(this)) {
    setModel(model_);
    setItemDelegate(new RowHeightDelegate(this));
    setRootIsDecorated(false);
    setUniformRowHeights(true);
    setItemsExpandable(false);
    setAllColumnsShowFocus(true);
    setSelectionBehavior(QAbstractItemView::SelectRows);
    setSelectionMode(QAbstractItemView::ExtendedSelection);
    setEditTriggers(QAbstractItemView::NoEditTriggers);
    setTextElideMode(Qt::ElideNone);
    setFrameShape(QFrame::NoFrame);
    static const char *files[] = {"FILELIST_ICON_KEY.BMP", "FILELIST_ICON_KEY_PARTIAL.BMP",
                                  "FILELIST_ICON_VIDEO_WARNING.BMP", "FILELIST_ICON_GPS_FOUND.BMP"};
    for (int i = 0; i < 4; ++i) icons[i] = vcl::Form::resourceBitmap(files[i]);
}

QList<int> FileListView::selectedRows() const {
    QList<int> out;
    if (!selectionModel()) return out;
    for (const QModelIndex &i : selectionModel()->selectedRows()) out.append(i.row());
    std::sort(out.begin(), out.end());
    return out;
}

void FileListView::selectRow(int row) {
    if (row < 0 || row >= model_->rowCount()) return;
    const QModelIndex i = model_->index(row, 0);
    selectionModel()->setCurrentIndex(i, QItemSelectionModel::ClearAndSelect | QItemSelectionModel::Rows);
    scrollTo(i);
}

void FileListView::drawRow(QPainter *p, const QStyleOptionViewItem &option, const QModelIndex &index) const {
    const int row = index.row();
    const bool selected = selectionModel() && selectionModel()->isRowSelected(row, QModelIndex());
    QHeaderView *h = header();
    const bool status = model_->hasStatus() && row == 0;
    for (int vis = 0; vis < h->count(); ++vis) {
        const int col = h->logicalIndex(vis);
        if (h->isSectionHidden(col)) continue;
        QRect cell(h->sectionViewportPosition(col), option.rect.y(), h->sectionSize(col), option.rect.height());
        if (status && vis >= 1) {
            // FL-011: the status text spans from display position 1 to the right edge.
            cell.setRight(viewport()->width());
            p->fillRect(cell, selected ? Lang::instance().color("colorListSelBG") : Lang::instance().color("colorListBG"));
            p->setFont(listFont(14));
            p->setPen(Lang::instance().color(selected ? "colorListSelText" : "colorListText"));
            p->drawText(cell.adjusted(1, 0, 0, 0), Qt::AlignLeft | Qt::AlignVCenter, model_->statusText());
            break;
        }
        paintCell(p, cell, row, col, selected);
    }
    // The focused row gets a dotted rectangle.
    if (currentIndex().row() == row && hasFocus()) {
        QPen pen(Lang::instance().color("colorListText"), 1, Qt::DotLine);
        p->setPen(pen);
        p->setBrush(Qt::NoBrush);
        const int right = h->sectionViewportPosition(h->logicalIndex(h->count() - 1)) + h->sectionSize(h->logicalIndex(h->count() - 1));
        p->drawRect(QRectF(0.5, option.rect.y() + 0.5, right - 1, option.rect.height() - 1));
    }
}

// FL-021 for one cell.
void FileListView::paintCell(QPainter *p, QRect cell, int row, int column, bool selected) const {
    const Lang &lang = Lang::instance();
    const QColor bg = lang.color(selected ? "colorListSelBG" : "colorListBG");
    const QColor grid = lang.color("colorListGrid");
    // Cell rectangles start one pixel left and up, so neighbouring outlines overlap.
    const QRect r = cell.adjusted(-1, -1, 0, 0);
    if (cell.width() < 8) {
        p->fillRect(cell, bg);
        if (!selected) p->fillRect(QRect(cell.left(), cell.top(), 1, cell.height()), lang.color("colorListBG"));
        return;
    }
    p->fillRect(r, bg);
    p->setPen(grid);
    p->setBrush(Qt::NoBrush);
    p->drawRect(QRectF(r).adjusted(0.5, 0.5, -0.5, -0.5));

    const FileListModel &m = *model_;
    const bool statusRow = m.hasStatus() && row == 0;
    QString s = statusRow && column != 0 ? QString() : m.text(row, column);
    const int icon = statusRow ? -1 : m.icon(row, column);
    int textLeft = r.left() + 1;
    if (icon >= 0) {
        p->drawPixmap(r.left() + 1, r.top() + 1, icons[icon]);
    }
    if (icon >= 0 || (column == 0 && m.fileRows().anyLocked)) textLeft += column == 3 ? 17 - 3 : 17;

    const QRect area(textLeft, r.top() + 1, r.right() - textLeft, r.height() - 2);
    int height = 14;
    QFont f = listFont(height);
    // Shrink the font while the text does not fit (not for the path column).
    if (column != 5) {
        while (QFontMetrics(f).horizontalAdvance(s) >= area.width() - 1 && height >= 11) {
            height -= 2;
            f = listFont(height);
        }
    }
    const QFontMetrics fm(f);
    if (column == 5 || fm.horizontalAdvance(s) >= area.width()) {
        int x = fm.horizontalAdvance("..");
        for (int i = 0; i < s.size(); ++i) {
            x += fm.horizontalAdvance(s[i]);
            if (x > area.width()) {
                s = s.left(i) + "..";
                break;
            }
        }
    }
    p->setFont(f);
    p->setPen(lang.color(selected ? "colorListSelText" : "colorListText"));
    const bool right = kRightAligned[column];
    const int w = fm.horizontalAdvance(s);
    const int x = right ? r.right() - w - 2 : area.left();
    p->drawText(QRect(x, area.top(), w + 2, area.height()), Qt::AlignLeft | Qt::AlignVCenter, s);
}
