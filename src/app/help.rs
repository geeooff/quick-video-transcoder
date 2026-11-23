//! The windows that explain rather than edit: *About*, and what to do when FFmpeg is missing. Also the
//! *Send to* menu items and the files later launches hand over.

use std::sync::Arc;

use crossbeam_channel::Receiver;
use eframe::egui;

use super::TranscoderApp;
use crate::build_info;
use crate::ffmpeg::{Install, MIN_MAJOR_VERSION};

/// Where FFmpeg builds for Windows are downloaded (the "release full" build).
const FFMPEG_BUILDS: &str = "https://www.gyan.dev/ffmpeg/builds/";
/// The one command that installs FFmpeg for the current user.
const WINGET_COMMAND: &str = "winget install --id Gyan.FFmpeg -e";

impl TranscoderApp {
    /// Queues the files later launches sent, and brings the window forward.
    pub(super) fn poll_incoming(&mut self, ctx: &egui::Context) {
        let files = self.incoming.as_ref().map(|i| i.take()).unwrap_or_default();
        if files.is_empty() {
            return;
        }
        self.info(format!("{} file(s) received from Send to", files.len()));
        self.add_files(files);
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        // Windows may refuse the focus to a background window: the taskbar button flashes instead.
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
            egui::UserAttentionType::Informational,
        ));
    }

    /// *File* menu: add the application to Explorer's *Send to* menu, or take it away.
    pub(super) fn sendto_menu(&mut self, ui: &mut egui::Ui) {
        let label = if self.sendto_installed {
            "Update the Send to shortcut"
        } else {
            "Add to Explorer's Send to menu"
        };
        if ui
            .button(label)
            .on_hover_text(
                "Select videos in Explorer, right-click, Send to, Quick Video Transcoder",
            )
            .clicked()
        {
            ui.close();
            match crate::sendto::install() {
                Ok(link) => self.info(format!("Send to shortcut written: {}", link.display())),
                Err(error) => self.error(format!("{error:#}")),
            }
            self.sendto_installed = crate::sendto::is_installed();
        }
        if ui
            .add_enabled(
                self.sendto_installed,
                egui::Button::new("Remove from Explorer's Send to menu"),
            )
            .clicked()
        {
            ui.close();
            match crate::sendto::remove() {
                Ok(()) => self.info("Send to shortcut removed"),
                Err(error) => self.error(format!("{error:#}")),
            }
            self.sendto_installed = crate::sendto::is_installed();
        }
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        let mut show_notices = false;
        egui::Window::new("About Quick Video Transcoder")
            .open(&mut self.show_about)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.heading(format!("Quick Video Transcoder {}", build_info::VERSION));
                ui.label("Trims game captures and re-encodes them to fit a size limit.");
                ui.add_space(8.0);
                egui::Grid::new("about").num_columns(2).show(ui, |ui| {
                    ui.label("Commit");
                    ui.monospace(format!("{}{}", build_info::COMMIT_SHORT, build_info::DIRTY));
                    ui.end_row();
                    ui.label("Documentation");
                    ui.hyperlink_to("for this version", build_info::documentation());
                    ui.end_row();
                    ui.label("Changes");
                    ui.hyperlink_to("CHANGELOG.md", build_info::doc_url("CHANGELOG.md"));
                    ui.end_row();
                    ui.label("Licence");
                    ui.hyperlink_to("MIT", build_info::doc_url("LICENSE"));
                    ui.end_row();
                    ui.label("Built with");
                    if ui
                        .link("third-party libraries and their licences")
                        .clicked()
                    {
                        show_notices = true;
                    }
                    ui.end_row();
                });
            });
        if show_notices {
            self.open_notices();
        }
    }

    /// Writes the notices built into the executable to the data folder and opens them in the browser.
    fn open_notices(&mut self) {
        let Some(dir) = crate::profile::data_dir() else {
            self.error("no data folder to open the third-party notices from");
            return;
        };
        let path = dir.join("THIRD-PARTY-NOTICES.html");
        let written =
            std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, build_info::NOTICES));
        match written {
            Ok(()) => super::open_file(&path),
            Err(error) => self.error(format!("cannot write {}: {error}", path.display())),
        }
    }

    /// Shown when FFmpeg is missing or too old: what to install, a button that installs it with winget
    /// when winget is there, and a button to look again.
    pub(super) fn ffmpeg_window(&mut self, ctx: &egui::Context) {
        let Some(help) = &mut self.ffmpeg_help else {
            return;
        };
        help.poll();
        let mut open = true;
        let mut action = None;
        egui::Window::new("FFmpeg is needed")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(540.0)
            .show(ctx, |ui| action = help.ui(ui));
        match action {
            Some(Action::CheckAgain) if self.detection.is_none() => {
                self.ffmpeg_help = None;
                self.detection = Some(super::spawn_detection(Arc::clone(&self.notify)));
            }
            Some(Action::Install) => help_install(self),
            _ if !open => self.ffmpeg_help = None,
            _ => {}
        }
    }
}

/// Starts winget, and checks again once it is done.
fn help_install(app: &mut TranscoderApp) {
    let Some(help) = &mut app.ffmpeg_help else {
        return;
    };
    let (sender, receiver) = crossbeam_channel::bounded(1);
    let notify = Arc::clone(&app.notify);
    std::thread::spawn(move || {
        let _ = sender.send(crate::ffmpeg::install_with_winget().map_err(|e| format!("{e:#}")));
        notify();
    });
    help.installing = Some(receiver);
    help.failure = None;
    app.info(format!(
        "installing FFmpeg with winget ({})…",
        crate::ffmpeg::WINGET_PACKAGE
    ));
}

/// What the user asked for in the FFmpeg window.
enum Action {
    CheckAgain,
    Install,
}

/// The state of the *FFmpeg is needed* window.
pub(super) struct FfmpegHelp {
    problem: Install,
    /// Whether winget runs here: checked once, in the background.
    winget: Option<bool>,
    winget_check: Option<Receiver<bool>>,
    /// A running `winget install`.
    installing: Option<Receiver<Result<(), String>>>,
    /// Why the last install failed.
    failure: Option<String>,
    /// An install finished: look for FFmpeg again.
    installed: bool,
}

impl FfmpegHelp {
    pub(super) fn new(problem: Install, notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = sender.send(crate::ffmpeg::winget_available());
            notify();
        });
        Self {
            problem,
            winget: None,
            winget_check: Some(receiver),
            installing: None,
            failure: None,
            installed: false,
        }
    }

    fn poll(&mut self) {
        if let Some(available) = self.winget_check.as_ref().and_then(|r| r.try_recv().ok()) {
            self.winget = Some(available);
            self.winget_check = None;
        }
        if let Some(result) = self.installing.as_ref().and_then(|r| r.try_recv().ok()) {
            self.installing = None;
            match result {
                Ok(()) => self.installed = true,
                Err(error) => self.failure = Some(error),
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.label(match &self.problem {
            Install::Missing(program) => format!(
                "Quick Video Transcoder reads and encodes videos with FFmpeg, a free program it does \
                 not include, and {program} cannot be found on this computer."
            ),
            Install::TooOld(version) => format!(
                "The FFmpeg found is too old ({version}). Version {MIN_MAJOR_VERSION} or later is needed."
            ),
            Install::Ready(_) => String::new(),
        });
        ui.add_space(10.0);
        if self.installed {
            // Checked right away: the window goes once FFmpeg is found.
            return Some(Action::CheckAgain);
        }
        match (self.winget, &self.installing) {
            (_, Some(_)) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Installing FFmpeg with winget: a minute or two…");
                });
            }
            (Some(true), None) => {
                ui.label(format!(
                    "It can be installed for you with winget, Windows' package manager: the {} \
                     package (gyan.dev's full build, about 100 MB, for your account only).",
                    crate::ffmpeg::WINGET_PACKAGE
                ));
                if ui.button("Install FFmpeg for me").clicked() {
                    action = Some(Action::Install);
                }
                ui.label(
                    egui::RichText::new(
                        "Installing accepts the package's licence (GPL) and the winget source's terms.",
                    )
                    .small()
                    .weak(),
                );
                if let Some(failure) = &self.failure {
                    ui.colored_label(ui.visuals().error_fg_color, failure);
                }
                ui.add_space(10.0);
                ui.label("Or install it yourself:");
            }
            (None, None) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Looking for winget…");
                });
            }
            (Some(false), None) => {}
        }
        ui.horizontal(|ui| {
            ui.label("In Terminal:");
            ui.code(WINGET_COMMAND);
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(WINGET_COMMAND.to_owned());
            }
        });
        ui.horizontal(|ui| {
            ui.label("Or download the \"release full\" build from");
            ui.hyperlink(FFMPEG_BUILDS);
        });
        ui.label("unzip it, and add its bin folder to the Path environment variable.");
        ui.add_space(12.0);
        if ui
            .add_enabled(self.installing.is_none(), egui::Button::new("Check again"))
            .clicked()
        {
            action = Some(Action::CheckAgain);
        }
        action
    }
}
