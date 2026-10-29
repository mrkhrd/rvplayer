// TFormGPSLog ("GPS Log Converter"): a list of GPS logs with points, duration and length,
// previewed on the main map, converted into the checked export formats.
// Spec: graphs-map-layout.md §3.13 (GM-190…GM-192), file-list.md FL-064,
// shell-settings-forms.md MN-012 (UnitFormGPSLog).
#pragma once

#include "core/GpsFiles.h"
#include "vcl/Form.h"

class QTimer;
class QTreeWidget;
class QTreeWidgetItem;

class FormGPSLog : public vcl::Form {
    Q_OBJECT
public:
    explicit FormGPSLog(QWidget *parent = nullptr);

    // Popup L111–L142: fills an empty list with `files`, lists the export formats once and
    // shows the window next to `owner`.
    void popup(const QStringList &files, QWidget *owner, bool british, double tzShift);
    // FL-064: only `paths` ticked and selected.
    void markOnly(const QStringList &paths);
    QStringList checkedFiles() const;

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormShow(vcl::Event *e);
    Q_INVOKABLE void FormClose(vcl::Event *e);
    Q_INVOKABLE void ButtonConvertClick(vcl::Event *e);
    Q_INVOKABLE void CheckBoxAllClick(vcl::Event *e);
    Q_INVOKABLE void ListViewFilesClick(vcl::Event *e);
    Q_INVOKABLE void ListViewFilesSelectItem(vcl::Event *e);
    Q_INVOKABLE void TimerDrawRouteTimer(vcl::Event *e);
    Q_INVOKABLE void N_FileList_ShowExplorerClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_AddClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_DelClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_ClearClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_DelDiskClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_DelDiskLogsClick(vcl::Event *e);
    Q_INVOKABLE void N_SpaceClick(vcl::Event *e);

signals:
    // GM-191: draw these tracks on the main map; selected ones stand out.
    void previewFiles(const QStringList &paths, const QList<bool> &selected);
    // L415: the logs were deleted from disk; the main list should forget them.
    void logsDeleted();

protected:
    void dragEnterEvent(QDragEnterEvent *e) override;
    void dropEvent(QDropEvent *e) override;

private:
    QTreeWidget *list() const;
    QTreeWidgetItem *addFile(const QString &path);
    void afterAdd(QTreeWidgetItem *it);
    void fillNext();  // OnTimer: the statistics of one more row
    QString convert(const QVector<gpsfiles::Source> &sources, QString path);
    void deleteWithConversions(const QString &path);  // DelIt

    QTimer *statsTimer = nullptr;
    bool british = false;
    double tzShift = 0;
};
