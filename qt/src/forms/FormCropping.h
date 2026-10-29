// TFormCropping ("Cutting"): a list of fragments (start/end/speed per file) joined into one video
// without recompression, optionally with the GPS/G-sensor track.
// Spec: shell-settings-forms.md §6.5 (SF-CROPPING-001…036).
#pragma once

#include "FormCroppingItem.h"
#include "vcl/Form.h"

#include <QPointer>
#include <QVector>

class FormSetupSubtitleTemplate;
class QTreeWidget;

class FormCropping : public vcl::Form {
    Q_OBJECT
public:
    explicit FormCropping(QWidget *parent = nullptr);

    void setTimeZoneShift(double days) { tzShift = days; }
    void setBritish(bool on) { british = on; }
    // SF-CROPPING-010/011: appends one whole file or part of it; false when it cannot be read.
    bool addFile(const QString &path, double p1 = 0, double p2 = 1, TDateTime start = 0, const QString &trackSource = {},
                 double fps = 0, int gop = 0);
    // The output file name proposed by the main window (SF-CROPPING-035).
    void proposeFileName(const QString &path);
    bool hasFileName() const;
    int count() const { return int(items.size()); }
    const QVector<CropItem> &cropItems() const { return items; }

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormShow(vcl::Event *e);
    Q_INVOKABLE void FormClose(vcl::Event *e);
    Q_INVOKABLE void FormResize(vcl::Event *e);
    Q_INVOKABLE void ListViewData(vcl::Event *e) {}
    Q_INVOKABLE void SpeedButtonSelectFileClick(vcl::Event *e);
    Q_INVOKABLE void ButtonStartClick(vcl::Event *e);
    Q_INVOKABLE void ButtonCancelClick(vcl::Event *e);
    Q_INVOKABLE void ButtonSubtitleClick(vcl::Event *e);
    Q_INVOKABLE void PopupMenuCropListPopup(vcl::Event *e);
    Q_INVOKABLE void N_EditClick(vcl::Event *e);
    Q_INVOKABLE void N_SpeedClick(vcl::Event *e);
    Q_INVOKABLE void N_NoIntersectionClick(vcl::Event *e);
    Q_INVOKABLE void N_UpClick(vcl::Event *e);
    Q_INVOKABLE void N_DownClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_AddClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_DelClick(vcl::Event *e);
    Q_INVOKABLE void N_FileList_ClearClick(vcl::Event *e);

signals:
    // SF-CROPPING-024/030: the main window hides while exporting, then lists the result.
    void exportStarted();
    void exportFinished(const QString &output);

protected:
    void dragEnterEvent(QDragEnterEvent *e) override;
    void dropEvent(QDropEvent *e) override;

private:
    QTreeWidget *list() const;
    QList<int> selectedRows() const;
    void refreshList();
    void calcTotal();                     // SF-CROPPING-012
    void setEnables(bool enabled);        // SF-CROPPING-006
    void drawProgress(double seconds, double fraction);  // SF-CROPPING-031
    void selectRow(int row);
    void moveSelected(int step);

    QVector<CropItem> items;
    double tzShift = 0;
    bool british = false;  // miles in subtitles
    QPointer<FormSetupSubtitleTemplate> subtitleForm;
    int currentRow = -1;
    double progress = 0;
    bool running = false;
};
