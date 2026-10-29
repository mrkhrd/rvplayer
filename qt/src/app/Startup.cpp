#include "Startup.h"

#include "i18n/Lang.h"
#include "rv_ffi.h"
#include "vcl/Widgets.h"

#include <QApplication>
#include <QDir>
#include <QIcon>
#include <QStandardPaths>

namespace app {

void beforeApplication() {
    // The map is an OpenGL widget; shared contexts let it move between windows (full screen).
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
}

void afterApplication(QApplication &a) {
    QApplication::setApplicationName("RV Player");
    QApplication::setApplicationVersion("0.1.0");
    // Windows without an icon of their own (the main window) show the application's.
    QApplication::setWindowIcon(QIcon(":/app/rvplayer.ico"));
    // The native Windows look of the original's themed VCL controls (not Qt's Windows 11 style).
    QApplication::setStyle("windowsvista");
    qRegisterMetaType<vcl::Event *>();
    Lang::instance().load();
    // What was read from videos (durations, GPS) is kept across runs: a folder read before
    // lists at once, a memory card included.
    const QString cache = QDir(QStandardPaths::writableLocation(QStandardPaths::CacheLocation)).filePath("file-info.json");
    rv_info_cache_set_file(QDir::toNativeSeparators(cache).toUtf8().constData());
    QObject::connect(&a, &QCoreApplication::aboutToQuit, [] { rv_info_cache_flush(); });
}

}  // namespace app
