//! The windows that explain rather than edit: *About*, and what to do when FFmpeg is missing. Also the
//! *Send to* menu items and the files later launches hand over.

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
                });
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(
                        "Third-party licences are in THIRD-PARTY-NOTICES.html, next to the program.",
                    )
                    .small()
                    .weak(),
                );
            });
    }

    /// Shown when FFmpeg is missing or too old: what to install, and a button to look again.
    pub(super) fn ffmpeg_window(&mut self, ctx: &egui::Context) {
        let Some(problem) = self.ffmpeg_problem.clone() else {
            return;
        };
        let mut open = true;
        let mut check_again = false;
        egui::Window::new("FFmpeg is needed")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(520.0)
            .show(ctx, |ui| {
                let problem = match &problem {
                    Install::Missing(program) => format!(
                        "Quick Video Transcoder reads and encodes videos with FFmpeg, and {program} \
                         cannot be found on this computer."
                    ),
                    Install::TooOld(version) => format!(
                        "The FFmpeg found is too old ({version}). Version {MIN_MAJOR_VERSION} or later \
                         is needed."
                    ),
                    Install::Ready(_) => String::new(),
                };
                ui.label(problem);
                ui.add_space(8.0);
                ui.label("The simplest way to install it: open Terminal (Windows key, type Terminal), run");
                ui.horizontal(|ui| {
                    ui.code(WINGET_COMMAND);
                    if ui.small_button("Copy").clicked() {
                        ui.ctx().copy_text(WINGET_COMMAND.to_owned());
                    }
                });
                ui.label("and accept. Then click Check again here.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Or download the \"release full\" build from");
                    ui.hyperlink(FFMPEG_BUILDS);
                });
                ui.label("unzip it, and add its bin folder to the Path environment variable.");
                ui.add_space(12.0);
                if ui.button("Check again").clicked() {
                    check_again = true;
                }
            });
        if check_again && self.detection.is_none() {
            self.ffmpeg_problem = None;
            self.detection = Some(super::spawn_detection(std::sync::Arc::clone(&self.notify)));
        } else if !open {
            self.ffmpeg_problem = None;
        }
    }
}
