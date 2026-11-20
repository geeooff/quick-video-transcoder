//! Right panel: the settings of the selected videos (profile, limits, sound, quality, kept part), the
//! estimate of the focused one, and the output and encoder preferences. Also the profiles window.

use eframe::egui::{self, RichText};

use super::TranscoderApp;
use super::queue_panel::{GOOD, WARN};
use crate::estimate::{Verdict, quality_word};
use crate::ffmpeg::encoders::{EncoderBackend, VideoCodec};
use crate::plan;
use crate::profile::{AudioChoice, Profile, ToneMapping};
use crate::queue::JobId;
use crate::timecode;

/// Copies to `target` every field that differs between `before` and `after`: an edit made with several
/// videos selected changes that field on each of them, and nothing else.
pub(super) fn apply_changes(target: &mut Profile, before: &Profile, after: &Profile) {
    macro_rules! fields {
        ($($field:ident),*) => {
            $(if before.$field != after.$field {
                target.$field = after.$field.clone();
            })*
        };
    }
    fields!(
        max_size_mb,
        max_short_side,
        min_short_side,
        max_fps,
        min_fps,
        codec,
        sdr_only,
        tone_mapping,
        audio_bitrate_kbps,
        audio,
        quality_ceiling,
        quality_floor,
        suffix
    );
}

const SHORT_SIDES: [u32; 8] = [2160, 1440, 1080, 900, 720, 540, 480, 360];
const FRAME_RATES: [f64; 6] = [120.0, 60.0, 50.0, 30.0, 25.0, 24.0];
const AUDIO_RATES: [u32; 6] = [64, 96, 128, 160, 192, 256];

/// What the user did in a time code field.
enum TimeEdit {
    Unchanged,
    Set(f64),
    Invalid(String),
}

/// A time code field showing `value`, applied when it loses the focus after the user typed in it.
fn time_edit(ui: &mut egui::Ui, text: &mut String, value: f64) -> TimeEdit {
    let response = ui.add(
        egui::TextEdit::singleline(text)
            .desired_width(110.0)
            .font(egui::TextStyle::Monospace),
    );
    // `lost_focus` lasts two frames, the second with the text already reset: apply only a typed edit.
    let typed = response.id;
    if response.changed() {
        ui.data_mut(|data| data.insert_temp(typed, true));
    }
    let mut edit = TimeEdit::Unchanged;
    if response.lost_focus()
        && ui.data_mut(|data| data.remove_temp::<bool>(typed).unwrap_or(false))
        && !ui.input(|input| input.key_pressed(egui::Key::Escape))
    {
        edit = match timecode::parse(text) {
            Some(seconds) => TimeEdit::Set(seconds),
            None => TimeEdit::Invalid(text.trim().to_owned()),
        };
    }
    if !response.has_focus() {
        *text = timecode::format_precise(value);
    }
    edit
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).strong().size(15.0));
    ui.add_space(2.0);
}

impl TranscoderApp {
    pub(super) fn settings_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(6.0);
                let selected = self.selected_ids();
                let focus = self
                    .focus
                    .filter(|f| selected.contains(f))
                    .or(selected.first().copied());
                match focus.and_then(|id| self.queue.get(id)) {
                    Some(job) => {
                        if selected.len() > 1 {
                            ui.heading(format!("{} videos selected", selected.len()));
                            ui.label(RichText::new("A change applies to each of them").weak());
                        } else {
                            ui.add(
                                egui::Label::new(RichText::new(job.name()).heading()).truncate(),
                            )
                            .on_hover_text(job.path.display().to_string());
                        }
                    }
                    None => {
                        ui.heading("No video selected");
                    }
                }
                if let Some(focus) = focus {
                    self.profile_section(ui, focus, &selected);
                    self.trim_section(ui, focus, selected.len());
                    self.estimate_section(ui, focus);
                }
                self.output_section(ui);
            });
    }

    fn profile_section(&mut self, ui: &mut egui::Ui, focus: JobId, selected: &[JobId]) {
        let Some(job) = self.queue.get(focus) else {
            return;
        };
        let before = job.profile.clone();
        let media = job.media.clone();
        let origin = self.profiles.find(&job.profile_name).cloned();
        let modified = origin
            .as_ref()
            .is_some_and(|p| p.name == before.name && *p != before);

        section(ui, "Profile");
        let mut chosen: Option<Profile> = None;
        ui.horizontal(|ui| {
            let label = if modified {
                format!("{} (changed)", job.profile_name)
            } else {
                job.profile_name.clone()
            };
            egui::ComboBox::from_id_salt("profile")
                .selected_text(label)
                .width(180.0)
                .show_ui(ui, |ui| {
                    for profile in &self.profiles.profiles {
                        if ui
                            .selectable_label(profile.name == job.profile_name, &profile.name)
                            .clicked()
                        {
                            chosen = Some(profile.clone());
                        }
                    }
                });
            if modified
                && let Some(origin) = &origin
                && ui
                    .button("Reset")
                    .on_hover_text("Back to the profile's settings")
                    .clicked()
            {
                chosen = Some(origin.clone());
            }
        });
        if let Some(profile) = chosen {
            for id in selected {
                if let Some(job) = self.queue.get_mut(*id) {
                    job.profile = profile.clone();
                    job.profile_name = profile.name.clone();
                    job.touch();
                }
            }
            self.prefs.profile = profile.name;
            return;
        }

        let mut after = before.clone();
        let codecs: Vec<VideoCodec> = VideoCodec::ALL
            .into_iter()
            .filter(|&c| !self.queue.capabilities.backends(c).is_empty())
            .collect();
        egui::Grid::new("limits")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Size limit");
                ui.add(
                    egui::DragValue::new(&mut after.max_size_mb)
                        .range(0.5..=4000.0)
                        .speed(0.5)
                        .max_decimals(1)
                        .suffix(" MB"),
                )
                .on_hover_text("Megabytes of 1,000,000 bytes");
                ui.end_row();

                ui.label("Resolution");
                egui::ComboBox::from_id_salt("short side")
                    .selected_text(format!("up to {}p", after.max_short_side))
                    .show_ui(ui, |ui| {
                        for short in SHORT_SIDES {
                            ui.selectable_value(
                                &mut after.max_short_side,
                                short,
                                format!("{short}p"),
                            );
                        }
                    });
                ui.end_row();

                ui.label("Frame rate");
                egui::ComboBox::from_id_salt("fps")
                    .selected_text(format!("up to {} fps", after.max_fps))
                    .show_ui(ui, |ui| {
                        for fps in FRAME_RATES {
                            ui.selectable_value(&mut after.max_fps, fps, format!("{fps} fps"));
                        }
                    });
                ui.end_row();

                ui.label("Codec");
                egui::ComboBox::from_id_salt("codec")
                    .selected_text(after.codec.label())
                    .show_ui(ui, |ui| {
                        for codec in &codecs {
                            ui.selectable_value(&mut after.codec, *codec, codec.label());
                        }
                    });
                ui.end_row();

                ui.label("HDR");
                ui.add_enabled(
                    after.codec.supports_hdr(),
                    egui::Checkbox::new(&mut after.sdr_only, "Convert to SDR"),
                )
                .on_hover_text(
                    "Off: an HDR video stays HDR (10-bit); phones without HDR show it washed out",
                )
                .on_disabled_hover_text("H.264 cannot carry HDR");
                ui.end_row();

                let hdr_source = media.as_ref().is_some_and(|m| m.dynamic.is_hdr());
                ui.label("Tone mapping");
                ui.add_enabled_ui(hdr_source || selected.len() > 1, |ui| {
                    egui::ComboBox::from_id_salt("tone")
                        .selected_text(after.tone_mapping.label())
                        .show_ui(ui, |ui| {
                            for tone in ToneMapping::ALL {
                                ui.selectable_value(&mut after.tone_mapping, tone, tone.label());
                            }
                        });
                })
                .response
                .on_disabled_hover_text("This video is SDR");
                ui.end_row();

                ui.label("Sound");
                egui::ComboBox::from_id_salt("audio")
                    .selected_text(audio_label(after.audio, media.as_ref()))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut after.audio, AudioChoice::First, "First track");
                        if let Some(media) = &media
                            && media.audio.len() > 1
                        {
                            for track in &media.audio {
                                let choice = AudioChoice::Track(track.index);
                                ui.selectable_value(
                                    &mut after.audio,
                                    choice,
                                    audio_label(choice, Some(media)),
                                );
                            }
                        }
                        ui.selectable_value(
                            &mut after.audio,
                            AudioChoice::MixAll,
                            "Every track mixed",
                        );
                        ui.selectable_value(&mut after.audio, AudioChoice::None, "No sound");
                    });
                ui.end_row();

                ui.label("Sound bitrate");
                egui::ComboBox::from_id_salt("audio rate")
                    .selected_text(format!("{} kbit/s", after.audio_bitrate_kbps))
                    .show_ui(ui, |ui| {
                        for rate in AUDIO_RATES {
                            ui.selectable_value(
                                &mut after.audio_bitrate_kbps,
                                rate,
                                format!("{rate} kbit/s"),
                            );
                        }
                    });
                ui.end_row();
            });
        egui::CollapsingHeader::new("Quality bounds")
            .id_salt("quality bounds")
            .show(ui, |ui| {
                egui::Grid::new("quality").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Best worth it").on_hover_text(
                        "Past this quality the extra bytes are not visible: an easy clip ends up smaller than the limit",
                    );
                    ui.add(
                        egui::Slider::new(&mut after.quality_ceiling, 14..=30)
                            .clamping(egui::SliderClamping::Edits)
                            .custom_formatter(|v, _| format!("QP {v} ({})", quality_word(v))),
                    );
                    ui.end_row();
                    ui.label("Worst accepted").on_hover_text(
                        "Below this quality the frame rate, then the resolution, are lowered",
                    );
                    ui.add(
                        egui::Slider::new(&mut after.quality_floor, 24..=42)
                            .clamping(egui::SliderClamping::Edits)
                            .custom_formatter(|v, _| format!("QP {v} ({})", quality_word(v))),
                    );
                    ui.end_row();
                    ui.label("Lowest resolution");
                    egui::ComboBox::from_id_salt("min short side")
                        .selected_text(format!("{}p", after.min_short_side))
                        .show_ui(ui, |ui| {
                            for short in SHORT_SIDES.into_iter().filter(|&s| s <= after.max_short_side) {
                                ui.selectable_value(&mut after.min_short_side, short, format!("{short}p"));
                            }
                        });
                    ui.end_row();
                    ui.label("Lowest frame rate");
                    egui::ComboBox::from_id_salt("min fps")
                        .selected_text(format!("{} fps", after.min_fps))
                        .show_ui(ui, |ui| {
                            for fps in FRAME_RATES.into_iter().filter(|&f| f <= after.max_fps) {
                                ui.selectable_value(&mut after.min_fps, fps, format!("{fps} fps"));
                            }
                        });
                    ui.end_row();
                    ui.label("File name suffix");
                    ui.add(egui::TextEdit::singleline(&mut after.suffix).desired_width(120.0));
                    ui.end_row();
                });
            });
        after.sanitize();
        if after != before {
            for id in selected {
                if let Some(job) = self.queue.get_mut(*id) {
                    let mut profile = job.profile.clone();
                    apply_changes(&mut profile, &before, &after);
                    profile.sanitize();
                    if profile != job.profile {
                        job.profile = profile;
                        job.touch();
                    }
                }
            }
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_profile_name)
                    .hint_text("Profile name")
                    .desired_width(150.0),
            );
            let name = self.new_profile_name.trim().to_owned();
            let exists = self.profiles.find(&name).is_some();
            let label = if exists {
                "Update profile"
            } else {
                "Save as profile"
            };
            if ui
                .add_enabled(!name.is_empty(), egui::Button::new(label))
                .on_hover_text("Keep these settings as a profile for later videos")
                .clicked()
            {
                let mut profile = after.clone();
                profile.name.clone_from(&name);
                match self.profiles.profiles.iter_mut().find(|p| p.name == name) {
                    Some(existing) => *existing = profile,
                    None => self.profiles.profiles.push(profile),
                }
                if let Some(job) = self.queue.get_mut(focus) {
                    job.profile.name.clone_from(&name);
                    job.profile_name.clone_from(&name);
                }
                self.save_profiles();
                self.new_profile_name.clear();
            }
        });
    }

    fn trim_section(&mut self, ui: &mut egui::Ui, focus: JobId, selected: usize) {
        let Some(job) = self.queue.get(focus) else {
            return;
        };
        let Some(duration) = job.media.as_ref().map(|m| m.duration) else {
            return;
        };
        section(ui, "Kept part");
        if selected > 1 {
            ui.label(RichText::new("The kept part is set video by video, on the timeline").weak());
        }
        let trim = job.trim;
        let mut start_text = ui
            .data(|d| d.get_temp::<String>(egui::Id::new(("trim start", focus))))
            .unwrap_or_default();
        let mut end_text = ui
            .data(|d| d.get_temp::<String>(egui::Id::new(("trim end", focus))))
            .unwrap_or_default();
        let mut start_edit = TimeEdit::Unchanged;
        let mut end_edit = TimeEdit::Unchanged;
        egui::Grid::new("trim")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Start");
                ui.push_id(("start", focus), |ui| {
                    start_edit = time_edit(ui, &mut start_text, trim.start)
                });
                ui.end_row();
                ui.label("End");
                ui.push_id(("end", focus), |ui| {
                    end_edit = time_edit(ui, &mut end_text, trim.end)
                });
                ui.end_row();
                ui.label("Duration");
                ui.monospace(timecode::format_precise(trim.duration()));
                ui.end_row();
            });
        ui.data_mut(|d| {
            d.insert_temp(egui::Id::new(("trim start", focus)), start_text);
            d.insert_temp(egui::Id::new(("trim end", focus)), end_text);
        });
        let reset = ui
            .add_enabled(
                trim.start > 0.0 || trim.end < duration,
                egui::Button::new("Keep everything"),
            )
            .clicked();
        if self.focus == Some(focus) {
            match start_edit {
                TimeEdit::Set(seconds) => self.set_trim_start(seconds),
                TimeEdit::Invalid(text) => self.error(format!("not a time: {text}")),
                TimeEdit::Unchanged => {}
            }
            match end_edit {
                // The field shows the end itself, not the frame under the playhead.
                TimeEdit::Set(seconds) => {
                    let frame = self
                        .queue
                        .get(focus)
                        .and_then(|j| j.media.as_ref())
                        .map_or(0.0, |m| 1.0 / m.fps());
                    self.set_trim_end(seconds - frame);
                }
                TimeEdit::Invalid(text) => self.error(format!("not a time: {text}")),
                TimeEdit::Unchanged => {}
            }
        }
        if reset && let Some(job) = self.queue.get_mut(focus) {
            job.trim.start = 0.0;
            job.trim.end = duration;
            job.touch();
        }
    }

    fn estimate_section(&mut self, ui: &mut egui::Ui, focus: JobId) {
        let Some(job) = self.queue.get(focus) else {
            return;
        };
        let Some(media) = &job.media else { return };
        section(ui, "Estimate");
        let key = self.queue.key(job);
        let duration = job.trim.duration();
        let audio = plan::audio_kbps(media, &job.profile);
        let budget = plan::video_budget(job.profile.max_bytes(), duration, audio);
        match job.current_estimate(key.as_ref()) {
            Some(Ok(estimate)) => {
                let (text, color) = match estimate.verdict {
                    Verdict::BestQuality => (
                        "Fits at the best quality worth it: the file will be smaller than the limit",
                        GOOD,
                    ),
                    Verdict::Fitted => ("Fills the size limit", ui.visuals().text_color()),
                    Verdict::BelowFloor => (
                        "Below the worst accepted quality even at the lowest settings: shorten the kept part",
                        WARN,
                    ),
                };
                ui.label(RichText::new(text).color(color));
                egui::Grid::new("estimate")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Output");
                        ui.label(format!(
                            "{} at {} fps ({})",
                            estimate.target.size,
                            estimate.target.rate.label(),
                            estimate.target.label()
                        ));
                        ui.end_row();
                        ui.label("Video bitrate");
                        ui.label(format!("{:.0} kbit/s", estimate.video_bps / 1000.0));
                        ui.end_row();
                        ui.label("Quality");
                        ui.label(format!(
                            "{} (QP {:.1})",
                            quality_word(estimate.quality),
                            estimate.quality
                        ));
                        ui.end_row();
                        ui.label("Size");
                        ui.label(format!(
                            "≈ {:.1} MB of {:.1} MB",
                            estimate.predicted_bytes as f64 / 1e6,
                            job.profile.max_size_mb
                        ));
                        ui.end_row();
                        ui.label("Measured");
                        ui.label(format!("{} sample encode(s)", estimate.measurements));
                        ui.end_row();
                    });
            }
            Some(Err(error)) => {
                ui.label(RichText::new(error).color(ui.visuals().error_fg_color));
            }
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Estimating…");
                });
                if let Some(bps) = budget {
                    ui.label(
                        RichText::new(format!("Budget: {:.0} kbit/s of video", bps / 1000.0))
                            .weak(),
                    );
                }
            }
        }
        if let Some(engine) = job.engine {
            ui.label(
                RichText::new(format!("Filters: {}", engine.label()))
                    .weak()
                    .small(),
            );
        }
    }

    fn output_section(&mut self, ui: &mut egui::Ui) {
        section(ui, "Output");
        let mut settings = self.queue.settings.clone();
        ui.horizontal(|ui| {
            let folder = settings.output_dir.as_ref().map_or_else(
                || "Next to each video".to_owned(),
                |d| d.display().to_string(),
            );
            ui.add(egui::Label::new(folder).truncate());
        });
        ui.horizontal(|ui| {
            if ui.button("Choose folder…").clicked() {
                let mut dialog = rfd::FileDialog::new().set_title("Output folder");
                if let Some(dir) = settings
                    .output_dir
                    .as_ref()
                    .or(self.prefs.last_dir.as_ref())
                {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(dir) = dialog.pick_folder() {
                    settings.output_dir = Some(dir);
                }
            }
            if ui
                .add_enabled(
                    settings.output_dir.is_some(),
                    egui::Button::new("Next to each video"),
                )
                .clicked()
            {
                settings.output_dir = None;
            }
        });
        egui::Grid::new("encoder")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Encoder");
                let label = settings.backend.map_or("Automatic", EncoderBackend::label);
                egui::ComboBox::from_id_salt("backend")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.backend, None, "Automatic");
                        for backend in EncoderBackend::ALL {
                            let works = VideoCodec::ALL
                                .into_iter()
                                .any(|c| self.queue.capabilities.has_encoder(c, backend));
                            if works {
                                ui.selectable_value(
                                    &mut settings.backend,
                                    Some(backend),
                                    backend.label(),
                                );
                            }
                        }
                    });
                ui.end_row();
                ui.label("Encodes at once");
                ui.add(egui::Slider::new(&mut settings.parallel_encodes, 1..=6))
                    .on_hover_text(
                        "Two saturate an RTX 4090's two NVENC engines; more only queue up",
                    );
                ui.end_row();
            });
        if settings != self.queue.settings {
            self.prefs.parallel_encodes = settings.parallel_encodes;
            self.prefs.backend = settings.backend;
            self.prefs.output_dir.clone_from(&settings.output_dir);
            self.queue.set_settings(settings);
        }
    }

    pub(super) fn profiles_window(&mut self, ctx: &egui::Context) {
        if !self.show_profiles {
            return;
        }
        let mut open = true;
        let mut changed = false;
        let mut remove = None;
        let mut raise = None;
        egui::Window::new("Profiles")
            .open(&mut open)
            .collapsible(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label("The first profile is given to new videos. A profile is changed by saving a video's settings under its name.");
                ui.add_space(6.0);
                let count = self.profiles.profiles.len();
                egui::Grid::new("profiles list").striped(true).num_columns(4).show(ui, |ui| {
                    for (index, profile) in self.profiles.profiles.iter_mut().enumerate() {
                        let response = ui.add(egui::TextEdit::singleline(&mut profile.name).desired_width(140.0));
                        if response.lost_focus() && response.changed() {
                            changed = true;
                        }
                        changed |= response.changed();
                        ui.label(
                            RichText::new(format!(
                                "{:.0} MB · {}p{} · {}{}",
                                profile.max_size_mb,
                                profile.max_short_side,
                                profile.max_fps,
                                profile.codec.label(),
                                if profile.sdr_only { " · SDR" } else { "" }
                            ))
                            .weak(),
                        );
                        if ui.add_enabled(index > 0, egui::Button::new("▲")).on_hover_text("Move up").clicked() {
                            raise = Some(index);
                        }
                        if ui.add_enabled(count > 1, egui::Button::new("✖")).on_hover_text("Delete").clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
                ui.add_space(6.0);
                if ui.button("Add the built-in Messenger profile").clicked() {
                    let mut profile = Profile::messenger();
                    if self.profiles.find(&profile.name).is_some() {
                        profile.name = format!("Messenger {}", self.profiles.profiles.len() + 1);
                    }
                    self.profiles.profiles.push(profile);
                    changed = true;
                }
            });
        if let Some(index) = raise {
            self.profiles.profiles.swap(index, index - 1);
            changed = true;
        }
        if let Some(index) = remove {
            self.profiles.profiles.remove(index);
            changed = true;
        }
        if changed {
            for profile in &mut self.profiles.profiles {
                profile.sanitize();
            }
            self.save_profiles();
        }
        self.show_profiles = open;
    }
}

/// How an audio choice reads for a video.
fn audio_label(choice: AudioChoice, media: Option<&crate::ffmpeg::probe::MediaInfo>) -> String {
    match choice {
        AudioChoice::First => "First track".to_owned(),
        AudioChoice::Track(index) => {
            let title = media
                .and_then(|m| m.audio.get(index))
                .and_then(|t| t.title.clone())
                .map(|t| format!(" ({t})"))
                .unwrap_or_default();
            format!("Track {}{title}", index + 1)
        }
        AudioChoice::MixAll => "Every track mixed".to_owned(),
        AudioChoice::None => "No sound".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_changes_only_the_edited_field() {
        let before = Profile::messenger();
        let after = Profile {
            max_size_mb: 50.0,
            ..before.clone()
        };
        let mut other = Profile {
            max_short_side: 1080,
            codec: VideoCodec::Av1,
            ..Profile::messenger()
        };
        apply_changes(&mut other, &before, &after);
        assert_eq!(other.max_size_mb, 50.0);
        assert_eq!(other.max_short_side, 1080);
        assert_eq!(other.codec, VideoCodec::Av1);
    }
}
