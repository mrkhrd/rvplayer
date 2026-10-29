// TFormWhatsNew: the original program's release notes, read-only.
// Spec: shell-settings-forms.md §6.2 (SF-WHATSNEW-001…004).
#pragma once

#include "vcl/Form.h"

class FormWhatsNew : public vcl::Form {
    Q_OBJECT
public:
    explicit FormWhatsNew(QWidget *parent = nullptr);

    Q_INVOKABLE void FormCreate(vcl::Event *e);
    Q_INVOKABLE void FormShow(vcl::Event *e);
};
