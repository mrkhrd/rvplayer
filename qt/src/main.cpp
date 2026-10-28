// RV Player (Qt): a faithful reimplementation of Registrator Viewer's windows on top of the
// Rust core (crates/rv-ffi).

#include "forms/MainForm.h"
#include "i18n/Lang.h"
#include "map/MapView.h"

#include <QApplication>
#include <QMessageBox>
#include <cstdio>

int main(int argc, char *argv[]) {
    MapView::registerScheme();
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);  // required by Qt WebEngine
    QApplication app(argc, argv);
    QApplication::setApplicationName("RV Player");
    // The native Windows look of the original's themed VCL controls (not Qt's Windows 11 style).
    QApplication::setStyle("windowsvista");
    qRegisterMetaType<vcl::Event *>();

    Lang::instance().load();

    MainForm form;
    if (!form.load(":/original/forms/TFORMMAIN.dfm")) {
        QMessageBox::critical(nullptr, "RV Player", "Cannot load the main form: " + form.loadError());
        return 1;
    }
    const QStringList todo = form.unimplementedHandlers();
    if (qEnvironmentVariableIsSet("RV_LIST_HANDLERS")) {
        for (const QString &h : todo) std::fprintf(stderr, "unimplemented: %s\n", qPrintable(h));
    }
    std::fprintf(stderr, "main form: %lld handlers not implemented yet\n", static_cast<long long>(todo.size()));

    form.showMaximized();
    return QApplication::exec();
}
