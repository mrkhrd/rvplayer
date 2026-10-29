// Process set-up shared by the application and the UI tests.
#pragma once

class QApplication;

namespace app {

// Before the QApplication exists: application attributes (shared OpenGL contexts for the map).
void beforeApplication();
// After it exists: style, metatypes, translations.
void afterApplication(QApplication &a);

}  // namespace app
