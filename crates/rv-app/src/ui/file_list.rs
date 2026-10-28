//! File list (`PanelFileListMenu` toolbar + `ListView` of the original form).

use egui::{Color32, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

use crate::app::RvApp;
use crate::ui::format_duration;

const KM_PER_MILE: f64 = 1.609_344;

pub fn show(app: &mut RvApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let s = &app.strings;
        let open = ui.button("📂").on_hover_text(s.t("N_File_Open")).clicked();
        let refresh = ui
            .button("⟳")
            .on_hover_text(s.t("N_FileList_Refresh"))
            .clicked();
        let clear = ui
            .button("🗑")
            .on_hover_text(s.t("N_FileList_Clear"))
            .clicked();
        if app.loading.is_some() {
            ui.spinner();
        }
        ui.label(&app.status);
        if open {
            app.pick_folder();
        }
        if refresh {
            app.refresh();
        }
        if clear {
            app.clear_files();
        }
    });

    let s = &app.strings;
    let metric = app.view.metric;
    let headers = [
        s.t("Name"),
        s.t("Duration"),
        s.t("DateTime"),
        if metric { "km" } else { "mi" },
        if metric { "km/h" } else { "mph" },
        "File Location",
        s.t("Mb"),
        "Resolution",
    ];
    let mut clicked = None;
    // Selectable cell text would take the click for text selection instead of selecting the row.
    ui.style_mut().interaction.selectable_labels = false;
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .column(Column::initial(190.0).at_least(60.0).clip(true))
        .column(Column::initial(55.0))
        .column(Column::initial(125.0))
        .column(Column::initial(50.0))
        .column(Column::initial(50.0))
        .column(Column::initial(120.0).clip(true))
        .column(Column::initial(55.0))
        .column(Column::remainder().at_least(70.0))
        .header(20.0, |mut header| {
            for title in headers {
                header.col(|ui| {
                    ui.strong(title);
                });
            }
        })
        .body(|body| {
            body.rows(18.0, app.files.len(), |mut row| {
                let i = row.index();
                let f = &app.files[i];
                row.set_selected(app.selected == Some(i));
                let has_gps = !f.track.gps.is_empty();
                let per_unit = if metric { 1.0 } else { 1.0 / KM_PER_MILE };
                row.col(|ui| {
                    let name = RichText::new(&f.name);
                    match &f.error {
                        Some(e) => {
                            ui.label(name.color(Color32::RED)).on_hover_text(e);
                        }
                        None => {
                            ui.label(name)
                                .on_hover_text(f.camera.as_deref().unwrap_or_default());
                        }
                    }
                });
                row.col(|ui| {
                    ui.label(format_duration(f.duration));
                });
                row.col(|ui| {
                    ui.label(
                        f.start
                            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_default(),
                    );
                });
                row.col(|ui| {
                    if has_gps {
                        ui.label(format!("{:.2}", f.distance_km() * per_unit));
                    }
                });
                row.col(|ui| {
                    if has_gps {
                        ui.label(format!("{:.0}", f.avg_speed_kmh() * per_unit));
                    }
                });
                row.col(|ui| {
                    ui.label(f.folder());
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}", f.size as f64 / (1024.0 * 1024.0)));
                });
                row.col(|ui| {
                    ui.label(f.resolution().unwrap_or_default());
                });
                if row.response().clicked() {
                    clicked = Some(i);
                }
            });
        });
    if let Some(i) = clicked {
        app.select(i);
    }
}
