//! Audio preview: plays an [`AudioDecoder`] on the default output device (`cpal`) and provides the
//! playback clock (the media time being heard), which the video follows.
//!
//! The device callback is real time: it takes a short lock, does no I/O and never waits on a decoder.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, FromSample, OutputCallbackInfo, SampleFormat, SizedSample, StreamConfig};

use crate::ffmpeg::audio::{AudioDecoder, CHANNELS};

/// Decoded audio kept ahead of the device.
const BUFFER_SECONDS: f64 = 1.0;
/// Without a device callback for this long, the device is considered gone and the clock unusable.
const STALL: Duration = Duration::from_millis(500);

/// Samples waiting for the device and the clock derived from what the device consumed.
struct Queue {
    rate: u32,
    /// Changes with every source or position: older decoders stop feeding the queue.
    generation: u64,
    /// Interleaved stereo samples.
    samples: VecDeque<f32>,
    /// Media time of the first decoded sample.
    start: f64,
    /// Frames handed to the device since `start`.
    consumed: u64,
    playing: bool,
    /// The decoder reached the end of the stream (or failed): no more samples will come.
    finished: bool,
    gain: f32,
    /// Media time heard at an instant, set by the last callback that played samples.
    heard: Option<(Instant, f64)>,
    last_callback: Instant,
    /// The device reported a fatal error.
    failed: bool,
}

impl Queue {
    fn new(rate: u32, now: Instant) -> Self {
        Self {
            rate: rate.max(1),
            generation: 0,
            samples: VecDeque::new(),
            start: 0.0,
            consumed: 0,
            playing: false,
            finished: true,
            gain: 1.0,
            heard: None,
            last_callback: now,
            failed: false,
        }
    }

    fn capacity(&self) -> usize {
        (f64::from(self.rate) * BUFFER_SECONDS) as usize * CHANNELS
    }

    /// Empties the queue for a decoder starting at `start` and returns its generation.
    fn reset(&mut self, start: f64) -> u64 {
        self.generation += 1;
        self.samples.clear();
        self.start = start;
        self.consumed = 0;
        self.playing = false;
        self.finished = false;
        self.heard = None;
        self.generation
    }

    /// Media time of the next sample to hand to the device.
    fn written(&self) -> f64 {
        self.start + self.consumed as f64 / f64::from(self.rate)
    }

    /// Fills `out` (interleaved, `channels` per frame) from the queue, or with silence when paused or
    /// starved. `latency` is the delay until the first frame of `out` is heard.
    fn fill(&mut self, out: &mut [f32], channels: usize, now: Instant, latency: Duration) {
        self.last_callback = now;
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let available = if self.playing {
            (self.samples.len() / CHANNELS).min(frames)
        } else {
            0
        };
        if available > 0 {
            self.heard = Some((now + latency, self.written()));
        }
        for (index, frame) in out.chunks_exact_mut(channels).enumerate() {
            let (left, right) = if index < available {
                let left = self.samples.pop_front().unwrap_or(0.0);
                let right = self.samples.pop_front().unwrap_or(0.0);
                (left * self.gain, right * self.gain)
            } else {
                (0.0, 0.0)
            };
            match frame {
                [mono] => *mono = 0.5 * (left + right),
                [l, r, rest @ ..] => {
                    *l = left;
                    *r = right;
                    rest.fill(0.0);
                }
                [] => {}
            }
        }
        self.consumed += available as u64;
    }

    /// Media time heard at `now`, or `None` when the audio cannot drive playback (paused, finished, no
    /// device). While waiting for the first samples it holds the start time.
    fn clock(&self, now: Instant) -> Option<f64> {
        if !self.playing || self.failed || now.saturating_duration_since(self.last_callback) > STALL
        {
            return None;
        }
        let written = self.written();
        let time = match self.heard {
            Some((instant, pts)) if now >= instant => pts + (now - instant).as_secs_f64(),
            Some((instant, pts)) => pts - (instant - now).as_secs_f64(),
            None => written,
        };
        if self.finished && self.samples.is_empty() && time >= written {
            // Everything decoded has been heard (or there is no audio here).
            return None;
        }
        // Clamping to what was handed to the device stops the clock on an underrun.
        Some(time.clamp(self.start, written))
    }
}

struct Shared {
    queue: Mutex<Queue>,
    /// Signaled when the device consumed samples or the generation changed.
    space: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Decoder sink: queues samples of `generation`, waiting while the queue is full.
    fn push(&self, generation: u64, samples: Option<&[f32]>) -> bool {
        let mut queue = self.lock();
        let Some(samples) = samples else {
            if queue.generation == generation {
                queue.finished = true;
            }
            return false;
        };
        loop {
            if queue.generation != generation {
                return false;
            }
            if queue.samples.len() < queue.capacity() {
                queue.samples.extend(samples);
                return true;
            }
            queue = self
                .space
                .wait_timeout(queue, Duration::from_millis(100))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// The default audio output device, fed by one ffmpeg audio decoder at a time.
pub struct AudioOutput {
    shared: Arc<Shared>,
    decoder: Option<AudioDecoder>,
    _stream: cpal::Stream,
}

impl AudioOutput {
    /// Opens the default output device. It plays silence until [`AudioOutput::start`] and
    /// [`AudioOutput::play`].
    pub fn open() -> Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .context("no audio output device")?;
        let supported = device
            .default_output_config()
            .context("cannot read the audio output configuration")?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::new(config.sample_rate, Instant::now())),
            space: Condvar::new(),
        });
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, &shared),
            SampleFormat::I16 => build::<i16>(&device, &config, &shared),
            SampleFormat::I32 => build::<i32>(&device, &config, &shared),
            SampleFormat::U16 => build::<u16>(&device, &config, &shared),
            other => bail!("unsupported audio sample format {other}"),
        }?;
        stream.play().context("cannot start the audio output")?;
        log::info!(
            "audio output: {} Hz, {} channels, {format}",
            config.sample_rate,
            config.channels
        );
        Ok(Self {
            shared,
            decoder: None,
            _stream: stream,
        })
    }

    /// The device's sample rate.
    pub fn rate(&self) -> u32 {
        self.shared.lock().rate
    }

    /// Starts decoding with `args` (built for [`AudioOutput::rate`]) from media time `start`. Silent until
    /// [`AudioOutput::play`].
    pub fn start(&mut self, args: Vec<String>, start: f64) -> Result<()> {
        let generation = self.shared.lock().reset(start);
        self.shared.space.notify_all();
        // The previous decoder sees the new generation and stops.
        self.decoder = None;
        let shared = Arc::clone(&self.shared);
        match AudioDecoder::spawn(args, move |samples| shared.push(generation, samples)) {
            Ok(decoder) => {
                self.decoder = Some(decoder);
                Ok(())
            }
            Err(error) => {
                self.shared.lock().finished = true;
                Err(error)
            }
        }
    }

    /// Stops decoding and plays silence.
    pub fn stop(&mut self) {
        {
            let mut queue = self.shared.lock();
            queue.reset(0.0);
            queue.finished = true;
        }
        self.shared.space.notify_all();
        self.decoder = None;
    }

    /// Starts handing samples to the device.
    pub fn play(&self) {
        self.shared.lock().playing = true;
    }

    /// Plays silence, keeping the queue.
    pub fn pause(&self) {
        let mut queue = self.shared.lock();
        queue.playing = false;
        queue.heard = None;
    }

    /// Linear amplitude factor (0 = silent, 1 = unchanged).
    pub fn set_gain(&self, gain: f32) {
        self.shared.lock().gain = gain;
    }

    /// Media time heard now, or `None` when the audio cannot drive playback.
    pub fn clock(&self, now: Instant) -> Option<f64> {
        self.shared.lock().clock(now)
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Whether a stream error means the output stopped for good. The other errors are transient: cpal keeps
/// the stream running, and an output that stops calling back is caught by [`STALL`].
fn stream_lost(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::DeviceNotAvailable | ErrorKind::HostUnavailable | ErrorKind::StreamInvalidated
    )
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let data_shared = Arc::clone(shared);
    let error_shared = Arc::clone(shared);
    let mut mix = Vec::new();
    let stream = device.build_output_stream(
        *config,
        move |out: &mut [T], info: &OutputCallbackInfo| {
            let timestamp = info.timestamp();
            let latency = timestamp
                .playback
                .saturating_duration_since(timestamp.callback);
            mix.resize(out.len(), 0.0);
            data_shared
                .lock()
                .fill(&mut mix, channels, Instant::now(), latency);
            data_shared.space.notify_all();
            for (sample, value) in out.iter_mut().zip(&mix) {
                *sample = T::from_sample(*value);
            }
        },
        move |error: cpal::Error| {
            if stream_lost(error.kind()) {
                log::warn!("audio output lost: {error}");
                error_shared.lock().failed = true;
            } else {
                log::debug!("audio output: {error}");
            }
        },
        None,
    )?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn playing_queue(now: Instant, frames: usize) -> Queue {
        let mut queue = Queue::new(1000, now);
        queue.reset(10.0);
        queue
            .samples
            .extend((0..frames * CHANNELS).map(|i| i as f32));
        queue.playing = true;
        queue
    }

    #[test]
    fn follows_what_the_device_plays() {
        let t0 = Instant::now();
        let mut queue = playing_queue(t0, 500);
        assert_eq!(queue.clock(t0), Some(10.0));
        let mut out = vec![0.0; 100 * 2];
        queue.fill(&mut out, 2, t0, 20 * MS);
        assert_eq!(&out[..4], &[0.0, 1.0, 2.0, 3.0]);
        assert_eq!(queue.clock(t0 + 10 * MS), Some(10.0));
        let heard = queue.clock(t0 + 70 * MS).unwrap();
        assert!((heard - 10.05).abs() < 1e-9, "{heard}");
        // Never beyond what the device received (underrun).
        assert_eq!(queue.clock(t0 + 400 * MS), Some(10.1));
    }

    #[test]
    fn plays_silence_while_paused() {
        let t0 = Instant::now();
        let mut queue = playing_queue(t0, 500);
        queue.playing = false;
        let mut out = vec![1.0; 64];
        queue.fill(&mut out, 2, t0, MS);
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(queue.clock(t0), None);
    }

    #[test]
    fn hands_over_to_the_wall_clock_when_the_audio_ends() {
        let t0 = Instant::now();
        let mut queue = playing_queue(t0, 100);
        queue.finished = true;
        let mut out = vec![0.0; 100 * 2];
        queue.fill(&mut out, 2, t0, Duration::ZERO);
        assert!(queue.clock(t0 + 50 * MS).is_some());
        assert_eq!(queue.clock(t0 + 100 * MS), None);
    }

    #[test]
    fn only_a_lost_stream_disables_the_audio_clock() {
        assert!(stream_lost(ErrorKind::DeviceNotAvailable));
        assert!(!stream_lost(ErrorKind::Xrun));
        let t0 = Instant::now();
        let mut queue = playing_queue(t0, 100);
        assert_eq!(queue.clock(t0 + STALL + MS), None);
        queue.failed = true;
        assert_eq!(queue.clock(t0), None);
    }

    #[test]
    fn maps_stereo_to_the_device_channels() {
        let t0 = Instant::now();
        let mut queue = playing_queue(t0, 4);
        queue.gain = 0.5;
        let mut mono = vec![0.0; 1];
        queue.fill(&mut mono, 1, t0, MS);
        assert_eq!(mono, [0.25]);
        let mut surround = vec![9.0; 6];
        queue.fill(&mut surround, 6, t0, MS);
        assert_eq!(surround, [1.0, 1.5, 0.0, 0.0, 0.0, 0.0]);
    }
}
