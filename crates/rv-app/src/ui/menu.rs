//! Main menu (`MainMenu` of the original form) and the About window.

use egui::{Button, Ui};

use crate::app::{GraphKind, MapSource, RvApp};
use crate::i18n::{Language, Strings};

fn item(ui: &mut Ui, s: &Strings, key: &str, shortcut: &str) -> bool {
    ui.add(Button::new(s.t(key)).shortcut_text(shortcut))
        .clicked()
}

/// Features of the original menu that are not implemented yet; shown disabled.
fn todo_item(ui: &mut Ui, s: &Strings, key: &str, shortcut: &str) {
    ui.add_enabled(false, Button::new(s.t(key)).shortcut_text(shortcut));
}

pub fn show(app: &mut RvApp, ui: &mut Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        file_menu(app, ui);
        view_menu(app, ui);
        playback_menu(app, ui);
        let s = &app.strings;
        ui.menu_button(s.t("N_VideoZoom"), |ui| {
            for key in [
                "N_VideoZoom_Letterbox",
                "N_VideoZoom_PanAndScan",
                "N_VideoZoom_Stretch",
                "N_VideoZoom_ZoomIn",
                "N_VideoZoom_ZoomOut",
                "N_VideoZoom_FlipV",
                "N_VideoZoom_FlipH",
            ] {
                todo_item(ui, s, key, "");
            }
        });
        ui.menu_button(label(app, "N_Help"), |ui| {
            if item(ui, &app.strings, "N_Help_About", "") {
                app.show_about = true;
            }
        });
    });
}

/// Owned menu caption, so the menu body can borrow `app` mutably.
fn label(app: &RvApp, key: &str) -> String {
    app.strings.t(key).to_string()
}

fn file_menu(app: &mut RvApp, ui: &mut Ui) {
    ui.menu_button(label(app, "N_File"), |ui| {
        if item(ui, &app.strings, "N_File_Open", "Ctrl+O") {
            app.pick_folder();
        }
        if item(ui, &app.strings, "N_FileList_Refresh", "Ctrl+R") {
            app.refresh();
        }
        if item(ui, &app.strings, "N_FileList_Clear", "Ctrl+N") {
            app.clear_files();
        }
        ui.separator();
        todo_item(ui, &app.strings, "N_File_Save_Video", "Ctrl+S");
        todo_item(ui, &app.strings, "N_File_Repair_Video", "");
        todo_item(ui, &app.strings, "N_File_Convert_GPS", "Ctrl+G");
        ui.separator();
        if item(ui, &app.strings, "N_File_Exit", "Alt+F4") {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
}

fn view_menu(app: &mut RvApp, ui: &mut Ui) {
    ui.menu_button(label(app, "N_View"), |ui| {
        ui.menu_button(label(app, "N_View_Language"), |ui| {
            for lang in Language::ALL {
                let name = Strings::load(lang).native_name().to_string();
                if ui.radio(app.strings.language == lang, name).clicked() {
                    app.strings = Strings::load(lang);
                }
            }
        });
        ui.separator();
        let RvApp {
            strings: s,
            view,
            playback,
            ..
        } = app;
        ui.checkbox(&mut view.show_file_list, s.t("N_View_FileList"));
        ui.checkbox(&mut view.show_controls, s.t("N_View_ControlPanel"));
        ui.menu_button(s.t("N_View_Maps"), |ui| {
            ui.radio_value(
                &mut view.map,
                MapSource::OpenStreetMap,
                s.t("N_View_OpenStreetMaps"),
            );
            ui.radio_value(&mut view.map, MapSource::None, s.t("N_View_NoMaps"));
        });
        ui.menu_button(s.t("N_View_Graphics"), |ui| {
            for kind in GraphKind::ALL {
                let mut on = view.graphs.contains(&kind);
                if ui
                    .checkbox(&mut on, s.t(&format!("N_View_Graphics_{}", kind.letter())))
                    .changed()
                {
                    view.toggle_graph(kind);
                }
            }
            ui.separator();
            ui.checkbox(&mut view.show_graphs, s.t("N_View_Graphics_Show"));
        });
        ui.menu_button(s.t("N_View_Measurement"), |ui| {
            ui.radio_value(&mut view.metric, true, s.t("N_View_Measu_Metric"));
            ui.radio_value(&mut view.metric, false, s.t("N_View_Measu_British"));
        });
        ui.menu_button(s.t("N_View_Time"), |ui| {
            if item(ui, s, "N_View_Time_SetA", "Ctrl+Home") {
                playback.mark_a = Some(playback.position);
            }
            if item(ui, s, "N_View_Time_SetB", "Ctrl+End") {
                playback.mark_b = Some(playback.position);
            }
            ui.checkbox(&mut playback.loop_ab, s.t("N_View_Time_LoopAB"));
        });
    });
}

fn playback_menu(app: &mut RvApp, ui: &mut Ui) {
    ui.menu_button(label(app, "N_Playback"), |ui| {
        if item(ui, &app.strings, "N_Playback_Play", "Space") {
            app.toggle_play();
        }
        if item(ui, &app.strings, "N_Playback_Stop", "Esc") {
            app.stop();
        }
        ui.separator();
        if item(ui, &app.strings, "N_Playback_Left", "←") {
            app.seek(app.playback.position - 5.0);
        }
        if item(ui, &app.strings, "N_Playback_Right", "→") {
            app.seek(app.playback.position + 5.0);
        }
        todo_item(ui, &app.strings, "N_Playback_Frame_Prev", "B");
        todo_item(ui, &app.strings, "N_Playback_Frame_Next", "N");
        ui.separator();
        todo_item(ui, &app.strings, "N_Playback_Save_Frame", "Alt+S");
    });
}

pub fn about_window(app: &mut RvApp, ctx: &egui::Context) {
    egui::Window::new(app.strings.t("N_Help_About"))
        .open(&mut app.show_about)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("RV Player");
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.add_space(6.0);
            ui.label(
                "A new implementation of Registrator Viewer, the dashcam video and GPS viewer",
            );
            ui.label("created by Vadim Andreevich Kozlov (2011–2015).");
            ui.add_space(6.0);
            ui.hyperlink_to(
                "Map data © OpenStreetMap contributors",
                "https://www.openstreetmap.org/copyright",
            );
        });
}
