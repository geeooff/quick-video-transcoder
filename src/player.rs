//! Playback of the preview: a [`FrameStream`] for the picture, an [`AudioOutput`] for the sound and its
//! clock, seeks and frame steps.
//!
//! The sound is the master clock: frames are shown when the sound reaches their time. Without sound (or
//! device), or once it has ended, the wall clock takes over. A seek restarts `ffmpeg` from the previous
//! keyframe; seeks asked while one is in progress are coalesced (latest wins), so dragging the timeline
//! stays responsive. If the GPU path fails on a video, the preview falls back to the software filters.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::audio::AudioOutput;
use crate::ffmpeg::audio::audio_args;
use crate::ffmpeg::decoder::{DecodeConfig, Frame, FrameStream, StreamEvent};
use crate::ffmpeg::filters::Engine;
use crate::profile::AudioChoice;

/// The sound to play with a video.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioSource {
    /// Source file.
    pub path: PathBuf,
    /// Audio streams in the file.
    pub tracks: usize,
    /// Which ones to play.
    pub choice: AudioChoice,
}

/// Plays one video. Call [`Player::poll`] every UI frame and display what it returns.
#[derive(Default)]
pub struct Player {
    stream: Option<FrameStream>,
    config: Option<DecodeConfig>,
    audio_source: Option<AudioSource>,
    output: Option<AudioOutput>,
    gain: f32,
    audio_warning: Option<String>,
    fps: f64,
    duration: f64,
    playing: bool,
    /// Instant and media time of the last clock reading (wall clock fallback).
    anchor: Option<(Instant, f64)>,
    position: f64,
    queued: Option<Frame>,
    step_frame: Option<Frame>,
    loading: bool,
    pending_seek: Option<f64>,
    ended: bool,
    error: Option<String>,
    /// The audio queue no longer matches the video position (after a frame step).
    audio_stale: bool,
}

impl Player {
    /// A player with nothing open.
    pub fn new() -> Self {
        Self {
            gain: 1.0,
            ..Self::default()
        }
    }

    /// Opens a video and shows the frame at `start`.
    pub fn open(&mut self, config: DecodeConfig, audio: Option<AudioSource>, start: f64) {
        let output = self.output.take();
        let (gain, warning) = (self.gain, self.audio_warning.take());
        let fps = config.media.fps().max(1.0);
        let duration = config.media.duration;
        *self = Self {
            fps,
            duration,
            config: Some(config),
            audio_source: audio,
            output,
            gain,
            audio_warning: warning,
            ..Self::default()
        };
        if self.audio_source.is_some() && self.output.is_none() {
            match AudioOutput::open() {
                Ok(output) => self.output = Some(output),
                Err(error) => self.audio_warning = Some(format!("no sound: {error:#}")),
            }
        }
        if let Some(output) = &self.output {
            output.set_gain(self.gain);
        }
        self.restart(start);
    }

    /// Closes the video.
    pub fn close(&mut self) {
        let mut output = self.output.take();
        if let Some(output) = &mut output {
            output.stop();
        }
        let gain = self.gain;
        *self = Self {
            output,
            gain,
            ..Self::default()
        };
    }

    /// What is being decoded.
    pub fn config(&self) -> Option<&DecodeConfig> {
        self.config.as_ref()
    }

    /// Changes what is decoded (size, tone mapping…), keeping the position.
    pub fn set_config(&mut self, config: DecodeConfig) {
        if self.config.as_ref() != Some(&config) {
            self.config = Some(config);
            self.restart(self.position);
        }
    }

    /// Changes the sound (another track, mixed), keeping the position.
    pub fn set_audio(&mut self, audio: Option<AudioSource>) {
        if self.audio_source != audio {
            self.audio_source = audio;
            self.restart_audio(self.position);
            if self.playing
                && let Some(output) = &self.output
            {
                output.play();
            }
        }
    }

    /// Current media time.
    pub fn position(&self) -> f64 {
        self.position
    }

    /// `true` while playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// `true` while waiting for the first frame after a seek.
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// The last decoding error.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The video has sound and it can be played.
    pub fn has_audio(&self) -> bool {
        self.audio_source.is_some() && self.output.is_some()
    }

    /// Sets the audio gain (0 = mute, 1 = unchanged).
    pub fn set_gain(&mut self, gain: f32) {
        if self.gain != gain {
            self.gain = gain;
            if let Some(output) = &self.output {
                output.set_gain(gain);
            }
        }
    }

    /// A problem with the sound to show to the user, once.
    pub fn take_audio_warning(&mut self) -> Option<String> {
        self.audio_warning.take()
    }

    fn snap(&self, seconds: f64) -> f64 {
        crate::timecode::snap(seconds, self.fps, self.duration)
    }

    fn restart(&mut self, start: f64) {
        let Some(config) = self.config.clone() else {
            return;
        };
        let start = self.snap(start);
        self.stream = None;
        self.queued = None;
        self.step_frame = None;
        self.position = start;
        self.anchor = None;
        self.ended = false;
        self.error = None;
        self.restart_audio(start);
        match FrameStream::spawn(config, start) {
            Ok(stream) => {
                self.stream = Some(stream);
                self.loading = true;
            }
            Err(error) => {
                self.loading = false;
                self.playing = false;
                self.error = Some(format!("{error:#}"));
            }
        }
    }

    /// Restarts the audio decoder at `start`, silent until the playback clock starts.
    fn restart_audio(&mut self, start: f64) {
        self.audio_stale = false;
        let Some(output) = &mut self.output else {
            return;
        };
        let args = self
            .audio_source
            .as_ref()
            .and_then(|a| audio_args(&a.path, a.tracks, a.choice, start, output.rate()));
        match args {
            Some(args) => {
                if let Err(error) = output.start(args, start) {
                    self.audio_warning = Some(format!("cannot decode the sound: {error:#}"));
                }
            }
            None => output.stop(),
        }
    }

    fn start_clock(&mut self, now: Instant) {
        self.anchor = Some((now, self.position));
        if let Some(output) = &self.output {
            output.play();
        }
    }

    fn audio_clock(&self, now: Instant) -> Option<f64> {
        if self.audio_source.is_none() || self.audio_stale {
            return None;
        }
        self.output.as_ref()?.clock(now)
    }

    /// Seeks to `seconds`.
    pub fn seek(&mut self, seconds: f64) {
        if self.loading {
            self.pending_seek = Some(seconds);
            self.position = self.snap(seconds);
        } else {
            self.restart(seconds);
        }
    }

    /// Starts playing.
    pub fn play(&mut self) {
        if self.ended {
            self.restart(self.position);
        } else if self.audio_stale {
            self.restart_audio(self.position);
        }
        self.playing = true;
        if !self.loading {
            self.start_clock(Instant::now());
        }
    }

    /// Pauses.
    pub fn pause(&mut self) {
        self.playing = false;
        self.anchor = None;
        if let Some(output) = &self.output {
            output.pause();
        }
    }

    /// Plays or pauses.
    pub fn toggle(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Moves `frames` frames forward (decoding on) or backward (seeking).
    pub fn step(&mut self, frames: i64) {
        self.pause();
        if frames == 1 && !self.loading && !self.ended {
            self.audio_stale = true;
            let next = self.queued.take().or_else(|| {
                match self.stream.as_ref()?.next_timeout(Duration::from_secs(5))? {
                    StreamEvent::Frame(frame) => Some(frame),
                    StreamEvent::End { .. } => {
                        self.ended = true;
                        None
                    }
                }
            });
            if let Some(frame) = next {
                self.position = frame.pts;
                self.step_frame = Some(frame);
            }
        } else {
            self.seek(self.position + frames as f64 / self.fps);
        }
    }

    /// Gives a displayed frame back so its buffer can be reused.
    pub fn recycle(&mut self, frame: Frame) {
        if let Some(stream) = &self.stream {
            stream.recycle(frame.data);
        }
    }

    /// Advances playback. Returns the frame to display, if it changed.
    pub fn poll(&mut self) -> Option<Frame> {
        let now = Instant::now();
        if let Some(frame) = self.step_frame.take() {
            return Some(frame);
        }
        let stream = self.stream.as_ref()?;
        if self.loading {
            return match stream.try_next()? {
                StreamEvent::Frame(frame) => {
                    self.loading = false;
                    self.position = frame.pts;
                    if let Some(target) = self.pending_seek.take() {
                        let frame = Some(frame);
                        self.restart(target);
                        return frame;
                    }
                    if self.playing {
                        self.start_clock(now);
                    }
                    Some(frame)
                }
                StreamEvent::End { detail } => {
                    self.loading = false;
                    let engine = stream.config.engine;
                    if !detail.is_empty() && engine != Engine::SOFTWARE {
                        log::warn!(
                            "preview with {} failed, using the software filters: {detail}",
                            engine.label()
                        );
                        if let Some(config) = &mut self.config {
                            config.engine = Engine::SOFTWARE;
                        }
                        let target = self.pending_seek.take().unwrap_or(self.position);
                        self.restart(target);
                    } else {
                        self.playing = false;
                        self.error = Some(if detail.is_empty() {
                            "no frame decoded at this position".to_owned()
                        } else {
                            detail
                        });
                    }
                    None
                }
            };
        }
        if !self.playing {
            return None;
        }
        let target = match self.audio_clock(now) {
            Some(heard) => {
                self.anchor = Some((now, heard));
                heard
            }
            None => {
                let (instant, pts) = *self.anchor.get_or_insert((now, self.position));
                pts + now.duration_since(instant).as_secs_f64()
            }
        };
        let half_frame = 0.5 / self.fps;
        let mut shown: Option<Frame> = None;
        loop {
            let frame = match self.queued.take() {
                Some(frame) => frame,
                None => match self.stream.as_ref()?.try_next() {
                    Some(StreamEvent::Frame(frame)) => frame,
                    Some(StreamEvent::End { .. }) => {
                        self.ended = true;
                        self.pause();
                        break;
                    }
                    None => break,
                },
            };
            if frame.pts <= target + half_frame {
                if let Some(previous) = shown.replace(frame) {
                    self.recycle(previous);
                }
            } else {
                self.queued = Some(frame);
                break;
            }
        }
        if let Some(frame) = &shown {
            self.position = frame.pts;
        }
        shown
    }
}
