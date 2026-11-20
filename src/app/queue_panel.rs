//! Left panel: the queue, one row per video with its thumbnail, source, kept duration, estimate or
//! progress, and the actions on the selection.

use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, UiBuilder, vec2};

use super::{TranscoderApp, reveal};
use crate::estimate::{Verdict, quality_word};
use crate::queue::{JobId, Phase};
use crate::timecode;

const ROW_HEIGHT: f32 = 74.0;
const THUMB: egui::Vec2 = vec2(96.0, 54.0);

/// Accent of the application: the trim range, the selection.
pub(super) const ACCENT: Color32 = Color32::from_rgb(70, 150, 255);
/// Finished, fits.
pub(super) const GOOD: Color32 = Color32::from_rgb(90, 200, 120);
/// A compromise was needed.
pub(super) const WARN: Color32 = Color32::from_rgb(235, 180, 60);

/// What a row asks for when clicked.
enum RowAction {
    Select { toggle: bool, range: bool },
    Copy,
    Reveal,
    Cancel,
}

impl TranscoderApp {
    pub(super) fn queue_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("Queue");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button("➕ Add…")
                    .on_hover_text("Add videos (Ctrl+O), or drop them here")
                    .clicked()
                {
                    self.add_dialog();
                }
            });
        });
        ui.separator();
        egui::Panel::bottom("queue actions")
            .show_separator_line(true)
            .show(ui, |ui| self.queue_actions(ui));
        if self.queue.jobs.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Drop videos here").size(18.0).weak());
                ui.label(RichText::new("or use Explorer's Send to menu").weak());
            });
            return;
        }
        let mut actions: Vec<(JobId, RowAction)> = Vec::new();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let ids: Vec<JobId> = self.queue.jobs.iter().map(|j| j.id).collect();
                for id in ids {
                    if let Some(action) = self.row(ui, id) {
                        actions.push((id, action));
                    }
                }
            });
        for (id, action) in actions {
            match action {
                RowAction::Select { toggle, range } => self.click_row(id, toggle, range),
                RowAction::Copy => {
                    let files = self
                        .queue
                        .get(id)
                        .and_then(|j| match &j.phase {
                            Phase::Done(encoded) => Some(vec![encoded.output.clone()]),
                            _ => None,
                        })
                        .unwrap_or_default();
                    self.copy_outputs(files);
                }
                RowAction::Reveal => {
                    if let Some(Phase::Done(encoded)) = self.queue.get(id).map(|j| &j.phase)
                        && let Some(dir) = encoded.output.parent()
                    {
                        reveal(dir, Some(&encoded.output));
                    }
                }
                RowAction::Cancel => self.queue.cancel(id),
            }
        }
    }

    fn click_row(&mut self, id: JobId, toggle: bool, range: bool) {
        if range && let Some(anchor) = self.anchor {
            let index = |id| self.queue.jobs.iter().position(|j| j.id == id);
            if let (Some(a), Some(b)) = (index(anchor), index(id)) {
                let (low, high) = (a.min(b), a.max(b));
                if !toggle {
                    self.selection.clear();
                }
                self.selection
                    .extend(self.queue.jobs[low..=high].iter().map(|j| j.id));
            }
        } else if toggle {
            if !self.selection.remove(&id) {
                self.selection.insert(id);
            }
            self.anchor = Some(id);
        } else {
            self.selection.clear();
            self.selection.insert(id);
            self.anchor = Some(id);
        }
        if self.selection.contains(&id) {
            self.focus = Some(id);
        }
    }

    fn row(&mut self, ui: &mut egui::Ui, id: JobId) -> Option<RowAction> {
        let job = self.queue.get(id)?;
        let selected = self.selection.contains(&id);
        let focused = self.focus == Some(id);
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
        let visuals = ui.visuals();
        let fill = if selected {
            ACCENT.gamma_multiply(if focused { 0.35 } else { 0.22 })
        } else if response.hovered() {
            visuals.widgets.hovered.weak_bg_fill
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .rect_filled(rect.shrink2(vec2(0.0, 1.0)), 4.0, fill);
        if focused {
            let bar = egui::Rect::from_min_size(
                rect.min + vec2(0.0, 4.0),
                vec2(3.0, rect.height() - 8.0),
            );
            ui.painter().rect_filled(bar, 1.0, ACCENT);
        }
        let mut action = None;
        if response.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            action = Some(RowAction::Select {
                toggle: modifiers.command,
                range: modifiers.shift,
            });
        }
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(rect.shrink2(vec2(8.0, 6.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        // Thumbnail: the strip's middle frame, or a placeholder.
        let texture = self.strips.get(&id).and_then(|s| {
            let middle = s.textures.len() / 2;
            s.textures[middle..]
                .iter()
                .chain(s.textures[..middle].iter())
                .flatten()
                .next()
                .cloned()
        });
        match texture {
            Some(texture) => {
                child.add(
                    egui::Image::new(egui::load::SizedTexture::new(texture.id(), THUMB))
                        .corner_radius(3.0),
                );
            }
            None => {
                let (thumb, _) = child.allocate_exact_size(THUMB, Sense::hover());
                child
                    .painter()
                    .rect_filled(thumb, 3.0, Color32::from_gray(30));
            }
        }
        child.add_space(8.0);
        child.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add(egui::Label::new(RichText::new(job.name()).strong()).truncate())
                .on_hover_text(job.path.display().to_string());
            if let Some(media) = &job.media {
                let kept = job.trim.duration();
                let trimmed = (kept - media.duration).abs() > 0.05;
                let duration = if trimmed {
                    format!(
                        "{} of {}",
                        timecode::format_short(kept, true),
                        timecode::format_short(media.duration, false)
                    )
                } else {
                    timecode::format_short(kept, true)
                };
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{} · {duration} · {}",
                            media.summary(),
                            job.profile.name
                        ))
                        .small()
                        .weak(),
                    )
                    .truncate(),
                );
            }
            let key = self.queue.key(job);
            match &job.phase {
                Phase::Probing => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Reading…").small());
                    });
                }
                Phase::Unusable(error) => {
                    ui.add(
                        egui::Label::new(
                            RichText::new(error)
                                .small()
                                .color(ui.visuals().error_fg_color),
                        )
                        .truncate(),
                    );
                }
                Phase::Estimating => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Estimating the size…").small());
                    });
                }
                Phase::Waiting => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Waiting for an encoder…").small());
                        if ui.small_button("Cancel").clicked() {
                            action = Some(RowAction::Cancel);
                        }
                    });
                }
                Phase::Encoding(progress) => {
                    ui.horizontal(|ui| {
                        let mut text = format!("{:.0} %", progress.fraction * 100.0);
                        if let Some(speed) = progress.speed {
                            text.push_str(&format!(" · {speed:.1}x"));
                        }
                        if progress.attempt > 1 {
                            text.push_str(&format!(" · retry {}", progress.attempt - 1));
                        }
                        ui.add(
                            egui::ProgressBar::new(progress.fraction)
                                .desired_width(ui.available_width() - 60.0)
                                .desired_height(14.0)
                                .text(RichText::new(text).small()),
                        );
                        if ui.small_button("Cancel").clicked() {
                            action = Some(RowAction::Cancel);
                        }
                    });
                }
                Phase::Done(encoded) => {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("✔ {:.2} MB", encoded.bytes as f64 / 1e6))
                                .small()
                                .color(GOOD),
                        )
                        .on_hover_text(encoded.output.display().to_string());
                        if ui
                            .small_button("Copy")
                            .on_hover_text("Copy the file, to paste it into the conversation")
                            .clicked()
                        {
                            action = Some(RowAction::Copy);
                        }
                        if ui
                            .small_button("Show")
                            .on_hover_text("Show in Explorer")
                            .clicked()
                        {
                            action = Some(RowAction::Reveal);
                        }
                    });
                }
                Phase::Failed(error) => {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("✖ {error}"))
                                .small()
                                .color(ui.visuals().error_fg_color),
                        )
                        .truncate(),
                    )
                    .on_hover_text(error);
                }
                Phase::Idle => match job.current_estimate(key.as_ref()) {
                    Some(Ok(estimate)) => {
                        let color = match estimate.verdict {
                            Verdict::BestQuality => GOOD,
                            Verdict::Fitted => ui.visuals().text_color(),
                            Verdict::BelowFloor => WARN,
                        };
                        let wanted = if job.wanted { " · queued" } else { "" };
                        ui.label(
                            RichText::new(format!(
                                "{} · ≈ {:.1} MB · {}{wanted}",
                                estimate.target.label(),
                                estimate.predicted_bytes as f64 / 1e6,
                                quality_word(estimate.quality)
                            ))
                            .small()
                            .color(color),
                        );
                    }
                    Some(Err(error)) => {
                        ui.add(
                            egui::Label::new(
                                RichText::new(error)
                                    .small()
                                    .color(ui.visuals().error_fg_color),
                            )
                            .truncate(),
                        )
                        .on_hover_text(error);
                    }
                    None => {
                        ui.label(RichText::new("Settings changed…").small().weak());
                    }
                },
            }
        });
        action
    }

    fn queue_actions(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        let selected = self.selected_ids();
        let encodable = |phase: &Phase| !matches!(phase, Phase::Unusable(_) | Phase::Probing);
        let selected_ready = selected
            .iter()
            .filter_map(|id| self.queue.get(*id))
            .filter(|j| encodable(&j.phase) && !j.busy())
            .count();
        let all_ready = self
            .queue
            .jobs
            .iter()
            .filter(|j| encodable(&j.phase) && !j.busy() && !matches!(j.phase, Phase::Done(_)))
            .count();
        ui.horizontal_wrapped(|ui| {
            let encode = egui::Button::new(
                RichText::new(format!("▶ Encode selected ({selected_ready})")).strong(),
            )
            .fill(ACCENT.gamma_multiply(0.6));
            if ui
                .add_enabled(selected_ready > 0, encode)
                .on_hover_text("Encode the selected videos with their estimate (Ctrl+E)")
                .clicked()
            {
                self.encode_selected();
            }
            if ui
                .add_enabled(
                    all_ready > 0,
                    egui::Button::new(format!("Encode all ({all_ready})")),
                )
                .clicked()
            {
                self.encode_all();
            }
            let running: Vec<JobId> = self
                .queue
                .jobs
                .iter()
                .filter(|j| j.wanted || matches!(j.phase, Phase::Waiting | Phase::Encoding(_)))
                .map(|j| j.id)
                .collect();
            if ui
                .add_enabled(!running.is_empty(), egui::Button::new("⏹ Stop all"))
                .clicked()
            {
                for id in running {
                    self.queue.cancel(id);
                }
            }
            let done = self.done_outputs(false);
            if ui
                .add_enabled(
                    !done.is_empty(),
                    egui::Button::new(format!("Copy encoded ({})", done.len())),
                )
                .on_hover_text("Copy every encoded file, to paste them into a conversation")
                .clicked()
            {
                self.copy_outputs(done);
            }
        });
        ui.add_space(2.0);
    }
}
