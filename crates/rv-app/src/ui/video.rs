//! Video area (`PanelVideo` of the original form): decoded frames following the play position,
//! with the date/speed/coordinates overlay.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::{
    Align2, Color32, ColorImage, FontId, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2,
};
use rv_video::Frame;
use rv_video::player::{Event, Player};

use crate::app::RvApp;
use crate::ui::clock_at;

const KM_PER_MILE: f64 = 1.609_344;
/// A frame is due when its time is within this many seconds after the play position.
const FRAME_TOLERANCE: f64 = 0.005;
/// When playback runs this far ahead of the decoded frames, restart decoding at the position.
const MAX_LAG: f64 = 1.0;
/// Decoded frames held by the view (each is a full RGBA image).
const MAX_BUFFERED: usize = 3;

#[derive(Default)]
pub struct VideoView {
    path: Option<PathBuf>,
    player: Option<Player>,
    /// Last `Playback::seek_serial` acted on.
    serial: u64,
    buffer: VecDeque<Frame>,
    /// Show the next decoded frame regardless of time (after opening or seeking).
    awaiting_first: bool,
    ended: bool,
    texture: Option<TextureHandle>,
    shown_time: Option<f64>,
    frame_size: [usize; 2],
    error: Option<String>,
}

impl VideoView {
    /// Keeps the decoder on `path` and the texture on the frame for `position`.
    fn update(
        &mut self,
        ctx: &egui::Context,
        path: Option<&Path>,
        position: f64,
        serial: u64,
        playing: bool,
    ) {
        if self.path.as_deref() != path {
            *self = Self {
                serial,
                texture: self.texture.take(),
                ..Self::default()
            };
            self.path = path.map(Path::to_path_buf);
            self.player = path.map(|p| Player::open(p.to_path_buf(), position));
            self.awaiting_first = true;
        }
        // Taken out for the update so frames can be uploaded while the player is in use.
        let Some(mut player) = self.player.take() else {
            return;
        };

        if serial != self.serial {
            self.serial = serial;
            restart(
                &mut player,
                &mut self.buffer,
                &mut self.awaiting_first,
                &mut self.ended,
                position,
            );
        }

        // Take only a few frames; the rest stay in the bounded channel so the decoder waits
        // instead of running ahead of playback.
        while self.buffer.len() < MAX_BUFFERED
            && let Some(event) = player.poll()
        {
            match event {
                Event::Frame { generation, frame } if generation == player.generation() => {
                    self.buffer.push_back(frame)
                }
                Event::EndOfStream { generation } if generation == player.generation() => {
                    self.ended = true
                }
                Event::Error(e) => self.error = Some(e),
                _ => {}
            }
        }

        let mut chosen = None;
        if self.awaiting_first
            && let Some(f) = self.buffer.pop_front()
        {
            self.awaiting_first = false;
            chosen = Some(f);
        }
        while self
            .buffer
            .front()
            .is_some_and(|f| f.time <= position + FRAME_TOLERANCE)
        {
            chosen = self.buffer.pop_front();
        }
        if let Some(frame) = chosen {
            self.upload(ctx, &frame);
        }

        let behind = self.shown_time.is_some_and(|t| t < position - MAX_LAG);
        if playing && behind && self.buffer.is_empty() && !self.awaiting_first && !self.ended {
            restart(
                &mut player,
                &mut self.buffer,
                &mut self.awaiting_first,
                &mut self.ended,
                position,
            );
        }
        if self.awaiting_first || !self.buffer.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(10));
        }
        self.player = Some(player);
    }

    fn upload(&mut self, ctx: &egui::Context, frame: &Frame) {
        let size = [frame.width as usize, frame.height as usize];
        let image = ColorImage::from_rgba_unmultiplied(size, &frame.rgba);
        match &mut self.texture {
            Some(t) => t.set(image, TextureOptions::LINEAR),
            None => {
                self.texture = Some(ctx.load_texture("video-frame", image, TextureOptions::LINEAR))
            }
        }
        self.frame_size = size;
        self.shown_time = Some(frame.time);
    }
}

fn restart(
    player: &mut Player,
    buffer: &mut VecDeque<Frame>,
    awaiting_first: &mut bool,
    ended: &mut bool,
    position: f64,
) {
    player.seek(position);
    buffer.clear();
    *awaiting_first = true;
    *ended = false;
}

pub fn show(app: &mut RvApp, ui: &mut Ui) {
    let path = app.current().map(|f| f.path.clone());
    let pb = &app.playback;
    app.video.update(
        ui.ctx(),
        path.as_deref(),
        pb.position,
        pb.seek_serial,
        pb.playing,
    );

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

    let v = &app.video;
    match (&v.texture, &v.error) {
        (_, Some(err)) => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                err,
                FontId::proportional(13.0),
                Color32::from_rgb(230, 90, 90),
            );
        }
        (Some(tex), None) if v.shown_time.is_some() => {
            painter.image(
                tex.id(),
                fit(rect, v.frame_size),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        _ => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                &file.name,
                FontId::proportional(14.0),
                Color32::from_gray(110),
            );
        }
    }

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

/// Largest rect with the frame's aspect ratio centred in `area` (letterbox).
fn fit(area: Rect, [w, h]: [usize; 2]) -> Rect {
    if w == 0 || h == 0 {
        return area;
    }
    let scale = (area.width() / w as f32).min(area.height() / h as f32);
    Rect::from_center_size(area.center(), Vec2::new(w as f32 * scale, h as f32 * scale))
}
