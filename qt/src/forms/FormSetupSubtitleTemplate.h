// TFormSetupSubtitleTemplate ("Setup Template for Subtitles"): the text of each SRT cue an
// export writes, with placeholders for the time and GPS values, and a live example.
// Spec: playback-video.md PV-092, shell-settings-forms.md SF-CROPPING-021 and the
// `subtitleTemplate` setting (UnitFormSetupSubtitleTemplate).
#pragma once

#include "core/GpsFiles.h"
#include "vcl/Form.h"

class FormSetupSubtitleTemplate : public vcl::Form {
    Q_OBJECT
public:
    explicit FormSetupSubtitleTemplate(QWidget *parent = nullptr);

    void setSettings(const gpsfiles::Settings &s) { settings = s; }

    Q_INVOKABLE void FormResize(vcl::Event *e);
    Q_INVOKABLE void FormShow(vcl::Event *e);
    Q_INVOKABLE void ButtonOkClick(vcl::Event *e);
    Q_INVOKABLE void CheckBoxDefaultClick(vcl::Event *e);
    Q_INVOKABLE void MemoTemplateKeyUp(vcl::Event *e);

private:
    void translate();
    void generate();  // the example under the template, for a random fix
    gpsfiles::Settings settings;
};
