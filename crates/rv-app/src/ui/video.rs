//! Video area (`PanelVideo` of the original form). Shows a placeholder until a video backend
//! is chosen; the overlay (clock and speed) already follows the play position.

use egui::{Align2, Color32, FontId, Sense, Ui, Vec2};

use crate::app::RvApp;
use crate::ui::clock_at;

const KM_PER_MILE: f64 = 1.609_344;

pub fn show(app: &mut RvApp, ui: &mut Ui) {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::BLACK);

    let Some(file) = app.current() else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            app.strings.t("N_File_Open"),
            FontId::proportional(16.0),
            Color32::GRAY,
        );
        if response.clicked() {
            app.pick_folder();
        }
        return;
    };

    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{}\n{}", file.name, file.resolution().unwrap_or_default()),
        FontId::proportional(14.0),
        Color32::from_gray(110),
    );
    painter.text(
        rect.center() + Vec2::new(0.0, 36.0),
        Align2::CENTER_CENTER,
        "video playback: backend not connected yet",
        FontId::proportional(11.0),
        Color32::from_gray(80),
    );

    // Overlay like the original's subtitle line: date/time and speed at the play position.
    let mut overlay = Vec::new();
    if let Some(clock) = clock_at(file.start, app.playback.position) {
        overlay.push(clock.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Some(p) = file.track.position_at(app.playback.position) {
        let (k, unit) = if app.view.metric {
            (1.0, "km/h")
        } else {
            (1.0 / KM_PER_MILE, "mph")
        };
        overlay.push(format!("{:.0} {unit}", p.speed_kmh * k));
        overlay.push(format!("{:.6}, {:.6}", p.lat, p.lon));
    }
    let text = overlay.join("   ");
    let pos = rect.left_bottom() + Vec2::new(10.0, -10.0);
    let font = FontId::monospace(14.0);
    painter.text(
        pos + Vec2::splat(1.0),
        Align2::LEFT_BOTTOM,
        &text,
        font.clone(),
        Color32::BLACK,
    );
    painter.text(
        pos,
        Align2::LEFT_BOTTOM,
        &text,
        font,
        Color32::from_rgb(255, 230, 0),
    );

    if response.double_clicked() {
        let fullscreen = ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
    } else if response.clicked() {
        app.toggle_play();
    }
}
