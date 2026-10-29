// TFormAbout: this program (RV Player: icon, name, version, project page) and the program it is
// based on, Registrator Viewer, with its author. Spec: shell-settings-forms.md §6.1; the DATAKAM
// logo and contacts are not shown (decisions.md: no brand items, Q-29).
#pragma once

#include "vcl/Form.h"

class FormAbout : public vcl::Form {
    Q_OBJECT
public:
    explicit FormAbout(QWidget *parent = nullptr);

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormShow(vcl::Event *e);
    Q_INVOKABLE void LabelMailClick(vcl::Event *e);
    Q_INVOKABLE void LabelSiteClick(vcl::Event *e);
    Q_INVOKABLE void LabelMailMouseEnter(vcl::Event *e);
    Q_INVOKABLE void LabelMailMouseLeave(vcl::Event *e);
};
