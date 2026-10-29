#include "Startup.h"

#include "i18n/Lang.h"
#include "vcl/Widgets.h"

#include <QApplication>
#include <QIcon>

namespace app {

void beforeApplication() {
    // The map is an OpenGL widget; shared contexts let it move between windows (full screen).
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
}

void afterApplication(QApplication &) {
    QApplication::setApplicationName("RV Player");
    QApplication::setApplicationVersion("0.1.0");
    // Windows without an icon of their own (the main window) show the application's.
    QApplication::setWindowIcon(QIcon(":/app/rvplayer.ico"));
    // The native Windows look of the original's themed VCL controls (not Qt's Windows 11 style).
    QApplication::setStyle("windowsvista");
    qRegisterMetaType<vcl::Event *>();
    Lang::instance().load();
}

}  // namespace app
