// TFormCroppingItem ("Cutting. Edit"): start, end and speed of one fragment of the Cutting list.
// Spec: shell-settings-forms.md §6.6 (SF-CROPPINGITEM-001…012).
#pragma once

#include "core/Group.h"
#include "vcl/Form.h"

#include <QSize>

// One fragment to export (TCropItem).
struct CropItem {
    QString path;
    qint64 durationMs = 0;
    double p1 = 0, p2 = 1;     // start and end as fractions of the file
    TDateTime start = 0;       // wall clock of the file start
    double speed = 1;          // > 1 faster, < 1 slower
    bool speedUpFps = true;    // faster: keep all frames (true) or key frames only (false)
    double fps = 30;
    int gop = 0;               // frames per key frame, 0 = unknown
    QSize frameSize;
    QString trackSource;       // GPS file for the meta track; empty = the video's own
    double outputSeconds() const { return durationMs / 1000.0 * (p2 - p1) / speed; }
};

class FormCroppingItem : public vcl::Form {
    Q_OBJECT
public:
    explicit FormCroppingItem(QWidget *parent = nullptr);

    // SF-CROPPINGITEM-003/004: edits `item` modally; true when OK was pressed.
    bool edit(CropItem &item, double tzShift);

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void ButtonOkClick(vcl::Event *e);
    Q_INVOKABLE void ButtonCancelClick(vcl::Event *e);
    Q_INVOKABLE void EditP1Change(vcl::Event *e);
    Q_INVOKABLE void EditP2Change(vcl::Event *e);
    Q_INVOKABLE void EditTime1Change(vcl::Event *e);
    Q_INVOKABLE void EditTime2Change(vcl::Event *e);
    Q_INVOKABLE void EditSpeedChange(vcl::Event *e);
    Q_INVOKABLE void TrackBarSpeedChange(vcl::Event *e);
    Q_INVOKABLE void SUM_FPS_CaptionClick(vcl::Event *e);
    Q_INVOKABLE void SUM_KeyFrames_CaptionClick(vcl::Event *e);

    // SF-CROPPINGITEM-005/006.
    static QString speedText(double speed);
    static double parseSpeed(const QString &text);  // -1 when invalid

private:
    void buildPresets();
    void pChanged(int which);
    void timeChanged(int which);
    void layoutControls();

    CropItem *item = nullptr;
    double tzShift = 0;
    QVector<double> presets;
    bool changing = false;  // the edits update each other once, not back and forth
    bool shifted = false;   // rows moved right once for long captions
};

// "H:MM:SS" of a number of seconds, and back (0 on bad input).
QString durationText(double seconds);
double parseDuration(const QString &text, bool *ok = nullptr);
