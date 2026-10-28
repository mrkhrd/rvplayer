// The main window's file list (ListView: owner-data, owner-drawn TListView in the original).
// Spec: file-list.md §2.2–§2.4, FL-011, FL-020, FL-021.
#pragma once

#include "FileManager.h"

#include <QAbstractTableModel>
#include <QPixmap>
#include <QTreeView>

class FileListModel : public QAbstractTableModel {
    Q_OBJECT
public:
    using QAbstractTableModel::QAbstractTableModel;
    static constexpr int kColumns = 8;

    void setRows(const FileRows &rows);
    const FileRows &fileRows() const { return rows_; }
    bool hasStatus() const { return !rows_.statusKey.isEmpty(); }
    // The text of a cell (FL-020, §4.8) with the current units and time zone.
    QString text(int row, int column) const;
    // Icon index of a cell (§2.4): 0 key, 1 partial key, 2 video warning, 3 GPS found; -1 none.
    int icon(int row, int column) const;
    QString statusText() const;  // ". . . Preview 35% . . ."

    void setBritish(bool b) { british = b; }
    void setTimeZoneShift(double days) { tzShift = days; }
    void setHeaders(const QStringList &h);

    int rowCount(const QModelIndex &parent = {}) const override;
    int columnCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QVariant headerData(int section, Qt::Orientation o, int role) const override;

private:
    FileRows rows_;
    QStringList headers;
    bool british = false;
    double tzShift = 0;
};

class FileListView : public QTreeView {
    Q_OBJECT
public:
    explicit FileListView(QWidget *parent = nullptr);
    FileListModel *listModel() const { return model_; }
    // Selected row numbers, ascending.
    QList<int> selectedRows() const;
    void selectRow(int row);

protected:
    void drawRow(QPainter *p, const QStyleOptionViewItem &option, const QModelIndex &index) const override;
    void drawBranches(QPainter *, const QRect &, const QModelIndex &) const override {}

private:
    void paintCell(QPainter *p, QRect cell, int row, int column, bool selected) const;
    FileListModel *model_;
    QPixmap icons[4];
};
