// TFormFileProperty ("Property"): a two-column, copyable property sheet of one file-list entry
// and, for a group, of each of its files. One window per selected row, not modal.
// Spec: shell-settings-forms.md §6.7 (SF-FILEPROPERTY-001…011), file-list.md FL-060.
#pragma once

#include "core/Group.h"
#include "filelist/FileManager.h"
#include "vcl/Form.h"

class QTableWidget;

class FormFileProperty : public vcl::Form {
    Q_OBJECT
public:
    explicit FormFileProperty(QWidget *parent = nullptr);

    // SF-FILEPROPERTY-004/005: fills the sheet for one row and shows the window.
    void popup(const FileRow &row, const Group &group, const QString &durationText, double tzShift, bool british);

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormResize(vcl::Event *e);
    Q_INVOKABLE void FormClose(vcl::Event *e);
    Q_INVOKABLE void ButtonCopyClick(vcl::Event *e);

protected:
    bool eventFilter(QObject *watched, QEvent *ev) override;

private:
    void add(const QString &name, const QString &value, bool translate = true);
    void addFile(const GroupFile &f, const QString &prefix);
    void addStartFinish(TDateTime start, TDateTime finish, int durationState, const QString &indent);
    QString dateTime(TDateTime t) const;
    QTableWidget *grid() const;

    double tzShift = 0;
    bool british = false;
    int labelWidth = 0;
};
