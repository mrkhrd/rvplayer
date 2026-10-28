//! RV Player: dashcam video and GPS track viewer, a new implementation of Registrator Viewer.
//!
//! Usage: `rvplayer [folder]` — opens the folder's MOV/MP4 files on start.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use rv_app::RvApp;

fn main() -> eframe::Result {
    let folder = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RV Player")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RV Player",
        options,
        Box::new(|cc| Ok(Box::new(RvApp::new(cc, folder)))),
    )
}
