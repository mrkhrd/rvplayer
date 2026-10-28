//! Graphs and seek bar (`PanelBottom` of the original form: zoom row + `ImageSeek`).

use egui::{Color32, Ui};
use egui_plot::{Line, Plot, PlotPoints, VLine};

use crate::app::{GraphKind, RvApp};
use crate::ui::format_duration;

const MAX_TIME_ZOOM: u32 = 4;
const KM_PER_MILE: f64 = 1.609_344;
const FEET_PER_METER: f64 = 3.280_84;

pub fn show(app: &mut RvApp, ui: &mut Ui) {
    let duration = app.duration();
    ui.horizontal(|ui| {
        let s = &app.strings;
        if ui
            .small_button("−")
            .on_hover_text(s.t("N_View_Time_ZoomOut"))
            .clicked()
        {
            app.view.time_zoom = app.view.time_zoom.saturating_sub(1).max(1);
        }
        if ui
            .small_button("+")
            .on_hover_text(s.t("N_View_Time_ZoomIn"))
            .clicked()
        {
            app.view.time_zoom = (app.view.time_zoom + 1).min(MAX_TIME_ZOOM);
        }
        ui.add(egui::Slider::new(&mut app.view.time_zoom, 1..=MAX_TIME_ZOOM).show_value(false));
        ui.label(format!(
            "{} / {}",
            format_duration(app.playback.position),
            format_duration(duration)
        ));
    });

    let Some(file) = app.current() else {
        return;
    };
    let kinds: Vec<GraphKind> = app.view.graphs.clone();
    if kinds.is_empty() || file.track.is_empty() {
        ui.centered_and_justified(|ui| ui.weak("—"));
        return;
    }

    // Visible time window: whole file at zoom 1, halved per step, following the play position.
    let window = duration / f64::from(1u32 << (app.view.time_zoom - 1));
    let lo = (app.playback.position - window / 2.0).clamp(0.0, (duration - window).max(0.0));
    let height = ((ui.available_height() - 4.0) / kinds.len() as f32).max(40.0);

    let mut seek_to = None;
    for kind in kinds {
        let series = series(app, kind);
        let color = app
            .strings
            .color(&format!("colorGraph_{}", kind.letter()), Color32::BLUE);
        let name = app
            .strings
            .t(&format!("N_View_Graphics_{}", kind.letter()))
            .to_string();
        let marks = [app.playback.mark_a, app.playback.mark_b];
        let position = app.playback.position;
        let response = Plot::new(("graph", kind.letter()))
            .height(height)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .show_x(false)
            .y_axis_min_width(36.0)
            .x_axis_formatter(|mark, _| format_duration(mark.value))
            .show(ui, |plot| {
                plot.set_auto_bounds([false, true]);
                plot.set_plot_bounds_x(lo..=lo + window);
                plot.line(Line::new(name, PlotPoints::from(series)).color(color));
                for t in marks.into_iter().flatten() {
                    plot.vline(VLine::new("A-B", t).color(Color32::from_rgb(255, 170, 0)));
                }
                plot.vline(
                    VLine::new("position", position)
                        .color(Color32::RED)
                        .width(1.5),
                );
                plot.pointer_coordinate()
            });
        if (response.response.clicked() || response.response.dragged())
            && let Some(p) = response.inner
        {
            seek_to = Some(p.x);
        }
    }
    if let Some(t) = seek_to {
        app.seek(t);
    }
}

fn series(app: &RvApp, kind: GraphKind) -> Vec<[f64; 2]> {
    let Some(f) = app.current() else {
        return Vec::new();
    };
    let metric = app.view.metric;
    let t = &f.track;
    match kind {
        GraphKind::Speed => {
            let k = if metric { 1.0 } else { 1.0 / KM_PER_MILE };
            t.gps.iter().map(|p| [p.time, p.speed_kmh * k]).collect()
        }
        GraphKind::Altitude => {
            let k = if metric { 1.0 } else { FEET_PER_METER };
            t.gps.iter().map(|p| [p.time, p.altitude * k]).collect()
        }
        GraphKind::TotalVector => t.gsensor.iter().map(|g| [g.time, g.magnitude()]).collect(),
        GraphKind::X => t.gsensor.iter().map(|g| [g.time, g.x]).collect(),
        GraphKind::Y => t.gsensor.iter().map(|g| [g.time, g.y]).collect(),
        GraphKind::Z => t.gsensor.iter().map(|g| [g.time, g.z]).collect(),
    }
}
