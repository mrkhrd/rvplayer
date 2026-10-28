//! RV Player application: main window state and panels. The binary (`main.rs`) only starts it;
//! keeping the app in a library lets UI tests drive it through `egui_kittest`.

pub mod app;
pub mod i18n;
pub mod ui;

pub use app::RvApp;
