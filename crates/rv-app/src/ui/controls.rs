//! Control panel (`PanelControl` of the original form): speed, G-sensor and heading gauges,
//! playback buttons, play rate, volume and loop A-B.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

use crate::app::RvApp;
use crate::ui::icons::{self, Icon};
use crate::ui::{clock_at, format_duration, map_view::draw_heading_arrow};

const KM_PER_MILE: f64 = 1.609_344;
const PLAY_RATES: [f64; 7] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

pub fn show(app: &mut RvApp, ui: &mut Ui) {
    gauges(app, ui);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let s = &app.strings;
        let play_icon = if app.playback.playing {
            Icon::Pause
        } else {
            Icon::Play
        };
        let play = icons::button(ui, play_icon, s.t("N_Playback_Play"), true).clicked();
        let stop = icons::button(ui, Icon::Stop, s.t("N_Playback_Stop"), true).clicked();
        icons::button(ui, Icon::FramePrev, s.t("N_Playback_Frame_Prev"), false);
        icons::button(ui, Icon::FrameNext, s.t("N_Playback_Frame_Next"), false);
        egui::ComboBox::from_id_salt("PanelPlaySpeed")
            .width(60.0)
            .selected_text(format!("×{}", app.playback.rate))
            .show_ui(ui, |ui| {
                for r in PLAY_RATES {
                    ui.selectable_value(&mut app.playback.rate, r, format!("×{r}"));
                }
            })
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, "Play speed"));
        ui.add(egui::Slider::new(&mut app.playback.volume, 0.0..=1.0).show_value(false))
            .on_hover_text("🔊")
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, "Volume"));
        ui.checkbox(&mut app.playback.loop_ab, s.t("N_View_Time_LoopAB"));
        if play {
            app.toggle_play();
        }
        if stop {
            app.stop();
        }
    });
    ui.horizontal(|ui| {
        ui.monospace(format!(
            "{} / {}",
            format_duration(app.playback.position),
            format_duration(app.duration())
        ));
        if let Some(clock) = clock_at(app.current().and_then(|f| f.start), app.playback.position) {
            ui.monospace(clock.format("%Y-%m-%d %H:%M:%S").to_string());
        }
    });
}

fn gauges(app: &RvApp, ui: &mut Ui) {
    let size = ((ui.available_width() - 16.0) / 3.0).clamp(70.0, 120.0);
    let file = app.current();
    let gps = file.and_then(|f| f.track.position_at(app.playback.position));
    let gs = file.and_then(|f| f.track.gsensor_at(app.playback.position));
    let arrow = app.strings.color("colorSpeedArrow", Color32::RED);
    ui.horizontal(|ui| {
        let (metric, unit, max) = if app.view.metric {
            (1.0, "km/h", 180.0)
        } else {
            (1.0 / KM_PER_MILE, "mph", 120.0)
        };
        gauge(ui, size, |p, r, text| {
            speed_gauge(
                p,
                r,
                text,
                gps.map(|g| g.speed_kmh * metric),
                max,
                unit,
                arrow,
            )
        });
        gauge(ui, size, |p, r, text| {
            gsensor_gauge(p, r, text, gs.map(|g| (g.x, g.y, g.magnitude())), arrow)
        });
        gauge(ui, size, |p, r, text| {
            heading_gauge(p, r, text, gps.map(|g| g.course), arrow)
        });
    });
}

fn gauge(ui: &mut Ui, size: f32, draw: impl FnOnce(&egui::Painter, Rect, Color32)) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.circle(
        rect.center(),
        size / 2.0 - 2.0,
        visuals.extreme_bg_color,
        visuals.widgets.noninteractive.bg_stroke,
    );
    draw(&painter, rect, visuals.text_color());
}

fn speed_gauge(
    p: &egui::Painter,
    rect: Rect,
    text: Color32,
    speed: Option<f64>,
    max: f64,
    unit: &str,
    arrow: Color32,
) {
    let c = rect.center();
    let r = rect.width() / 2.0 - 8.0;
    // Dial from 225° (0) clockwise to -45° (max), like a car speedometer.
    let angle = |v: f64| (225.0 - 270.0 * (v / max).clamp(0.0, 1.0) as f32).to_radians();
    let at = |a: f32, rr: f32| c + Vec2::new(a.cos(), -a.sin()) * rr;
    for i in 0..=9 {
        let a = angle(max * f64::from(i) / 9.0);
        p.line_segment([at(a, r - 6.0), at(a, r)], Stroke::new(1.5, text));
    }
    if let Some(v) = speed {
        p.line_segment([c, at(angle(v), r - 4.0)], Stroke::new(3.0, arrow));
        p.text(
            c + Vec2::new(0.0, r * 0.45),
            Align2::CENTER_CENTER,
            format!("{v:.0}"),
            FontId::proportional(r * 0.4),
            text,
        );
    }
    p.circle_filled(c, 4.0, arrow);
    p.text(
        c + Vec2::new(0.0, r * 0.8),
        Align2::CENTER_CENTER,
        unit,
        FontId::proportional(10.0),
        text,
    );
}

fn gsensor_gauge(
    p: &egui::Painter,
    rect: Rect,
    text: Color32,
    g: Option<(f64, f64, f64)>,
    arrow: Color32,
) {
    let c = rect.center();
    let r = rect.width() / 2.0 - 8.0;
    let stroke = Stroke::new(1.0, text.gamma_multiply(0.4));
    p.line_segment([c - Vec2::X * r, c + Vec2::X * r], stroke);
    p.line_segment([c - Vec2::Y * r, c + Vec2::Y * r], stroke);
    p.circle_stroke(c, r * 0.5, stroke);
    if let Some((x, y, total)) = g {
        // ±1 g fills the dial; x = left/right, y = forward/backward.
        let dot = c + Vec2::new(x.clamp(-1.0, 1.0) as f32, -(y.clamp(-1.0, 1.0) as f32)) * r;
        p.circle_filled(dot, 5.0, arrow);
        p.text(
            c + Vec2::new(0.0, r * 0.75),
            Align2::CENTER_CENTER,
            format!("{total:.2} g"),
            FontId::proportional(10.0),
            text,
        );
    }
}

fn heading_gauge(
    p: &egui::Painter,
    rect: Rect,
    text: Color32,
    course: Option<f64>,
    arrow: Color32,
) {
    let c = rect.center();
    let r = rect.width() / 2.0 - 8.0;
    for (label, deg) in [("N", 0.0f32), ("E", 90.0), ("S", 180.0), ("W", 270.0)] {
        let a = deg.to_radians();
        let pos: Pos2 = c + Vec2::new(a.sin(), -a.cos()) * (r - 7.0);
        p.text(
            pos,
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            text,
        );
    }
    if let Some(course) = course {
        draw_heading_arrow(p, c, course, arrow);
        let label_pos = c + Vec2::new(0.0, r * 0.55);
        p.text(
            label_pos,
            Align2::CENTER_CENTER,
            format!("{course:.0}°"),
            FontId::proportional(10.0),
            text,
        );
    } else {
        p.add(Shape::circle_stroke(c, 3.0, Stroke::new(1.0, text)));
    }
}
