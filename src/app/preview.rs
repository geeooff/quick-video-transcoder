//! Central panel: the focused video as it will be encoded (output size, tone mapping), with an overlay
//! saying what is shown.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, pos2, vec2};

use super::TranscoderApp;
use crate::ffmpeg::probe::codec_label;

impl TranscoderApp {
    pub(super) fn preview(&mut self, ui: &mut egui::Ui) {
        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, Sense::click());
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, Color32::from_gray(12));
        if response.clicked() && self.playing_job.is_some() {
            self.toggle_playback();
        }
        let (Some(texture), Some(size)) = (&self.frame_texture, self.frame_size) else {
            let text = if self.queue.jobs.is_empty() {
                "Drop game captures here, or add them with Ctrl+O"
            } else if self.playing_job.is_some() {
                "Decoding…"
            } else {
                "Select a video"
            };
            painter.text(
                area.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::proportional(18.0),
                Color32::GRAY,
            );
            return;
        };
        let (width, height) = (size.width as f32, size.height as f32);
        // Fit, at most two screen pixels per output pixel (sharpness stays judgeable).
        let ppp = ui.ctx().pixels_per_point();
        let scale = (area.width() / width)
            .min(area.height() / height)
            .min(1.0 / ppp * 2.0);
        let image = Rect::from_center_size(area.center(), vec2(width, height) * scale);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), image, uv, Color32::WHITE);
        self.overlay(&painter, area);
    }

    fn overlay(&self, painter: &egui::Painter, area: Rect) {
        let Some(job) = self.playing_job.and_then(|id| self.queue.get(id)) else {
            return;
        };
        let Some(media) = &job.media else { return };
        let mut lines = Vec::new();
        if let Some(size) = self.frame_size {
            let mut line = format!(
                "Preview {size} · source {}x{} {}",
                media.width,
                media.height,
                codec_label(&media.codec)
            );
            if media.dynamic.is_hdr() {
                let shown = if job.profile.sdr_only {
                    "tone mapped as encoded"
                } else {
                    "kept HDR, shown tone mapped"
                };
                line.push_str(&format!(" · {} {shown}", media.dynamic.label()));
            }
            lines.push(line);
        }
        if self.player.is_loading() {
            lines.push("Seeking…".to_owned());
        }
        if let Some(error) = self.player.error() {
            lines.push(format!("Decoder: {error}"));
        }
        let font = FontId::monospace(12.5);
        let mut pos = area.left_top() + vec2(10.0, 8.0);
        for line in lines {
            let galley = painter.layout_no_wrap(line, font.clone(), Color32::WHITE);
            let rect = Rect::from_min_size(pos, galley.size()).expand(4.0);
            painter.rect_filled(rect, 3.0, Color32::from_black_alpha(160));
            painter.galley(pos, galley, Color32::WHITE);
            pos.y += rect.height() + 4.0;
        }
    }
}
