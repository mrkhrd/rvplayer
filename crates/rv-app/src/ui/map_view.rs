//! Map with the GPS track (replaces the original `WebBrowser` + Leaflet page).

use egui::{Align2, Color32, FontId, Pos2, Response, Shape, Stroke, Ui, Vec2};
use rv_core::GpsPoint;
use walkers::{Map, MapMemory, Plugin, Projector, Tiles, lon_lat};

use crate::app::RvApp;

/// Pixels within which a click on the map counts as a click on the route.
const ROUTE_HIT_PX: f32 = 12.0;

pub fn show(app: &mut RvApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui.small_button("+").clicked() {
            let _ = app.map_memory.zoom_in();
        }
        if ui.small_button("−").clicked() {
            let _ = app.map_memory.zoom_out();
        }
        ui.checkbox(&mut app.view.follow_on_map, "⌖")
            .on_hover_text("Follow the current position");
    });

    let file = app.selected.and_then(|i| app.files.get(i));
    let points: &[GpsPoint] = file.map_or(&[], |f| f.track.gps.as_slice());
    let current = file.and_then(|f| f.track.position_at(app.playback.position));
    let at = |t: Option<f64>| t.and_then(|t| file.and_then(|f| f.track.position_at(t)));
    let fallback = lon_lat(
        app.strings.t("gmStartX").parse().unwrap_or(30.3),
        app.strings.t("gmStartY").parse().unwrap_or(59.9),
    );
    let my_position = current.map_or(fallback, |p| lon_lat(p.lon, p.lat));

    let mut clicked_route: Option<(f64, f64)> = None;
    let plugin = TrackPlugin {
        points,
        current,
        mark_a: at(app.playback.mark_a),
        mark_b: at(app.playback.mark_b),
        route_color: app
            .strings
            .color("colorGraph_S", Color32::from_rgb(0, 0, 255)),
        arrow_color: app.strings.color("colorSpeedArrow", Color32::RED),
        clicked_route: &mut clicked_route,
    };
    let response = ui.add(
        Map::new(
            Some(&mut app.tiles as &mut dyn Tiles),
            &mut app.map_memory,
            my_position,
        )
        .with_plugin(plugin),
    );
    if response.dragged() {
        app.view.follow_on_map = false;
    }
    if let (Some((lat, lon)), Some(f)) = (clicked_route, app.current())
        && let Some(t) = f.track.nearest_time(lat, lon)
    {
        app.seek(t);
    }
}

struct TrackPlugin<'a> {
    points: &'a [GpsPoint],
    current: Option<GpsPoint>,
    mark_a: Option<GpsPoint>,
    mark_b: Option<GpsPoint>,
    route_color: Color32,
    arrow_color: Color32,
    clicked_route: &'a mut Option<(f64, f64)>,
}

impl Plugin for TrackPlugin<'_> {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        _memory: &MapMemory,
    ) {
        let painter = ui.painter_at(response.rect);
        let screen = |p: &GpsPoint| projector.project(lon_lat(p.lon, p.lat)).to_pos2();
        let route: Vec<Pos2> = self.points.iter().map(screen).collect();
        if route.len() > 1 {
            painter.add(Shape::line(
                route.clone(),
                Stroke::new(6.0, Color32::from_white_alpha(200)),
            ));
            painter.add(Shape::line(
                route.clone(),
                Stroke::new(3.5, self.route_color),
            ));
        }
        for (mark, label) in [(self.mark_a, "A"), (self.mark_b, "B")] {
            if let Some(p) = mark {
                let c = screen(&p);
                painter.circle(
                    c,
                    9.0,
                    Color32::from_rgb(255, 200, 0),
                    Stroke::new(1.5, Color32::BLACK),
                );
                painter.text(
                    c,
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(11.0),
                    Color32::BLACK,
                );
            }
        }
        if let Some(p) = self.current {
            draw_heading_arrow(&painter, screen(&p), p.course, self.arrow_color);
        }
        painter.text(
            response.rect.right_bottom() - Vec2::new(4.0, 2.0),
            Align2::RIGHT_BOTTOM,
            "© OpenStreetMap contributors",
            FontId::proportional(10.0),
            Color32::from_gray(60),
        );

        if response.clicked()
            && let Some(click) = response.interact_pointer_pos()
            && route.iter().any(|p| p.distance(click) <= ROUTE_HIT_PX)
        {
            let geo = projector.unproject(click.to_vec2());
            *self.clicked_route = Some((geo.y(), geo.x()));
        }
    }
}

/// Arrow pointing in the direction of travel (course in degrees clockwise from north).
pub fn draw_heading_arrow(painter: &egui::Painter, at: Pos2, course: f64, color: Color32) {
    let a = (course as f32).to_radians();
    let dir = Vec2::new(a.sin(), -a.cos());
    let side = Vec2::new(dir.y, -dir.x);
    let tip = at + dir * 14.0;
    let left = at - dir * 9.0 + side * 9.0;
    let right = at - dir * 9.0 - side * 9.0;
    let notch = at - dir * 4.0;
    painter.add(Shape::convex_polygon(
        vec![tip, right, notch],
        color,
        Stroke::NONE,
    ));
    painter.add(Shape::convex_polygon(
        vec![tip, notch, left],
        color,
        Stroke::NONE,
    ));
    painter.add(Shape::closed_line(
        vec![tip, right, notch, left],
        Stroke::new(1.5, Color32::WHITE),
    ));
}
