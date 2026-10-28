//! Main window state and layout. The panel structure follows the original `TFormMain`:
//!
//! ```text
//! MainMenu
//! PanelRight (right, resizable)          PanelLeft (client)
//! ├─ PanelControl (bottom): gauges,      ├─ PanelBottom (bottom, resizable):
//! │  play/stop, frames, volume, loop A-B  │    zoom row + graphs/seek bar
//! ├─ WebBrowser → map (bottom)            └─ PanelVideo (client)
//! └─ PanelListView (client): file list
//! ```

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use egui::{Key, KeyboardShortcut, Modifiers};
use rv_core::fileinfo::{self, FileInfo};
use walkers::{HeaderValue, HttpOptions, HttpTiles, MapMemory, lon_lat, sources::OpenStreetMap};

use crate::i18n::{Language, Strings};
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphKind {
    Speed,
    Altitude,
    TotalVector,
    X,
    Y,
    Z,
}

impl GraphKind {
    pub const ALL: [GraphKind; 6] = [
        GraphKind::Speed,
        GraphKind::Altitude,
        GraphKind::TotalVector,
        GraphKind::X,
        GraphKind::Y,
        GraphKind::Z,
    ];

    /// Suffix used by translation (`N_View_Graphics_S`) and color (`colorGraph_S`) keys.
    pub fn letter(self) -> &'static str {
        match self {
            GraphKind::Speed => "S",
            GraphKind::Altitude => "H",
            GraphKind::TotalVector => "V",
            GraphKind::X => "X",
            GraphKind::Y => "Y",
            GraphKind::Z => "Z",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSource {
    OpenStreetMap,
    None,
}

pub enum LoadEvent {
    Total(usize),
    File(Box<FileInfo>),
    Failed(String),
}

pub struct Playback {
    /// Current position in seconds from the start of the selected file.
    pub position: f64,
    pub playing: bool,
    pub rate: f64,
    pub volume: f32,
    pub loop_ab: bool,
    pub mark_a: Option<f64>,
    pub mark_b: Option<f64>,
    /// Incremented on every jump of the position (seek, stop, new file) so the video
    /// follows; normal playback advances `position` without touching it.
    pub seek_serial: u64,
}

pub struct View {
    pub show_file_list: bool,
    pub show_controls: bool,
    pub show_graphs: bool,
    pub graphs: Vec<GraphKind>,
    /// Graph time zoom: 1 = whole file, each step halves the visible window.
    pub time_zoom: u32,
    pub map: MapSource,
    pub follow_on_map: bool,
    pub metric: bool,
}

impl View {
    pub fn toggle_graph(&mut self, kind: GraphKind) {
        if let Some(i) = self.graphs.iter().position(|g| *g == kind) {
            self.graphs.remove(i);
        } else {
            self.graphs.push(kind);
        }
    }
}

pub struct RvApp {
    pub strings: Strings,
    pub folder: Option<PathBuf>,
    pub files: Vec<FileInfo>,
    pub selected: Option<usize>,
    pub loading: Option<(Receiver<LoadEvent>, usize)>,
    pub status: String,
    pub playback: Playback,
    pub view: View,
    pub map_memory: MapMemory,
    pub tiles: HttpTiles,
    pub show_about: bool,
    pub video: ui::video::VideoView,
}

impl RvApp {
    pub fn new(cc: &eframe::CreationContext<'_>, folder: Option<PathBuf>) -> Self {
        let cache = std::env::var_os("LOCALAPPDATA")
            .map(|d| PathBuf::from(d).join("rvplayer").join("tiles"));
        let tiles = HttpTiles::with_options(
            OpenStreetMap,
            HttpOptions {
                cache,
                // Tile servers (OpenStreetMap in particular) require an identifying user agent.
                user_agent: Some(HeaderValue::from_static(concat!(
                    "rvplayer/",
                    env!("CARGO_PKG_VERSION")
                ))),
                ..Default::default()
            },
            cc.egui_ctx.clone(),
        );
        let mut app = Self {
            strings: Strings::load(Language::English),
            folder: None,
            files: Vec::new(),
            selected: None,
            loading: None,
            status: String::new(),
            playback: Playback {
                position: 0.0,
                playing: false,
                rate: 1.0,
                volume: 1.0,
                loop_ab: false,
                mark_a: None,
                mark_b: None,
                seek_serial: 0,
            },
            view: View {
                show_file_list: true,
                show_controls: true,
                show_graphs: true,
                graphs: vec![GraphKind::Speed],
                time_zoom: 1,
                map: MapSource::OpenStreetMap,
                follow_on_map: true,
                metric: true,
            },
            map_memory: MapMemory::default(),
            tiles,
            show_about: false,
            video: ui::video::VideoView::default(),
        };
        if let Some(folder) = folder {
            app.open_folder(folder);
        }
        app
    }

    pub fn current(&self) -> Option<&FileInfo> {
        self.selected.and_then(|i| self.files.get(i))
    }

    pub fn duration(&self) -> f64 {
        self.current().map_or(0.0, |f| f.duration)
    }

    pub fn pick_folder(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            self.open_folder(dir);
        }
    }

    /// Lists video files and reads their headers on a background thread.
    pub fn open_folder(&mut self, dir: PathBuf) {
        self.files.clear();
        self.selected = None;
        self.playback.position = 0.0;
        self.playback.playing = false;
        self.folder = Some(dir.clone());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || match fileinfo::list_videos(&dir) {
            Ok(paths) => {
                let _ = tx.send(LoadEvent::Total(paths.len()));
                for p in paths {
                    if tx
                        .send(LoadEvent::File(Box::new(FileInfo::load(&p))))
                        .is_err()
                    {
                        return;
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(LoadEvent::Failed(format!("{}: {e}", dir.display())));
            }
        });
        self.loading = Some((rx, 0));
    }

    pub fn refresh(&mut self) {
        if let Some(dir) = self.folder.clone() {
            self.open_folder(dir);
        }
    }

    pub fn clear_files(&mut self) {
        self.files.clear();
        self.selected = None;
        self.loading = None;
        self.status.clear();
    }

    pub fn select(&mut self, index: usize) {
        if self.selected == Some(index) {
            return;
        }
        self.selected = Some(index);
        self.playback.position = 0.0;
        self.playback.mark_a = None;
        self.playback.mark_b = None;
        if let Some(p) = self.files[index].track.gps.first() {
            self.map_memory.center_at(lon_lat(p.lon, p.lat));
            let _ = self.map_memory.set_zoom(15.0);
        }
    }

    pub fn seek(&mut self, t: f64) {
        self.playback.position = t.clamp(0.0, self.duration());
        self.playback.seek_serial += 1;
    }

    pub fn toggle_play(&mut self) {
        if self.current().is_some() {
            if self.playback.position >= self.duration() {
                self.seek(0.0);
            }
            self.playback.playing = !self.playback.playing;
        }
    }

    pub fn stop(&mut self) {
        self.playback.playing = false;
        self.seek(0.0);
    }

    fn poll_loader(&mut self) {
        let Some((rx, total)) = &mut self.loading else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(LoadEvent::Total(n)) => *total = n,
                Ok(LoadEvent::File(info)) => self.files.push(*info),
                Ok(LoadEvent::Failed(msg)) => {
                    self.status = msg;
                    self.loading = None;
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.status = format!("{} / {total}", self.files.len());
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = self.files.len().to_string();
                    self.loading = None;
                    if self.selected.is_none() && !self.files.is_empty() {
                        self.select(0);
                    }
                    return;
                }
            }
        }
    }

    /// Advances the play position. Stands in for the video clock until a video backend exists.
    fn tick(&mut self, ctx: &egui::Context) {
        if !self.playback.playing {
            return;
        }
        let dt = f64::from(ctx.input(|i| i.stable_dt)).min(0.1);
        let mut pos = self.playback.position + dt * self.playback.rate;
        if let (true, Some(a), Some(b)) = (
            self.playback.loop_ab,
            self.playback.mark_a,
            self.playback.mark_b,
        ) && b > a
            && pos >= b
        {
            pos = a;
            self.playback.seek_serial += 1;
        }
        if pos >= self.duration() {
            pos = self.duration();
            self.playback.playing = false;
        }
        self.playback.position = pos;
        if self.view.follow_on_map
            && let Some(p) = self.current().and_then(|f| f.track.position_at(pos))
        {
            self.map_memory.center_at(lon_lat(p.lon, p.lat));
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let ctrl = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let none = |k| KeyboardShortcut::new(Modifiers::NONE, k);
        // Keys from the original menu: Space, Esc, arrows, F2, F4, F6/F7, Ctrl+O/R/N, Ctrl+Home/End/L.
        let typing = ctx.egui_wants_keyboard_input();
        ctx.input_mut(|i| {
            if i.consume_shortcut(&ctrl(Key::O)) {
                self.pick_folder();
            }
            if i.consume_shortcut(&ctrl(Key::R)) {
                self.refresh();
            }
            if i.consume_shortcut(&ctrl(Key::N)) {
                self.clear_files();
            }
            if i.consume_shortcut(&ctrl(Key::Home)) {
                self.playback.mark_a = Some(self.playback.position);
            }
            if i.consume_shortcut(&ctrl(Key::End)) {
                self.playback.mark_b = Some(self.playback.position);
            }
            if i.consume_shortcut(&ctrl(Key::L)) {
                self.playback.loop_ab = !self.playback.loop_ab;
            }
            if i.consume_shortcut(&none(Key::F2)) {
                self.view.show_file_list = !self.view.show_file_list;
            }
            if i.consume_shortcut(&none(Key::F4)) {
                self.view.show_controls = !self.view.show_controls;
            }
            if i.consume_shortcut(&none(Key::F6)) {
                self.view.show_graphs = false;
            }
            if i.consume_shortcut(&none(Key::F7)) {
                self.view.show_graphs = true;
            }
            if typing {
                return;
            }
            if i.consume_shortcut(&none(Key::Space)) {
                self.toggle_play();
            }
            if i.consume_shortcut(&none(Key::Escape)) {
                self.stop();
            }
            if i.consume_shortcut(&none(Key::ArrowLeft)) {
                self.seek(self.playback.position - 5.0);
            }
            if i.consume_shortcut(&none(Key::ArrowRight)) {
                self.seek(self.playback.position + 5.0);
            }
        });
    }
}

impl eframe::App for RvApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_loader();
        if self.loading.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.shortcuts(&ctx);
        self.tick(&ctx);

        egui::Panel::top("MainMenu").show(ui, |ui| ui::menu::show(self, ui));

        if self.view.show_file_list || self.view.show_controls || self.view.map != MapSource::None {
            egui::Panel::right("PanelRight")
                .resizable(true)
                .default_size(510.0)
                .min_size(320.0)
                .show(ui, |ui| {
                    if self.view.show_controls {
                        egui::Panel::bottom("PanelControl")
                            .resizable(false)
                            .show(ui, |ui| ui::controls::show(self, ui));
                    }
                    if self.view.map != MapSource::None {
                        egui::Panel::bottom("Map")
                            .resizable(true)
                            .default_size(320.0)
                            .min_size(120.0)
                            .show(ui, |ui| ui::map_view::show(self, ui));
                    }
                    if self.view.show_file_list {
                        egui::CentralPanel::default().show(ui, |ui| ui::file_list::show(self, ui));
                    }
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                if self.view.show_graphs {
                    egui::Panel::bottom("PanelBottom")
                        .resizable(true)
                        .default_size(170.0)
                        .min_size(90.0)
                        .show(ui, |ui| ui::graphs::show(self, ui));
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| ui::video::show(self, ui));
            });

        ui::menu::about_window(self, &ctx);
    }
}
