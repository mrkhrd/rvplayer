// Process set-up shared by the application and the UI tests.
#pragma once

class QApplication;

namespace app {

// Before the QApplication exists: URL schemes and attributes Qt WebEngine needs.
void beforeApplication();
// After it exists: style, metatypes, translations.
void afterApplication(QApplication &a);

}  // namespace app
