//! Bottom of the preview: playback buttons, the trim timeline with its two handles, and the thumbnail
//! strip. Dragging a handle shows the frame under it, so the kept part is cut on what is seen.

use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Rect, Sense, Stroke, pos2, vec2};

use super::TranscoderApp;
use super::queue_panel::ACCENT;
use crate::queue::Trim;
use crate::timecode;

const BAR_HEIGHT: f32 = 30.0;
const STRIP_HEIGHT: f32 = 50.0;
const HANDLE_WIDTH: f32 = 10.0;

impl TranscoderApp {
    pub(super) fn timeline(&mut self, ui: &mut egui::Ui) {
        let Some((id, duration, fps, trim)) = self.playing_job.and_then(|id| {
            let job = self.queue.get(id)?;
            let media = job.media.as_ref()?;
            Some((id, media.duration, media.fps(), job.trim))
        }) else {
            ui.add_space(4.0);
            return;
        };
        ui.add_space(6.0);
        self.transport(ui, duration, trim);
        ui.add_space(4.0);
        self.trim_bar(ui, id, duration, fps, trim);
        self.strip(ui, id, duration, trim);
        ui.add_space(6.0);
    }

    fn transport(&mut self, ui: &mut egui::Ui, duration: f64, trim: Trim) {
        ui.horizontal(|ui| {
            if ui
                .button("⏮")
                .on_hover_text("Start of the kept part (Home)")
                .clicked()
            {
                self.player.seek(trim.start);
            }
            if ui
                .button("◀|")
                .on_hover_text("Previous frame (Left)")
                .clicked()
            {
                self.player.step(-1);
            }
            let play = if self.player.is_playing() {
                "⏸"
            } else {
                "▶"
            };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(play).size(16.0))
                        .min_size(vec2(36.0, 0.0)),
                )
                .on_hover_text("Play / pause the kept part (Space)")
                .clicked()
            {
                self.toggle_playback();
            }
            if ui
                .button("|▶")
                .on_hover_text("Next frame (Right)")
                .clicked()
            {
                self.player.step(1);
            }
            if ui
                .button("⏭")
                .on_hover_text("End of the kept part (End)")
                .clicked()
            {
                self.player.seek(trim.end);
            }
            ui.separator();
            ui.monospace(format!(
                "{} / {}",
                timecode::format_precise(self.player.position()),
                timecode::format_short(duration, true)
            ));
            ui.separator();
            if ui
                .button("[ Start here")
                .on_hover_text("Start the kept part at the current frame (I)")
                .clicked()
            {
                self.set_trim_start(self.player.position());
            }
            if ui
                .button("End here ]")
                .on_hover_text("End the kept part after the current frame (O)")
                .clicked()
            {
                self.set_trim_end(self.player.position());
            }
            ui.label(
                egui::RichText::new(format!(
                    "kept {}",
                    timecode::format_short(trim.duration(), true)
                ))
                .color(ACCENT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().slider_width = 90.0;
                let available = self.player.has_audio();
                ui.add_enabled_ui(available, |ui| {
                    let volume = ui
                        .add(egui::Slider::new(&mut self.prefs.volume, 0.0..=1.0).show_value(false))
                        .on_hover_text(format!("Volume {:.0} %", self.prefs.volume * 100.0));
                    if volume.changed() {
                        self.prefs.muted = false;
                    }
                    let icon = if self.prefs.muted || !available {
                        "🔇"
                    } else {
                        "🔊"
                    };
                    if ui
                        .add(egui::Button::new(icon).selected(self.prefs.muted))
                        .on_hover_text("Mute / unmute (M)")
                        .clicked()
                    {
                        self.prefs.muted = !self.prefs.muted;
                    }
                })
                .response
                .on_disabled_hover_text("No sound");
            });
        });
    }

    fn trim_bar(&mut self, ui: &mut egui::Ui, id: u64, duration: f64, fps: f64, trim: Trim) {
        let width = ui.available_width();
        let (rect, response) =
            ui.allocate_exact_size(vec2(width, BAR_HEIGHT), Sense::click_and_drag());
        let painter = ui.painter_at(rect.expand(2.0));
        let span = duration.max(1e-6);
        let x_of = |t: f64| rect.left() + (t / span) as f32 * rect.width();
        let t_of = |x: f32| ((x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64 * span;

        painter.rect_filled(rect, 4.0, Color32::from_gray(34));
        let kept = Rect::from_x_y_ranges(x_of(trim.start)..=x_of(trim.end), rect.y_range());
        painter.rect_filled(kept, 0.0, ACCENT.gamma_multiply(0.35));
        painter.rect_stroke(
            kept,
            0.0,
            Stroke::new(1.0, ACCENT),
            egui::StrokeKind::Inside,
        );

        // Ticks every 10 s, longer every minute.
        let tick = Stroke::new(1.0, Color32::from_gray(80));
        let step = if duration > 600.0 { 60.0 } else { 10.0 };
        let mut t = step;
        while t < duration {
            let long = (t as u64).is_multiple_of(60);
            let height = if long { 9.0 } else { 5.0 };
            painter.line_segment(
                [
                    pos2(x_of(t), rect.bottom() - height),
                    pos2(x_of(t), rect.bottom()),
                ],
                tick,
            );
            t += step;
        }

        // Handles, interacted after the bar so they win the hit test.
        let handle = |x: f32| {
            Rect::from_center_size(
                pos2(x, rect.center().y),
                vec2(HANDLE_WIDTH, BAR_HEIGHT + 4.0),
            )
        };
        let start_rect = handle(x_of(trim.start));
        let end_rect = handle(x_of(trim.end));
        let start = ui.interact(start_rect, ui.id().with(("trim start", id)), Sense::drag());
        let end = ui.interact(end_rect, ui.id().with(("trim end", id)), Sense::drag());
        for (r, resp) in [(start_rect, &start), (end_rect, &end)] {
            let active = resp.hovered() || resp.dragged();
            let fill = if active { Color32::WHITE } else { ACCENT };
            painter.rect_filled(r.shrink2(vec2(2.0, 0.0)), 2.0, fill);
            painter.line_segment(
                [
                    pos2(r.center().x, r.top() + 8.0),
                    pos2(r.center().x, r.bottom() - 8.0),
                ],
                Stroke::new(1.0, Color32::from_gray(30)),
            );
            if active {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            }
        }
        let frame = 1.0 / fps.max(1.0);
        if let Some(pointer) = start.interact_pointer_pos().filter(|_| start.dragged()) {
            let t = t_of(pointer.x);
            self.set_trim_start(t);
            let shown = self.queue.get(id).map_or(t, |j| j.trim.start);
            self.player.pause();
            self.player.seek(shown);
        }
        if let Some(pointer) = end.interact_pointer_pos().filter(|_| end.dragged()) {
            let t = t_of(pointer.x);
            // The handle sits at the end of the last kept frame: show that frame.
            self.set_trim_end(t - frame);
            let shown = self.queue.get(id).map_or(t, |j| j.trim.end - frame);
            self.player.pause();
            self.player.seek(shown);
        }

        // Shift + drag moves the kept part as a whole; a plain click or drag seeks.
        let shift = ui.input(|i| i.modifiers.shift);
        if response.dragged() && shift {
            let delta = f64::from(response.drag_delta().x / rect.width()) * span;
            if let Some(job) = self.queue.get_mut(id) {
                let length = job.trim.duration();
                let start = (job.trim.start + delta).clamp(0.0, (duration - length).max(0.0));
                if start != job.trim.start {
                    job.trim = Trim {
                        start,
                        end: start + length,
                    };
                    job.touch();
                }
            }
        } else if (response.clicked() || response.dragged())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            self.player.seek(t_of(pointer.x));
        }

        let x = x_of(self.player.position());
        painter.line_segment(
            [pos2(x, rect.top() - 2.0), pos2(x, rect.bottom() + 2.0)],
            Stroke::new(2.0, Color32::WHITE),
        );
        if let Some(pointer) = response.hover_pos() {
            let t = t_of(pointer.x);
            painter.line_segment(
                [pos2(pointer.x, rect.top()), pos2(pointer.x, rect.bottom())],
                Stroke::new(1.0, Color32::from_white_alpha(70)),
            );
            let (anchor, offset) = if pointer.x > rect.center().x {
                (Align2::RIGHT_CENTER, -6.0)
            } else {
                (Align2::LEFT_CENTER, 6.0)
            };
            painter.text(
                pos2(pointer.x + offset, rect.center().y),
                anchor,
                timecode::format_short(t, true),
                FontId::monospace(12.0),
                Color32::WHITE,
            );
        }
    }

    fn strip(&mut self, ui: &mut egui::Ui, id: u64, duration: f64, trim: Trim) {
        let width = ui.available_width();
        let (rect, response) =
            ui.allocate_exact_size(vec2(width, STRIP_HEIGHT), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(20));
        let Some(strip) = self.strips.get(&id) else {
            return;
        };
        let count = strip.textures.len().max(1);
        let cell = rect.width() / count as f32;
        for (index, texture) in strip.textures.iter().enumerate() {
            let cell_rect = Rect::from_min_size(
                pos2(rect.left() + cell * index as f32, rect.top()),
                vec2(cell, rect.height()),
            );
            let Some(texture) = texture else { continue };
            // Cover the cell, cropping the picture's sides.
            let [tw, th] = texture.size();
            let picture = tw as f32 / th.max(1) as f32;
            let cell_ratio = cell_rect.width() / cell_rect.height();
            let uv = if picture > cell_ratio {
                let keep = cell_ratio / picture;
                Rect::from_min_max(pos2(0.5 - keep / 2.0, 0.0), pos2(0.5 + keep / 2.0, 1.0))
            } else {
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
            };
            painter.image(texture.id(), cell_rect.shrink(0.5), uv, Color32::WHITE);
        }
        // Dim what is cut away.
        let span = duration.max(1e-6);
        let x_of = |t: f64| rect.left() + (t / span) as f32 * rect.width();
        let shade = Color32::from_black_alpha(170);
        painter.rect_filled(
            Rect::from_x_y_ranges(rect.left()..=x_of(trim.start), rect.y_range()),
            0.0,
            shade,
        );
        painter.rect_filled(
            Rect::from_x_y_ranges(x_of(trim.end)..=rect.right(), rect.y_range()),
            0.0,
            shade,
        );
        let x = x_of(self.player.position());
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(2.0, Color32::WHITE),
        );
        if (response.clicked() || response.dragged())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let t = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64 * span;
            self.player.seek(t);
        }
    }
}
