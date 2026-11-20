//! The egui application: the queue on the left, the preview and its trim timeline in the middle, the
//! settings of the selected videos on the right.
//!
//! Rules the UI follows:
//! - it renders state and holds no rule: the queue ([`crate::queue`]) owns the workers and what they
//!   decide, the profile ([`crate::profile`]) what a setting means;
//! - it never waits on a worker or touches the disk in a frame, except when the user acts (adding files,
//!   saving profiles);
//! - a setting edited with several videos selected changes that setting on every selected video, and only
//!   that one.

mod preview;
mod queue_panel;
mod settings;
mod timeline;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::ffmpeg::decoder::DecodeConfig;
use crate::ffmpeg::encoders::EncoderBackend;
use crate::ffmpeg::hardware::Capabilities;
use crate::plan;
use crate::player::{AudioSource, Player};
use crate::profile::{self, AudioChoice, ProfileSet};
use crate::queue::{JobId, Phase, Queue, QueueSettings};
use crate::thumbnails::{self, StripRequest, Thumbnail};

/// The application's identifier: window class, data folder name.
pub const APP_ID: &str = "quick-video-transcoder";

const PREFERENCES_KEY: &str = "preferences";

/// File extensions offered by the *Add videos* dialog (any file can still be dropped).
const VIDEO_EXTENSIONS: [&str; 9] = [
    "mp4", "mkv", "mov", "webm", "avi", "m4v", "ts", "m2ts", "wmv",
];

/// Settings remembered between sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    last_dir: Option<PathBuf>,
    /// Profile given to new videos.
    profile: String,
    parallel_encodes: usize,
    backend: Option<EncoderBackend>,
    output_dir: Option<PathBuf>,
    volume: f32,
    muted: bool,
    /// Last detected hardware, used until this session's detection finishes.
    capabilities: Option<Capabilities>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            last_dir: None,
            profile: String::new(),
            parallel_encodes: QueueSettings::default().parallel_encodes,
            backend: None,
            output_dir: None,
            volume: 1.0,
            muted: false,
            capabilities: None,
        }
    }
}

impl Preferences {
    fn queue_settings(&self) -> QueueSettings {
        QueueSettings {
            parallel_encodes: self.parallel_encodes.clamp(1, 8),
            backend: self.backend,
            output_dir: self.output_dir.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Level {
    Info,
    Error,
}

/// The thumbnail strip of a video.
#[derive(Default)]
struct Strip {
    textures: Vec<Option<egui::TextureHandle>>,
    cancel: Arc<AtomicBool>,
}

/// The application.
pub struct TranscoderApp {
    queue: Queue,
    profiles: ProfileSet,
    profiles_path: Option<PathBuf>,
    prefs: Preferences,
    /// Selected videos.
    selection: HashSet<JobId>,
    /// The video shown in the preview (the last one clicked).
    focus: Option<JobId>,
    /// Last clicked row, the anchor of Shift+click.
    anchor: Option<JobId>,

    player: Player,
    /// The video the player has open.
    playing_job: Option<JobId>,
    frame_texture: Option<egui::TextureHandle>,
    frame_size: Option<plan::Size>,

    strips: HashMap<JobId, Strip>,
    thumbnail_sender: Sender<Thumbnail>,
    thumbnail_receiver: Receiver<Thumbnail>,

    detection: Option<Receiver<Capabilities>>,
    log: VecDeque<(Level, String)>,
    show_log: bool,
    show_shortcuts: bool,
    show_profiles: bool,
    /// Name typed in the *Save as profile* field.
    new_profile_name: String,
    title: String,
    notify: Arc<dyn Fn() + Send + Sync>,
    /// `QVT_SCREENSHOT`: where to save a screenshot once the queue is idle, then quit (development aid).
    screenshot: Option<(PathBuf, Option<std::time::Instant>)>,
}

impl TranscoderApp {
    /// Creates the application and queues `files`.
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        let prefs: Preferences = cc
            .storage
            .and_then(|s| eframe::get_value(s, PREFERENCES_KEY))
            .unwrap_or_default();
        // Selectable labels sense clicks and drags for text selection, which steals the click from a queue
        // row under its file name: nothing in this window is text to copy.
        cc.egui_ctx
            .global_style_mut(|s| s.interaction.selectable_labels = false);
        let ctx = cc.egui_ctx.clone();
        let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(move || ctx.request_repaint());
        let capabilities = prefs.capabilities.clone().unwrap_or_default();
        let (sender, detection) = crossbeam_channel::bounded(1);
        let repaint = Arc::clone(&notify);
        std::thread::spawn(move || {
            let _ = sender.send(Capabilities::detect());
            repaint();
        });
        let profiles_path = profile::data_dir().map(|dir| dir.join(ProfileSet::FILE_NAME));
        let mut log = VecDeque::new();
        let profiles = match profiles_path.as_deref().map(ProfileSet::load) {
            Some(Ok(set)) => set,
            Some(Err(error)) => {
                log.push_back((
                    Level::Error,
                    format!("{error:#}; using the built-in profile"),
                ));
                ProfileSet::default()
            }
            None => ProfileSet::default(),
        };
        let (thumbnail_sender, thumbnail_receiver) = unbounded();
        let mut app = Self {
            queue: Queue::new(capabilities, prefs.queue_settings(), Arc::clone(&notify)),
            profiles,
            profiles_path,
            prefs,
            selection: HashSet::new(),
            focus: None,
            anchor: None,
            player: Player::new(),
            playing_job: None,
            frame_texture: None,
            frame_size: None,
            strips: HashMap::new(),
            thumbnail_sender,
            thumbnail_receiver,
            detection: Some(detection),
            log,
            show_log: false,
            show_shortcuts: false,
            show_profiles: false,
            new_profile_name: String::new(),
            title: String::new(),
            notify,
            screenshot: std::env::var_os("QVT_SCREENSHOT").map(|p| (PathBuf::from(p), None)),
        };
        app.add_files(files);
        app
    }

    fn info(&mut self, text: impl Into<String>) {
        self.push_log(Level::Info, text.into());
    }

    fn error(&mut self, text: impl Into<String>) {
        self.push_log(Level::Error, text.into());
    }

    fn push_log(&mut self, level: Level, text: String) {
        match level {
            Level::Info => log::info!("{text}"),
            Level::Error => log::error!("{text}"),
        }
        if level == Level::Error {
            self.show_log = true;
        }
        if self.log.len() == 300 {
            self.log.pop_front();
        }
        self.log.push_back((level, text));
    }

    /// The profile given to new videos.
    fn default_profile(&self) -> profile::Profile {
        self.profiles
            .find(&self.prefs.profile)
            .cloned()
            .unwrap_or_else(|| self.profiles.first())
    }

    /// Queues files, selecting the first new one when nothing is selected.
    fn add_files(&mut self, files: Vec<PathBuf>) {
        let profile = self.default_profile();
        for path in files {
            let path = native_separators(path);
            if path.is_dir() {
                self.error(format!("{} is a folder, not a video", path.display()));
                continue;
            }
            if let Some(dir) = path.parent() {
                self.prefs.last_dir = Some(dir.to_path_buf());
            }
            let id = self.queue.add(path, &profile);
            if self.focus.is_none() {
                self.focus = Some(id);
                self.selection.insert(id);
                self.anchor = Some(id);
            }
        }
    }

    fn add_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Add videos")
            .add_filter("Videos", &VIDEO_EXTENSIONS)
            .add_filter("All files", &["*"]);
        if let Some(dir) = &self.prefs.last_dir {
            dialog = dialog.set_directory(dir);
        }
        if let Some(files) = dialog.pick_files() {
            self.add_files(files);
        }
    }

    fn remove_selected(&mut self) {
        let ids: Vec<JobId> = self.selection.drain().collect();
        for id in ids {
            self.queue.remove(id);
            if let Some(strip) = self.strips.remove(&id) {
                strip.cancel.store(true, Ordering::Relaxed);
            }
        }
        if self.focus.is_some_and(|f| self.queue.get(f).is_none()) {
            self.focus = self.queue.jobs.first().map(|j| j.id);
            self.selection.extend(self.focus);
        }
    }

    /// Selected videos in queue order.
    fn selected_ids(&self) -> Vec<JobId> {
        self.queue
            .jobs
            .iter()
            .filter(|j| self.selection.contains(&j.id))
            .map(|j| j.id)
            .collect()
    }

    fn encode_selected(&mut self) {
        let ids = self.selected_ids();
        self.queue.encode(&ids);
    }

    fn encode_all(&mut self) {
        let ids: Vec<JobId> = self
            .queue
            .jobs
            .iter()
            .filter(|j| !matches!(j.phase, Phase::Done(_)))
            .map(|j| j.id)
            .collect();
        self.queue.encode(&ids);
    }

    /// Outputs of the selected videos that are encoded (all encoded ones when none is selected).
    fn done_outputs(&self, only_selected: bool) -> Vec<PathBuf> {
        self.queue
            .jobs
            .iter()
            .filter(|j| !only_selected || self.selection.contains(&j.id))
            .filter_map(|j| match &j.phase {
                Phase::Done(encoded) => Some(encoded.output.clone()),
                _ => None,
            })
            .collect()
    }

    fn copy_outputs(&mut self, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        match crate::clipboard::copy_files(&files) {
            Ok(()) => self.info(format!(
                "{} file(s) copied: paste them into the conversation",
                files.len()
            )),
            Err(error) => self.error(format!("{error:#}")),
        }
    }

    fn save_profiles(&mut self) {
        let Some(path) = self.profiles_path.clone() else {
            self.error("no data folder: the profiles cannot be saved");
            return;
        };
        match self.profiles.save(&path) {
            Ok(()) => self.info(format!("profiles saved to {}", path.display())),
            Err(error) => self.error(format!("{error:#}")),
        }
    }

    fn poll_detection(&mut self) {
        let Some(capabilities) = self.detection.as_ref().and_then(|r| r.try_recv().ok()) else {
            return;
        };
        self.detection = None;
        self.info(format!("hardware: {}", capabilities.summary()));
        if capabilities.encoders.is_empty() {
            self.error("no working video encoder: is FFmpeg on the PATH?");
        }
        self.prefs.capabilities = Some(capabilities.clone());
        if capabilities != self.queue.capabilities {
            self.queue.set_capabilities(capabilities);
        }
    }

    fn poll_queue(&mut self) {
        for (is_error, line) in self.queue.poll() {
            if is_error {
                self.error(line);
            } else {
                self.info(line);
            }
        }
        // Thumbnails of newly probed videos.
        let ready: Vec<StripRequest> = self
            .queue
            .jobs
            .iter()
            .filter(|j| !self.strips.contains_key(&j.id))
            .filter_map(|j| {
                Some(StripRequest {
                    job: j.id,
                    path: j.path.clone(),
                    media: j.media.clone()?,
                    engine: j.engine?,
                    tone: j.profile.tone_mapping,
                })
            })
            .collect();
        for request in ready {
            let strip = Strip {
                textures: vec![None; thumbnails::COUNT],
                cancel: Arc::new(AtomicBool::new(false)),
            };
            thumbnails::spawn(
                request.clone(),
                self.thumbnail_sender.clone(),
                Arc::clone(&self.notify),
                Arc::clone(&strip.cancel),
            );
            self.strips.insert(request.job, strip);
        }
    }

    fn poll_thumbnails(&mut self, ctx: &egui::Context) {
        while let Ok(thumbnail) = self.thumbnail_receiver.try_recv() {
            let Some(strip) = self.strips.get_mut(&thumbnail.job) else {
                continue;
            };
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [
                    thumbnail.size.width as usize,
                    thumbnail.size.height as usize,
                ],
                &thumbnail.rgba,
            );
            let texture = ctx.load_texture(
                format!("thumb-{}-{}", thumbnail.job, thumbnail.index),
                image,
                egui::TextureOptions::LINEAR,
            );
            if let Some(slot) = strip.textures.get_mut(thumbnail.index) {
                *slot = Some(texture);
            }
        }
    }

    /// The preview's decode settings for the focused video: its output size (from the estimate, else the
    /// profile's best), its filters and tone mapping.
    fn decode_config(&self) -> Option<(JobId, DecodeConfig, Option<AudioSource>)> {
        let job = self.queue.get(self.focus?)?;
        let media = job.media.clone()?;
        let engine = job.engine?;
        let key = self.queue.key(job);
        let target = match job.current_estimate(key.as_ref()) {
            Some(Ok(estimate)) => estimate.target,
            _ => plan::best_target(&media, &job.profile),
        };
        let audio =
            (!media.audio.is_empty() && job.profile.audio != AudioChoice::None).then(|| {
                AudioSource {
                    path: job.path.clone(),
                    tracks: media.audio.len(),
                    choice: job.profile.audio,
                }
            });
        let config = DecodeConfig {
            path: job.path.clone(),
            media,
            engine,
            size: target.size,
            tone: job.profile.tone_mapping,
        };
        Some((job.id, config, audio))
    }

    /// Opens the focused video in the player, or follows a change of its settings.
    fn update_player(&mut self, ctx: &egui::Context) {
        match self.decode_config() {
            Some((id, config, audio)) => {
                if self.playing_job != Some(id) {
                    let start = self.queue.get(id).map_or(0.0, |j| j.trim.start);
                    self.player.open(config, audio, start);
                    self.playing_job = Some(id);
                    self.frame_size = None;
                } else {
                    // The software fallback the player chose is kept.
                    let fallback = self
                        .player
                        .config()
                        .is_some_and(|c| c.engine == crate::ffmpeg::filters::Engine::SOFTWARE);
                    let config = if fallback {
                        DecodeConfig {
                            engine: crate::ffmpeg::filters::Engine::SOFTWARE,
                            ..config
                        }
                    } else {
                        config
                    };
                    self.player.set_config(config);
                    self.player.set_audio(audio);
                }
            }
            None => {
                if self.playing_job.is_some() {
                    self.player.close();
                    self.playing_job = None;
                    self.frame_texture = None;
                    self.frame_size = None;
                }
            }
        }
        let gain = if self.prefs.muted {
            0.0
        } else {
            self.prefs.volume.powi(2)
        };
        self.player.set_gain(gain);
        if let Some(warning) = self.player.take_audio_warning() {
            self.error(warning);
        }
        // Playback stops at the end of the trimmed range.
        if self.player.is_playing()
            && let Some(job) = self.playing_job.and_then(|id| self.queue.get(id))
            && self.player.position()
                >= job.trim.end - 0.5 / job.media.as_ref().map_or(30.0, |m| m.fps())
        {
            self.player.pause();
        }
        if let Some(frame) = self.player.poll() {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &frame.data,
            );
            match &mut self.frame_texture {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => {
                    self.frame_texture =
                        Some(ctx.load_texture("preview", image, egui::TextureOptions::LINEAR));
                }
            }
            self.frame_size = Some(plan::Size {
                width: frame.width,
                height: frame.height,
            });
            self.player.recycle(frame);
        }
        if self.player.is_playing() || self.player.is_loading() {
            ctx.request_repaint();
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_files(dropped);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let command = |key| ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, key));
        if command(Key::O) {
            self.add_dialog();
        }
        if command(Key::E) {
            self.encode_selected();
        }
        if command(Key::A) && !ctx.egui_wants_keyboard_input() {
            self.selection = self.queue.jobs.iter().map(|j| j.id).collect();
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed = |key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        if pressed(Key::Delete) {
            self.remove_selected();
        }
        if self.playing_job.is_none() {
            return;
        }
        // Shift variants first: egui ignores an extra Shift when matching the plain key.
        let count =
            |modifiers, key| ctx.input_mut(|i| i.count_and_consume_key(modifiers, key)) as i64;
        let seconds =
            count(Modifiers::SHIFT, Key::ArrowRight) - count(Modifiers::SHIFT, Key::ArrowLeft);
        let frames =
            count(Modifiers::NONE, Key::ArrowRight) - count(Modifiers::NONE, Key::ArrowLeft);
        let tens = count(Modifiers::NONE, Key::PageDown) - count(Modifiers::NONE, Key::PageUp);
        let jump = seconds as f64 + 10.0 * tens as f64;
        if jump != 0.0 {
            self.player.seek(self.player.position() + jump);
        } else if frames != 0 {
            self.player.step(frames);
        }
        if pressed(Key::Space) {
            self.toggle_playback();
        }
        if pressed(Key::I) {
            self.set_trim_start(self.player.position());
        }
        if pressed(Key::O) {
            self.set_trim_end(self.player.position());
        }
        if pressed(Key::Home)
            && let Some(job) = self.playing_job.and_then(|id| self.queue.get(id))
        {
            let start = job.trim.start;
            self.player.seek(start);
        }
        if pressed(Key::End)
            && let Some(job) = self.playing_job.and_then(|id| self.queue.get(id))
        {
            let end = job.trim.end;
            self.player.seek(end);
        }
        if pressed(Key::M) {
            self.prefs.muted = !self.prefs.muted;
        }
    }

    /// Plays from the trim start when the playhead is outside the kept part.
    fn toggle_playback(&mut self) {
        if !self.player.is_playing()
            && let Some(job) = self.playing_job.and_then(|id| self.queue.get(id))
        {
            let position = self.player.position();
            let fps = job.media.as_ref().map_or(30.0, |m| m.fps());
            if position < job.trim.start || position >= job.trim.end - 1.0 / fps {
                let start = job.trim.start;
                self.player.seek(start);
            }
        }
        self.player.toggle();
    }

    fn set_trim_start(&mut self, seconds: f64) {
        let Some(job) = self.playing_job.and_then(|id| self.queue.get_mut(id)) else {
            return;
        };
        let Some(media) = &job.media else { return };
        let start = media
            .snap(seconds)
            .clamp(0.0, (job.trim.end - 0.5).max(0.0));
        if start != job.trim.start {
            job.trim.start = start;
            job.touch();
        }
    }

    fn set_trim_end(&mut self, seconds: f64) {
        let Some(job) = self.playing_job.and_then(|id| self.queue.get_mut(id)) else {
            return;
        };
        let Some(media) = &job.media else { return };
        let frame = 1.0 / media.fps().max(1.0);
        // The end is the end of the frame under the playhead, so the frame shown is kept.
        let end = (media.snap(seconds) + frame).clamp(job.trim.start + 0.5, media.duration);
        if end != job.trim.end {
            job.trim.end = end;
            job.touch();
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Add videos…  (Ctrl+O)").clicked() {
                    ui.close();
                    self.add_dialog();
                }
                if ui
                    .add_enabled(
                        !self.selection.is_empty(),
                        egui::Button::new("Remove selected  (Del)"),
                    )
                    .clicked()
                {
                    ui.close();
                    self.remove_selected();
                }
                if ui.button("Remove encoded videos").clicked() {
                    ui.close();
                    self.selection = self
                        .queue
                        .jobs
                        .iter()
                        .filter(|j| matches!(j.phase, Phase::Done(_)))
                        .map(|j| j.id)
                        .collect();
                    self.remove_selected();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Profiles", |ui| {
                if ui.button("Edit profiles…").clicked() {
                    ui.close();
                    self.show_profiles = true;
                }
                if let Some(path) = &self.profiles_path
                    && ui.button("Open profiles.toml folder").clicked()
                {
                    ui.close();
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                        reveal(dir, None);
                    }
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.show_log, "Log");
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard shortcuts").clicked() {
                    ui.close();
                    self.show_shortcuts = true;
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let encoding = self
                .queue
                .jobs
                .iter()
                .filter(|j| matches!(j.phase, Phase::Encoding(_) | Phase::Waiting))
                .count();
            let estimating = self
                .queue
                .jobs
                .iter()
                .filter(|j| j.phase == Phase::Estimating)
                .count();
            let done = self
                .queue
                .jobs
                .iter()
                .filter(|j| matches!(j.phase, Phase::Done(_)))
                .count();
            ui.label(format!(
                "{} video(s) · {encoding} encoding · {estimating} estimating · {done} done",
                self.queue.jobs.len()
            ));
            if self.detection.is_some() {
                ui.separator();
                ui.spinner();
                ui.label("Detecting the encoders…");
            }
            if let Some((level, line)) = self.log.back() {
                ui.separator();
                let text = egui::RichText::new(line).weak();
                let text = if *level == Level::Error {
                    text.color(ui.visuals().error_fg_color)
                } else {
                    text
                };
                if ui
                    .add(
                        egui::Label::new(text)
                            .truncate()
                            .sense(egui::Sense::click()),
                    )
                    .clicked()
                {
                    self.show_log = !self.show_log;
                }
            }
        });
    }

    fn log_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (level, line) in &self.log {
                    let text = egui::RichText::new(line).monospace().size(12.0);
                    let text = if *level == Level::Error {
                        text.color(ui.visuals().error_fg_color)
                    } else {
                        text
                    };
                    ui.add(egui::Label::new(text).wrap());
                }
            });
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Keyboard shortcuts")
            .open(&mut self.show_shortcuts)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::Grid::new("shortcuts").striped(true).show(ui, |ui| {
                    for (keys, action) in [
                        ("Space", "Play / pause the kept part"),
                        ("Left / Right", "Previous / next frame"),
                        ("Shift + Left / Right", "-1 s / +1 s"),
                        ("Page Up / Down", "-10 s / +10 s"),
                        ("I / O", "Start / end of the kept part at the current frame"),
                        ("Home / End", "Go to the start / end of the kept part"),
                        ("M", "Mute / unmute"),
                        ("Ctrl + O", "Add videos"),
                        ("Ctrl + A", "Select every video"),
                        ("Ctrl + E", "Encode the selected videos"),
                        ("Delete", "Remove the selected videos"),
                        ("Drag a handle", "Move the start or end of the kept part"),
                        ("Shift + drag", "Move the kept part as a whole"),
                    ] {
                        ui.monospace(keys);
                        ui.label(action);
                        ui.end_row();
                    }
                });
            });
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let total = self.queue.jobs.len();
        let encoding: Vec<f32> = self
            .queue
            .jobs
            .iter()
            .filter_map(|j| match &j.phase {
                Phase::Encoding(p) => Some(p.fraction),
                _ => None,
            })
            .collect();
        let title = if encoding.is_empty() {
            format!("Quick Video Transcoder — {total} video(s)")
        } else {
            let average = encoding.iter().sum::<f32>() / encoding.len() as f32;
            format!(
                "Quick Video Transcoder — encoding {} ({:.0} %)",
                encoding.len(),
                average * 100.0
            )
        };
        if title != self.title {
            self.title.clone_from(&title);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
    }

    /// `QVT_SCREENSHOT=<file.ppm>`: once every video is probed and estimated and the preview shows a
    /// frame, waits two seconds, saves the window as a binary PPM and quits. For checking the UI without
    /// driving it.
    fn take_screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, idle_since)) = &mut self.screenshot else {
            return;
        };
        let images: Vec<Arc<egui::ColorImage>> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                    _ => None,
                })
                .collect()
        });
        if let Some(image) = images.first() {
            let [width, height] = image.size;
            let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
            for pixel in &image.pixels {
                ppm.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
            }
            if let Err(error) = std::fs::write(&*path, ppm) {
                log::error!("cannot save the screenshot: {error}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let busy = self.detection.is_some()
            || self.frame_texture.is_none()
            || self.queue.jobs.iter().any(|j| {
                j.busy()
                    || self
                        .queue
                        .key(j)
                        .is_some_and(|k| j.current_estimate(Some(&k)).is_none())
            });
        match (busy, *idle_since) {
            (true, _) => *idle_since = None,
            (false, None) => *idle_since = Some(std::time::Instant::now()),
            (false, Some(since)) if since.elapsed() > Duration::from_secs(2) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            _ => {}
        }
    }

    /// Asks before closing while encodes run: closing stops them.
    fn confirm_close(&self, ctx: &egui::Context) {
        let running = self
            .queue
            .jobs
            .iter()
            .any(|j| matches!(j.phase, Phase::Encoding(_) | Phase::Waiting));
        if ctx.input(|i| i.viewport().close_requested()) && running {
            let quit = rfd::MessageDialog::new()
                .set_title("Encodes running")
                .set_description("Closing stops the running encodes and deletes their partial files. Close anyway?")
                .set_buttons(rfd::MessageButtons::YesNo)
                .set_level(rfd::MessageLevel::Warning)
                .show();
            if quit != rfd::MessageDialogResult::Yes {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }
}

/// A path written with Windows' own separators: Windows accepts `/`, but a path given with them (from a
/// shell) would reach the clipboard and the output names mixed with `\`.
fn native_separators(path: PathBuf) -> PathBuf {
    match path.to_str() {
        Some(text) if cfg!(windows) && text.contains('/') => PathBuf::from(text.replace('/', r"\")),
        _ => path,
    }
}

/// Shows a folder in Explorer, with `file` selected when given.
fn reveal(dir: &Path, file: Option<&Path>) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("explorer.exe");
        match file {
            // Explorer parses its own command line: the path is quoted as a whole after the comma.
            Some(file) => command.raw_arg(format!("/select,\"{}\"", file.display())),
            None => command.arg(dir),
        };
        let _ = command.spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let _ = std::process::Command::new(opener).arg(dir).spawn();
    }
}

impl eframe::App for TranscoderApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if std::env::var_os("QVT_DEBUG_HITS").is_some() {
            ctx.global_style_mut(|s| s.debug.show_widget_hits = true);
            if ctx.input(|i| i.pointer.any_pressed())
                && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
            {
                log::warn!(
                    "press at {pos:?}, layer {:?}, screen {:?}, ppp {}",
                    ctx.layer_id_at(pos),
                    ctx.viewport_rect(),
                    ctx.pixels_per_point()
                );
            }
        }
        self.confirm_close(&ctx);
        self.poll_detection();
        self.handle_dropped_files(&ctx);
        self.handle_shortcuts(&ctx);
        self.poll_queue();
        self.poll_thumbnails(&ctx);
        self.update_player(&ctx);
        self.update_title(&ctx);
        // Estimates wait for the settings to settle: look again shortly.
        ctx.request_repaint_after(Duration::from_millis(300));

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.show_log {
            egui::Panel::bottom("log")
                .resizable(true)
                .default_size(140.0)
                .size_range(60.0..=400.0)
                .show(ui, |ui| self.log_panel(ui));
        }
        egui::Panel::left("queue")
            .resizable(true)
            .default_size(400.0)
            .size_range(300.0..=640.0)
            .show(ui, |ui| self.queue_panel(ui));
        egui::Panel::right("settings")
            .resizable(true)
            .default_size(360.0)
            .size_range(300.0..=520.0)
            .show(ui, |ui| self.settings_panel(ui));
        egui::Panel::bottom("timeline").show(ui, |ui| self.timeline(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.preview(ui));

        self.shortcuts_window(&ctx);
        self.profiles_window(&ctx);
        self.take_screenshot(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, PREFERENCES_KEY, &self.prefs);
    }
}
