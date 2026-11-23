//! Right panel: the settings of the selected videos (profile, limits, sound, quality, kept part), the
//! estimate of the focused one, and the output and encoder preferences. Also the profiles window.

use eframe::egui::{self, RichText};

use super::TranscoderApp;
use super::queue_panel::{GOOD, WARN};
use crate::estimate::{Verdict, quality_word};
use crate::ffmpeg::encoders::{EncoderBackend, VideoCodec};
use crate::ffmpeg::probe::game_track;
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
        codecs,
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
                            ui.heading(crate::tr!(
                                "settings-heading-selected",
                                count = selected.len()
                            ));
                            ui.label(
                                RichText::new(crate::tr!("settings-change-applies-to-all")).weak(),
                            );
                        } else {
                            ui.add(
                                egui::Label::new(RichText::new(job.name()).heading()).truncate(),
                            )
                            .on_hover_text(job.path.display().to_string());
                        }
                    }
                    None => {
                        ui.heading(crate::tr!("settings-no-video"));
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

        section(ui, &crate::tr!("settings-section-profile"));
        let mut chosen: Option<Profile> = None;
        ui.horizontal(|ui| {
            let label = if modified {
                crate::tr!("settings-profile-changed", name = job.profile_name.as_str())
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
                    .button(crate::tr!("settings-profile-reset"))
                    .on_hover_text(crate::tr!("settings-profile-reset-tooltip"))
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
        // Codec lists offered, preferred codec first; the profile's own list comes first when it differs.
        let mut lists: Vec<Vec<VideoCodec>> = vec![
            vec![VideoCodec::Av1, VideoCodec::H265, VideoCodec::H264],
            vec![VideoCodec::H265, VideoCodec::H264],
            vec![VideoCodec::H264],
            vec![VideoCodec::H265],
            vec![VideoCodec::Av1],
        ];
        if let Some(origin) = &origin
            && !lists.contains(&origin.codecs)
        {
            lists.insert(0, origin.codecs.clone());
        }
        let capabilities = self.queue.capabilities.clone();
        let backend = self.queue.settings.backend;
        let resolved = |codecs: &[VideoCodec]| {
            capabilities.choose(codecs, backend).map_or_else(
                || crate::tr!("settings-codec-no-encoder"),
                |spec| format!("{} ({})", spec.codec.label(), spec.backend.label()),
            )
        };
        let list_label = |codecs: &[VideoCodec]| {
            if codecs.len() == 1 {
                crate::tr!("settings-codec-only", codec = codecs[0].label())
            } else {
                codecs
                    .iter()
                    .map(|c| c.label())
                    .collect::<Vec<_>>()
                    .join(" > ")
            }
        };
        egui::Grid::new("limits")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label(crate::tr!("settings-size-limit"));
                ui.add(
                    egui::DragValue::new(&mut after.max_size_mb)
                        .range(0.5..=4000.0)
                        .speed(0.5)
                        .max_decimals(1)
                        // Shown and typed in the user's format ("25,0 Mo"): either separator is read.
                        .custom_formatter(|mb, _| crate::tr!("settings-size-value", mb = mb))
                        .custom_parser(parse_localized_number),
                )
                .on_hover_text(crate::tr!("settings-size-limit-tooltip"));
                ui.end_row();

                ui.label(crate::tr!("settings-resolution"));
                egui::ComboBox::from_id_salt("short side")
                    .selected_text(crate::tr!(
                        "settings-resolution-up-to",
                        short = after.max_short_side.to_string()
                    ))
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

                ui.label(crate::tr!("settings-frame-rate"));
                egui::ComboBox::from_id_salt("fps")
                    .selected_text(crate::tr!("settings-frame-rate-up-to", fps = after.max_fps))
                    .show_ui(ui, |ui| {
                        for fps in FRAME_RATES {
                            ui.selectable_value(
                                &mut after.max_fps,
                                fps,
                                crate::tr!("settings-fps-value", fps = fps),
                            );
                        }
                    });
                ui.end_row();

                ui.label(crate::tr!("settings-codec"));
                egui::ComboBox::from_id_salt("codec")
                    .selected_text(format!(
                        "{} ➡ {}",
                        list_label(&after.codecs),
                        resolved(&after.codecs)
                    ))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for list in &lists {
                            let text = format!("{}  ➡ {}", list_label(list), resolved(list));
                            ui.selectable_value(&mut after.codecs, list.clone(), text);
                        }
                    })
                    .response
                    .on_hover_text(crate::tr!("settings-codec-tooltip"));
                ui.end_row();

                ui.label(crate::tr!("settings-hdr"));
                ui.add_enabled(
                    after.codecs.iter().any(|c| c.supports_hdr()),
                    egui::Checkbox::new(&mut after.sdr_only, crate::tr!("settings-hdr-convert")),
                )
                .on_hover_text(crate::tr!("settings-hdr-convert-tooltip"))
                .on_disabled_hover_text(crate::tr!("settings-hdr-convert-disabled-tooltip"));
                ui.end_row();

                let hdr_source = media.as_ref().is_some_and(|m| m.dynamic.is_hdr());
                ui.label(crate::tr!("settings-tone-mapping"));
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
                .on_disabled_hover_text(crate::tr!("settings-tone-mapping-disabled-tooltip"));
                ui.end_row();

                ui.label(crate::tr!("settings-sound"));
                egui::ComboBox::from_id_salt("audio")
                    .selected_text(audio_label(after.audio, media.as_ref()))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut after.audio,
                            AudioChoice::Auto,
                            audio_label(AudioChoice::Auto, media.as_ref()),
                        )
                        .on_hover_text(crate::tr!("settings-sound-auto-tooltip"));
                        if let Some(media) = &media {
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
                            crate::tr!("settings-sound-mix-all"),
                        );
                        ui.selectable_value(
                            &mut after.audio,
                            AudioChoice::None,
                            crate::tr!("settings-sound-none"),
                        );
                    });
                ui.end_row();

                ui.label(crate::tr!("settings-sound-bitrate"));
                egui::ComboBox::from_id_salt("audio rate")
                    .selected_text(crate::tr!(
                        "settings-kbps-value",
                        kbps = after.audio_bitrate_kbps
                    ))
                    .show_ui(ui, |ui| {
                        for rate in AUDIO_RATES {
                            ui.selectable_value(
                                &mut after.audio_bitrate_kbps,
                                rate,
                                crate::tr!("settings-kbps-value", kbps = rate),
                            );
                        }
                    });
                ui.end_row();
            });
        egui::CollapsingHeader::new(crate::tr!("settings-quality-bounds"))
            .id_salt("quality bounds")
            .show(ui, |ui| {
                egui::Grid::new("quality")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(crate::tr!("settings-best-worth-it"))
                            .on_hover_text(crate::tr!("settings-best-worth-it-tooltip"));
                        ui.add(
                            egui::Slider::new(&mut after.quality_ceiling, 14..=30)
                                .clamping(egui::SliderClamping::Edits)
                                .custom_formatter(|v, _| {
                                    crate::tr!(
                                        "settings-quality-value",
                                        qp = v,
                                        word = quality_word(v)
                                    )
                                }),
                        );
                        ui.end_row();
                        ui.label(crate::tr!("settings-worst-accepted"))
                            .on_hover_text(crate::tr!("settings-worst-accepted-tooltip"));
                        ui.add(
                            egui::Slider::new(&mut after.quality_floor, 24..=42)
                                .clamping(egui::SliderClamping::Edits)
                                .custom_formatter(|v, _| {
                                    crate::tr!(
                                        "settings-quality-value",
                                        qp = v,
                                        word = quality_word(v)
                                    )
                                }),
                        );
                        ui.end_row();
                        ui.label(crate::tr!("settings-lowest-resolution"));
                        egui::ComboBox::from_id_salt("min short side")
                            .selected_text(format!("{}p", after.min_short_side))
                            .show_ui(ui, |ui| {
                                for short in SHORT_SIDES
                                    .into_iter()
                                    .filter(|&s| s <= after.max_short_side)
                                {
                                    ui.selectable_value(
                                        &mut after.min_short_side,
                                        short,
                                        format!("{short}p"),
                                    );
                                }
                            });
                        ui.end_row();
                        ui.label(crate::tr!("settings-lowest-frame-rate"));
                        egui::ComboBox::from_id_salt("min fps")
                            .selected_text(crate::tr!("settings-fps-value", fps = after.min_fps))
                            .show_ui(ui, |ui| {
                                for fps in FRAME_RATES.into_iter().filter(|&f| f <= after.max_fps) {
                                    ui.selectable_value(
                                        &mut after.min_fps,
                                        fps,
                                        crate::tr!("settings-fps-value", fps = fps),
                                    );
                                }
                            });
                        ui.end_row();
                        ui.label(crate::tr!("settings-file-name-suffix"));
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
                    .hint_text(crate::tr!("settings-profile-name-hint"))
                    .desired_width(150.0),
            );
            let name = self.new_profile_name.trim().to_owned();
            let exists = self.profiles.find(&name).is_some();
            let label = if exists {
                crate::tr!("settings-profile-update")
            } else {
                crate::tr!("settings-profile-save")
            };
            if ui
                .add_enabled(!name.is_empty(), egui::Button::new(label))
                .on_hover_text(crate::tr!("settings-profile-save-tooltip"))
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
        section(ui, &crate::tr!("settings-section-kept-part"));
        if selected > 1 {
            ui.label(RichText::new(crate::tr!("settings-kept-part-multi-note")).weak());
        }
        let trim = job.trim;
        let mut start_text = ui
            .data(|d| d.get_temp::<String>(egui::Id::unique(("trim start", focus))))
            .unwrap_or_default();
        let mut end_text = ui
            .data(|d| d.get_temp::<String>(egui::Id::unique(("trim end", focus))))
            .unwrap_or_default();
        let mut start_edit = TimeEdit::Unchanged;
        let mut end_edit = TimeEdit::Unchanged;
        egui::Grid::new("trim")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label(crate::tr!("settings-trim-start"));
                ui.push_id(("start", focus), |ui| {
                    start_edit = time_edit(ui, &mut start_text, trim.start)
                });
                ui.end_row();
                ui.label(crate::tr!("settings-trim-end"));
                ui.push_id(("end", focus), |ui| {
                    end_edit = time_edit(ui, &mut end_text, trim.end)
                });
                ui.end_row();
                ui.label(crate::tr!("settings-trim-duration"));
                ui.monospace(timecode::format_precise(trim.duration()));
                ui.end_row();
            });
        ui.data_mut(|d| {
            d.insert_temp(egui::Id::unique(("trim start", focus)), start_text);
            d.insert_temp(egui::Id::unique(("trim end", focus)), end_text);
        });
        let reset = ui
            .add_enabled(
                trim.start > 0.0 || trim.end < duration,
                egui::Button::new(crate::tr!("settings-keep-everything")),
            )
            .clicked();
        if self.focus == Some(focus) {
            match start_edit {
                TimeEdit::Set(seconds) => self.set_trim_start(seconds),
                TimeEdit::Invalid(text) => {
                    self.error(crate::tr!("settings-not-a-time", text = text));
                }
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
                TimeEdit::Invalid(text) => {
                    self.error(crate::tr!("settings-not-a-time", text = text));
                }
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
        section(ui, &crate::tr!("settings-section-estimate"));
        let key = self.queue.key(job);
        let duration = job.trim.duration();
        let audio = plan::audio_kbps(media, &job.profile);
        let budget = plan::video_budget(job.profile.max_bytes(), duration, audio);
        match job.current_estimate(key.as_ref()) {
            Some(Ok(estimate)) => {
                let (text, color) = match estimate.verdict {
                    Verdict::BestQuality => (crate::tr!("settings-verdict-best-quality"), GOOD),
                    Verdict::Fitted => (
                        crate::tr!("settings-verdict-fitted"),
                        ui.visuals().text_color(),
                    ),
                    Verdict::BelowFloor => (crate::tr!("settings-verdict-below-floor"), WARN),
                };
                ui.label(RichText::new(text).color(color));
                egui::Grid::new("estimate")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(crate::tr!("settings-estimate-output"));
                        ui.label(crate::tr!(
                            "settings-estimate-output-value",
                            size = estimate.target.size.to_string(),
                            fps = estimate.target.rate.label(),
                            target = estimate.target.label()
                        ));
                        ui.end_row();
                        ui.label(crate::tr!("settings-estimate-video-bitrate"));
                        ui.label(crate::tr!(
                            "settings-kbps-value",
                            kbps = (estimate.video_bps / 1000.0).round()
                        ));
                        ui.end_row();
                        ui.label(crate::tr!("settings-estimate-quality"));
                        ui.label(crate::tr!(
                            "settings-estimate-quality-value",
                            word = quality_word(estimate.quality),
                            qp = estimate.quality
                        ));
                        ui.end_row();
                        ui.label(crate::tr!("settings-estimate-size"));
                        let predicted_mb: f64 = estimate.predicted_bytes as f64 / 1e6;
                        ui.label(crate::tr!(
                            "settings-estimate-size-value",
                            predicted = predicted_mb,
                            limit = job.profile.max_size_mb
                        ));
                        ui.end_row();
                        ui.label(crate::tr!("settings-estimate-measured"));
                        ui.label(crate::tr!(
                            "settings-estimate-measured-value",
                            count = estimate.measurements
                        ));
                        ui.end_row();
                    });
            }
            Some(Err(error)) => {
                ui.label(RichText::new(error).color(ui.visuals().error_fg_color));
            }
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(crate::tr!("settings-estimating"));
                });
                if let Some(bps) = budget {
                    ui.label(
                        RichText::new(crate::tr!(
                            "settings-estimate-budget",
                            kbps = (bps / 1000.0).round()
                        ))
                        .weak(),
                    );
                }
            }
        }
        if let Some(engine) = job.engine {
            ui.label(
                RichText::new(crate::tr!(
                    "settings-estimate-filters",
                    engine = engine.label()
                ))
                .weak()
                .small(),
            );
        }
    }

    fn output_section(&mut self, ui: &mut egui::Ui) {
        section(ui, &crate::tr!("settings-section-output"));
        let mut settings = self.queue.settings.clone();
        ui.horizontal(|ui| {
            let folder = settings.output_dir.as_ref().map_or_else(
                || crate::tr!("settings-output-next-to-video"),
                |d| d.display().to_string(),
            );
            ui.add(egui::Label::new(folder).truncate());
        });
        ui.horizontal(|ui| {
            if ui
                .button(crate::tr!("settings-output-choose-folder"))
                .clicked()
            {
                let mut dialog = rfd::FileDialog::new()
                    .set_title(crate::tr!("settings-output-folder-dialog-title"));
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
                    egui::Button::new(crate::tr!("settings-output-use-next-to-video")),
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
                ui.label(crate::tr!("settings-encoder"));
                let label = settings.backend.map_or_else(
                    || crate::tr!("settings-encoder-automatic"),
                    |backend| backend.label().to_owned(),
                );
                egui::ComboBox::from_id_salt("backend")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut settings.backend,
                            None,
                            crate::tr!("settings-encoder-automatic"),
                        );
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
                ui.label(crate::tr!("settings-parallel-encodes"));
                let measured = self.prefs.measured_parallel.as_ref().map(|(_, n)| *n);
                let slider = ui
                    .add(egui::Slider::new(&mut settings.parallel_encodes, 1..=6))
                    .on_hover_text(match measured {
                        Some(n) => crate::tr!("settings-parallel-measured-tooltip", count = n),
                        None => crate::tr!("settings-parallel-not-measured-tooltip"),
                    });
                if slider.changed() {
                    self.prefs.parallel_manual = true;
                }
                ui.end_row();
                ui.label("");
                ui.horizontal(|ui| {
                    let measuring = self.measuring.is_some();
                    let text = match (measuring, measured) {
                        (true, _) => crate::tr!("settings-parallel-measuring"),
                        (false, Some(n)) => crate::tr!("settings-parallel-measured", count = n),
                        (false, None) => crate::tr!("settings-parallel-not-measured"),
                    };
                    ui.label(egui::RichText::new(text).weak().small());
                    let back = self.prefs.parallel_manual
                        && measured.is_some_and(|n| n != settings.parallel_encodes);
                    if back
                        && ui
                            .small_button(crate::tr!("settings-parallel-use-measured"))
                            .clicked()
                    {
                        self.prefs.parallel_manual = false;
                        settings.parallel_encodes = measured.unwrap_or(settings.parallel_encodes);
                    }
                    if !measuring
                        && ui
                            .small_button(crate::tr!("settings-parallel-measure-again"))
                            .on_hover_text(crate::tr!("settings-parallel-measure-again-tooltip"))
                            .clicked()
                    {
                        self.prefs.parallel_manual = false;
                        self.start_parallel_measurement(true);
                    }
                });
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
        egui::Window::new(crate::tr!("profiles-title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(crate::tr!("profiles-intro"));
                ui.add_space(6.0);
                let count = self.profiles.profiles.len();
                egui::Grid::new("profiles list")
                    .striped(true)
                    .num_columns(4)
                    .show(ui, |ui| {
                        for (index, profile) in self.profiles.profiles.iter_mut().enumerate() {
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut profile.name).desired_width(140.0),
                            );
                            if response.lost_focus() && response.changed() {
                                changed = true;
                            }
                            changed |= response.changed();
                            ui.label(
                                RichText::new(if profile.sdr_only {
                                    crate::tr!(
                                        "profiles-summary-sdr",
                                        mb = profile.max_size_mb,
                                        short = profile.max_short_side.to_string(),
                                        fps = profile.max_fps,
                                        codecs = profile.codecs_label()
                                    )
                                } else {
                                    crate::tr!(
                                        "profiles-summary",
                                        mb = profile.max_size_mb,
                                        short = profile.max_short_side.to_string(),
                                        fps = profile.max_fps,
                                        codecs = profile.codecs_label()
                                    )
                                })
                                .weak(),
                            );
                            if ui
                                .add_enabled(index > 0, egui::Button::new("⏶"))
                                .on_hover_text(crate::tr!("profiles-move-up-tooltip"))
                                .clicked()
                            {
                                raise = Some(index);
                            }
                            if ui
                                .add_enabled(count > 1, egui::Button::new("✖"))
                                .on_hover_text(crate::tr!("profiles-delete-tooltip"))
                                .clicked()
                            {
                                remove = Some(index);
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                // Built-in profiles missing from the file (deleted, or added in a later version).
                let missing: Vec<Profile> = Profile::built_ins()
                    .into_iter()
                    .filter(|p| self.profiles.find(&p.name).is_none())
                    .collect();
                ui.add_enabled_ui(!missing.is_empty(), |ui| {
                    ui.menu_button(crate::tr!("profiles-add-built-in"), |ui| {
                        for profile in missing {
                            if ui.button(&profile.name).clicked() {
                                self.profiles.profiles.push(profile);
                                changed = true;
                                ui.close();
                            }
                        }
                    });
                });
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
    let audio = media.map_or(&[][..], |m| &m.audio[..]);
    match choice {
        AudioChoice::Auto => match game_track(audio).and_then(|i| audio.get(i)) {
            Some(track) => match &track.title {
                Some(title) => crate::tr!("settings-sound-auto-titled", title = title.as_str()),
                None => crate::tr!(
                    "settings-sound-auto-numbered",
                    number = track.index.saturating_add(1)
                ),
            },
            None => crate::tr!("settings-sound-auto-default"),
        },
        AudioChoice::Track(index) => audio.get(index).map_or_else(
            || crate::tr!("settings-sound-track", number = index.saturating_add(1)),
            |t| t.label(),
        ),
        AudioChoice::MixAll => crate::tr!("settings-sound-mix-all"),
        AudioChoice::None => crate::tr!("settings-sound-none"),
    }
}

/// A number typed in a number box, in any locale's format: the digits, and a comma or a point as the
/// decimal separator ("25,5 Mo", "25.5", "1 000"). Grouping spaces and units are ignored.
fn parse_localized_number(text: &str) -> Option<f64> {
    let kept: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '-'))
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    kept.parse().ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn number_boxes_read_either_decimal_separator() {
        assert_eq!(super::parse_localized_number("25,5 Mo"), Some(25.5));
        assert_eq!(super::parse_localized_number("25.5 MB"), Some(25.5));
        assert_eq!(super::parse_localized_number("1\u{202f}000"), Some(1000.0));
        assert_eq!(super::parse_localized_number("MB"), None);
    }

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
            codecs: vec![VideoCodec::Av1],
            ..Profile::messenger()
        };
        apply_changes(&mut other, &before, &after);
        assert_eq!(other.max_size_mb, 50.0);
        assert_eq!(other.max_short_side, 1080);
        assert_eq!(other.codecs, [VideoCodec::Av1]);
    }
}
