//! The *Check for updates* window: draws [`Machine`]'s phase and sends its buttons as events. The rules
//! are in [`crate::update`]; the work runs on a worker thread per effect, reporting back through a channel.

use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender, unbounded};
use eframe::egui;

use super::TranscoderApp;
use crate::queue::Phase as JobPhase;
use crate::update::feed::GitHub;
use crate::update::install::{self, Paths};
use crate::update::{Effect, Event, Fault, Machine, Phase, Release, Version};

/// The updater as the window sees it.
pub(super) struct Updater {
    machine: Machine,
    /// The window is shown.
    pub(super) open: bool,
    sender: Sender<Event>,
    receiver: Receiver<Event>,
    /// The data folder and executable are known (else nothing can be installed).
    paths: Option<Paths>,
}

impl Updater {
    /// The updater at start: the outcome of an update the previous run started opens the window.
    pub(super) fn new() -> Self {
        let current = running_version();
        let paths = Paths::current();
        let settled = paths.as_ref().and_then(|p| install::settle(p, current));
        let (sender, receiver) = unbounded();
        Self {
            open: settled.is_some(),
            machine: Machine::new(current, settled.unwrap_or(Phase::Idle)),
            sender,
            receiver,
            paths,
        }
    }
}

/// This executable's version; in debug builds `QVT_UPDATE_AS=0.9.0` pretends to be older, to try an
/// update against the real latest release.
fn running_version() -> Version {
    #[cfg(debug_assertions)]
    if let Some(pretend) = std::env::var("QVT_UPDATE_AS")
        .ok()
        .and_then(|v| Version::parse(&v))
    {
        return pretend;
    }
    Version::current()
}

impl TranscoderApp {
    /// *Help → Check for updates*: opens the window and checks at once.
    pub(super) fn open_updates(&mut self) {
        self.updater.open = true;
        self.send_update(Event::CheckAsked);
    }

    /// Applies an event and runs the effect it asks for.
    fn send_update(&mut self, event: Event) {
        let Some(effect) = self.updater.machine.apply(event) else {
            return;
        };
        let sender = self.updater.sender.clone();
        let notify = Arc::clone(&self.notify);
        match effect {
            Effect::Check => {
                log::info!("update: checking for updates");
                std::thread::spawn(move || {
                    let result = crate::update::feed::Feed::latest(&GitHub::new());
                    match &result {
                        Ok(release) => log::info!("update: latest release is {}", release.version),
                        Err(fault) => log::warn!("update: check failed: {fault:?}"),
                    }
                    let _ = sender.send(Event::Checked(result));
                    notify();
                });
            }
            Effect::Download(release) => {
                let Some(paths) = self.updater.paths.clone() else {
                    let _ = sender.send(Event::Downloaded(Err(Fault::CannotWrite(
                        "no data folder".to_owned(),
                    ))));
                    return;
                };
                log::info!(
                    "update: downloading {} ({} bytes)",
                    release.version,
                    release.exe_size
                );
                std::thread::spawn(move || {
                    let progress_sender = sender.clone();
                    let progress_notify = Arc::clone(&notify);
                    let mut last = 0u64;
                    let result = install::download(&GitHub::new(), &release, &paths, &mut |done| {
                        // A message per 1 % is plenty for a progress bar.
                        if done.saturating_sub(last) > release.exe_size / 100 {
                            last = done;
                            let _ = progress_sender.send(Event::Progress(done));
                            progress_notify();
                        }
                    });
                    match &result {
                        Ok(()) => log::info!("update: downloaded and verified {}", release.version),
                        Err(fault) => log::warn!("update: download failed: {fault:?}"),
                    }
                    let _ = sender.send(Event::Downloaded(result));
                    notify();
                });
            }
            Effect::Restart(release) => {
                let result = self
                    .updater
                    .paths
                    .as_ref()
                    .ok_or_else(|| Fault::Launch("no data folder".to_owned()))
                    .and_then(|paths| install::launch(&release, paths));
                match result {
                    // The swap script waits for this process: close now, the normal way.
                    Ok(()) => self.close_for_update = true,
                    Err(fault) => {
                        log::warn!("update: cannot start the swap: {fault:?}");
                        self.updater.machine.apply(Event::LaunchFailed(fault));
                    }
                }
            }
        }
    }

    /// Takes the worker's results.
    pub(super) fn poll_updates(&mut self) {
        while let Ok(event) = self.updater.receiver.try_recv() {
            self.send_update(event);
        }
    }

    fn encoding(&self) -> bool {
        self.queue
            .jobs
            .iter()
            .any(|j| matches!(j.phase, JobPhase::Encoding(_) | JobPhase::Waiting))
    }

    pub(super) fn update_window(&mut self, ctx: &egui::Context) {
        if !self.updater.open {
            return;
        }
        let mut open = true;
        let mut event = None;
        let encoding = self.encoding();
        let queued = self.queue.jobs.len();
        let phase = self.updater.machine.phase().clone();
        egui::Window::new(crate::tr!("update-title"))
            .id(egui::Id::unique("update window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(crate::tr!(
                    "update-running",
                    version = running_version().to_string()
                ));
                ui.add_space(8.0);
                event = phase_ui(ui, &phase, encoding, queued);
            });
        if let Some(event) = event {
            self.send_update(event);
        }
        // Closing hides the window; a download keeps going and is found again on reopening.
        if !open {
            self.updater.open = false;
        }
    }
}

/// The body of the window for a phase; returns the event a button sent.
fn phase_ui(ui: &mut egui::Ui, phase: &Phase, encoding: bool, queued: usize) -> Option<Event> {
    let mut event = None;
    match phase {
        Phase::Idle => {
            if ui.button(crate::tr!("update-check")).clicked() {
                event = Some(Event::CheckAsked);
            }
        }
        Phase::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(crate::tr!("update-checking"));
            });
        }
        Phase::UpToDate(version) => {
            ui.label(crate::tr!(
                "update-up-to-date",
                version = version.to_string()
            ));
            ui.add_space(8.0);
            if ui.button(crate::tr!("update-check-again")).clicked() {
                event = Some(Event::CheckAsked);
            }
        }
        Phase::Available(release) => {
            let mb = release.exe_size as f64 / 1e6;
            ui.strong(crate::tr!(
                "update-available",
                version = release.version.to_string()
            ));
            notes(ui, release);
            ui.horizontal(|ui| {
                if ui.button(crate::tr!("update-download", mb = mb)).clicked() {
                    event = Some(Event::DownloadAsked);
                }
                ui.hyperlink_to(crate::tr!("update-release-page"), &release.page);
            });
        }
        Phase::Downloading(release, done) => {
            ui.label(crate::tr!(
                "update-downloading",
                version = release.version.to_string()
            ));
            let fraction = *done as f32 / release.exe_size.max(1) as f32;
            ui.add(
                egui::ProgressBar::new(fraction)
                    .show_percentage()
                    .desired_width(420.0),
            );
        }
        Phase::Ready(release) => {
            ui.label(crate::tr!(
                "update-ready",
                version = release.version.to_string()
            ));
            ui.add_space(8.0);
            let restart =
                ui.add_enabled(!encoding, egui::Button::new(crate::tr!("update-restart")));
            if encoding {
                ui.label(
                    egui::RichText::new(crate::tr!("update-wait-for-encodes"))
                        .small()
                        .weak(),
                );
            } else if queued > 0 {
                // The queue lives in memory only: trims and settings of queued videos are not kept.
                ui.label(
                    egui::RichText::new(crate::tr!(
                        "update-queue-cleared",
                        count = (queued as f64)
                    ))
                    .small()
                    .color(ui.visuals().warn_fg_color),
                );
            }
            if restart.clicked() {
                event = Some(Event::RestartAsked);
            }
        }
        Phase::Restarting(release) => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(crate::tr!(
                    "update-restarting",
                    version = release.version.to_string()
                ));
            });
        }
        Phase::Updated(version) => {
            ui.label(crate::tr!("update-updated", version = version.to_string()));
        }
        Phase::Failed { fault, release } => {
            ui.colored_label(ui.visuals().error_fg_color, fault_text(fault));
            ui.label(
                egui::RichText::new(crate::tr!("update-see-log"))
                    .small()
                    .weak(),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                match release {
                    Some(release) if !matches!(fault, Fault::CannotWrite(_)) => {
                        if ui.button(crate::tr!("update-retry")).clicked() {
                            event = Some(Event::DownloadAsked);
                        }
                    }
                    _ => {
                        if ui.button(crate::tr!("update-check-again")).clicked() {
                            event = Some(Event::CheckAsked);
                        }
                    }
                }
                if let Some(release) = release {
                    ui.hyperlink_to(crate::tr!("update-release-page"), &release.page);
                }
            });
        }
    }
    event
}

/// A block of the release notes.
#[derive(Debug, PartialEq, Eq)]
enum Block {
    Heading(String),
    Item(String),
    Paragraph(String),
}

/// The release notes (the changelog section, Markdown) as blocks: `#` headings, `- ` items with their
/// wrapped continuation lines joined, paragraphs; emphasis and code marks dropped; the footer after `---`
/// left out.
fn note_blocks(notes: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for line in notes.lines() {
        let line = line.trim_end();
        if line.starts_with("---") {
            break;
        }
        let clean = |text: &str| text.trim().replace(['*', '`'], "");
        if line.starts_with('#') {
            blocks.push(Block::Heading(clean(line.trim_start_matches('#'))));
        } else if let Some(item) = line.strip_prefix("- ") {
            blocks.push(Block::Item(clean(item)));
        } else if line.is_empty() {
            continue;
        } else if let (true, Some(Block::Item(text) | Block::Paragraph(text))) =
            (line.starts_with(' '), blocks.last_mut())
        {
            text.push(' ');
            text.push_str(&clean(line));
        } else {
            blocks.push(Block::Paragraph(clean(line)));
        }
    }
    blocks
}

fn notes(ui: &mut egui::Ui, release: &Release) {
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for block in note_blocks(&release.notes) {
                match block {
                    Block::Heading(text) => {
                        ui.strong(text);
                    }
                    Block::Item(text) => {
                        ui.label(format!("• {text}"));
                    }
                    Block::Paragraph(text) => {
                        ui.label(text);
                    }
                }
            }
        });
    ui.add_space(6.0);
}

/// The user's sentence for a fault (the technical detail is in the log).
fn fault_text(fault: &Fault) -> String {
    match fault {
        Fault::NoConnection(_) => crate::tr!("update-fault-connection"),
        Fault::Status(status) => crate::tr!("update-fault-status", status = status.to_string()),
        Fault::RateLimited(Some(minutes)) => {
            crate::tr!("update-fault-rate-limited", minutes = (*minutes as f64))
        }
        Fault::RateLimited(None) => crate::tr!("update-fault-rate-limited-later"),
        Fault::Unexpected(_) => crate::tr!("update-fault-unexpected"),
        Fault::NotVerified => crate::tr!("update-fault-not-verified"),
        Fault::CannotWrite(_) => crate::tr!("update-fault-cannot-write"),
        Fault::Launch(reason) => crate::tr!("update-fault-install", detail = reason.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_notes_join_wrapped_items() {
        let notes = "The first release.\n\n### Added\n\n- A queue, filled from *Send to*,\n  with `I` / `O`.\n- One file.\n\n---\n\nBuilt from abc.";
        assert_eq!(
            note_blocks(notes),
            [
                Block::Paragraph("The first release.".to_owned()),
                Block::Heading("Added".to_owned()),
                Block::Item("A queue, filled from Send to, with I / O.".to_owned()),
                Block::Item("One file.".to_owned()),
            ]
        );
    }
}
