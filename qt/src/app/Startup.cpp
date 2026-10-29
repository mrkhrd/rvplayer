#include "Startup.h"

#include "i18n/Lang.h"
#include "map/MapView.h"
#include "vcl/Widgets.h"

#include <QApplication>

namespace app {

void beforeApplication() {
    MapView::registerScheme();
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);  // required by Qt WebEngine
}

void afterApplication(QApplication &) {
    QApplication::setApplicationName("RV Player");
    QApplication::setApplicationVersion("0.1.0");
    // The native Windows look of the original's themed VCL controls (not Qt's Windows 11 style).
    QApplication::setStyle("windowsvista");
    qRegisterMetaType<vcl::Event *>();
    Lang::instance().load();
}

}  // namespace app
