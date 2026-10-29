// TFormGroup ("Grouping"): pick list rows with check boxes and join them into one item.
// While ticking, the routes of the ticked rows are previewed on the main map.
// Spec: file-list.md FL-055 (UnitFormGroup: Popup, ButtonGroupClick, ListViewSelectItem,
// TimerDrawRouteTimer, FormKeyDown).
#pragma once

#include "vcl/Form.h"

#include <QSet>

class FileListView;
class QTreeWidget;

class FormGroup : public vcl::Form {
    Q_OBJECT
public:
    explicit FormGroup(QWidget *parent = nullptr);

    // Fills the list as a copy of the main list: its selected rows ticked, the current one also
    // selected and focused.
    void setRows(const FileListView *list);
    // Rows ticked when Group was pressed.
    QList<int> tickedRows() const;

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormShow(vcl::Event *e);
    Q_INVOKABLE void FormResize(vcl::Event *e);
    Q_INVOKABLE void FormKeyDown(vcl::Event *e);
    Q_INVOKABLE void ButtonGroupClick(vcl::Event *e);
    Q_INVOKABLE void ButtonCloseClick(vcl::Event *e);
    Q_INVOKABLE void ListViewClick(vcl::Event *e);
    Q_INVOKABLE void ListViewSelectItem(vcl::Event *e);
    Q_INVOKABLE void TimerDrawRouteTimer(vcl::Event *e);

signals:
    // Draw the routes of `rows` on the main map; `highlighted` (selected ones) stand out.
    void previewRoutes(const QList<int> &rows, const QSet<int> &highlighted);

private:
    QTreeWidget *list() const;
    void restartPreview();
};
