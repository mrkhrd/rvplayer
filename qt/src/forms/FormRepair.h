// TFormRepair ("Repairing"): rebuilds the header of a recording that was never finished (power
// cut), using a good file from the same camera as reference. Only the Fast method (in place) is
// offered; the original's Full method was an embedded third-party program (decisions.md Q-5).
// Spec: shell-settings-forms.md §6.8 (SF-REPAIR-001…015).
#pragma once

#include "core/Group.h"
#include "core/RepairEngine.h"
#include "vcl/Form.h"

#include <QDateTime>

class FormRepair : public vcl::Form {
    Q_OBJECT
public:
    explicit FormRepair(QWidget *parent = nullptr);
    ~FormRepair() override;

    // SF-REPAIR-003: opens with the broken file and a reference picked from `candidates`
    // (files of the list with an exact duration).
    void open(const QString &broken, const QVector<GroupFile> &candidates, const GroupFile *brokenFile);

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormResize(vcl::Event *e);
    Q_INVOKABLE void EditBrokenChange(vcl::Event *e);
    Q_INVOKABLE void ButtonFileBorkenClick(vcl::Event *e);
    Q_INVOKABLE void ButtonFileResultClick(vcl::Event *e) {}
    Q_INVOKABLE void LabelCaptionFastClick(vcl::Event *e);
    Q_INVOKABLE void LabelCaptionFullClick(vcl::Event *e) {}
    Q_INVOKABLE void ButtonRepairClick(vcl::Event *e);
    Q_INVOKABLE void TimerConsoleTimer(vcl::Event *e);

signals:
    // SF-REPAIR-015: the repaired file goes into the main file list.
    void repaired(const QString &path);

protected:
    void closeEvent(QCloseEvent *e) override;

private:
    void setEnable(bool idle);  // SF-REPAIR-004
    std::unique_ptr<repair::Job> job;
    QString target;
    QDateTime created, written;
};
