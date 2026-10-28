//! Vector icons for buttons (fonts do not reliably contain media-control glyphs).

use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    FramePrev,
    FrameNext,
}

/// Square button with a drawn icon; `tooltip` doubles as the accessibility label.
pub fn button(ui: &mut Ui, icon: Icon, tooltip: &str, enabled: bool) -> Response {
    let size = Vec2::splat(ui.spacing().interact_size.y + 4.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let response = response.on_hover_text(tooltip);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tooltip));
    if ui.is_rect_visible(rect) {
        let visuals = if enabled {
            ui.style().interact(&response)
        } else {
            &ui.visuals().widgets.noninteractive
        };
        ui.painter().rect(
            rect,
            visuals.corner_radius,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let color = if enabled {
            visuals.fg_stroke.color
        } else {
            ui.visuals().weak_text_color()
        };
        paint(ui, icon, rect.shrink(rect.width() * 0.28), color);
    }
    response
}

fn paint(ui: &Ui, icon: Icon, r: Rect, c: Color32) {
    let p = ui.painter();
    let tri = |a: Pos2, b: Pos2, d: Pos2| Shape::convex_polygon(vec![a, b, d], c, Stroke::NONE);
    match icon {
        Icon::Play => {
            p.add(tri(r.left_top(), r.right_center(), r.left_bottom()));
        }
        Icon::Pause => {
            let w = r.width() * 0.34;
            p.rect_filled(Rect::from_min_size(r.min, Vec2::new(w, r.height())), 0.0, c);
            p.rect_filled(
                Rect::from_min_size(Pos2::new(r.max.x - w, r.min.y), Vec2::new(w, r.height())),
                0.0,
                c,
            );
        }
        Icon::Stop => {
            p.rect_filled(r, 1.0, c);
        }
        Icon::FramePrev => {
            let bar = r.width() * 0.2;
            p.rect_filled(
                Rect::from_min_size(r.min, Vec2::new(bar, r.height())),
                0.0,
                c,
            );
            p.add(tri(
                r.right_top(),
                Pos2::new(r.min.x + bar, r.center().y),
                r.right_bottom(),
            ));
        }
        Icon::FrameNext => {
            let bar = r.width() * 0.2;
            p.rect_filled(
                Rect::from_min_size(
                    Pos2::new(r.max.x - bar, r.min.y),
                    Vec2::new(bar, r.height()),
                ),
                0.0,
                c,
            );
            p.add(tri(
                r.left_top(),
                Pos2::new(r.max.x - bar, r.center().y),
                r.left_bottom(),
            ));
        }
    }
}
