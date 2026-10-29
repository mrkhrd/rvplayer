// TFormAbout: product name, version and the original author's details.
// Spec: shell-settings-forms.md §6.1. The DATAKAM logo, title and contacts are not shown
// (decisions.md: no brand items); the credits come from the non-brand translation entries.
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

private:
    bool aboutLifted = false;
};
