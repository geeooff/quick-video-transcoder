//! Output geometry and the byte budget: pure arithmetic, no FFmpeg.
//!
//! - The frame rate is the source's nominal rate divided by a whole number ([`FrameRate::output`]): 60
//!   becomes 30 and 59.94 becomes 29.97, so every output frame is a source frame and motion keeps an even
//!   pace.
//! - The picture is never upscaled; its short side is capped (720 for 1280x720 and for a 720x1280 portrait
//!   clip) and both sides stay even, as 4:2:0 encoders require.
//! - The candidates the size fitting tries ([`ladder`]) go from the profile's best down to its minimums,
//!   by decreasing pixel rate.

use std::fmt;

use crate::ffmpeg::probe::MediaInfo;
use crate::profile::{AudioChoice, Profile};

/// A frame rate as FFmpeg writes it: a fraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameRate {
    /// Numerator.
    pub num: u32,
    /// Denominator, never zero.
    pub den: u32,
}

impl FrameRate {
    /// `num / den`, with a denominator of at least 1.
    pub const fn new(num: u32, den: u32) -> Self {
        Self {
            num,
            den: if den == 0 { 1 } else { den },
        }
    }

    /// Parses `60000/1001` or `30`.
    pub fn parse(text: &str) -> Option<Self> {
        let (num, den) = text.trim().split_once('/').unwrap_or((text.trim(), "1"));
        let (num, den): (u32, u32) = (num.parse().ok()?, den.parse().ok()?);
        (num > 0 && den > 0).then(|| Self::new(num, den))
    }

    /// Frames per second.
    pub fn as_f64(self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }

    /// The rate divided by `divisor`.
    pub fn divided(self, divisor: u32) -> Self {
        let den = u64::from(self.den) * u64::from(divisor.max(1));
        let gcd = gcd(u64::from(self.num), den);
        Self::new(
            (u64::from(self.num) / gcd) as u32,
            (den / gcd).min(u64::from(u32::MAX)) as u32,
        )
    }

    /// The highest rate `self / k` (k = 1, 2, 3…) not above `max` (with a 1 % tolerance, so 60.02 fps
    /// counts as 60).
    pub fn output(self, max: f64) -> Self {
        (1..=16)
            .map(|k| self.divided(k))
            .find(|rate| rate.as_f64() <= max * 1.01)
            .unwrap_or_else(|| self.divided(16))
    }

    /// Every rate `self / k` between `min` and `max`, highest first (at least one: [`FrameRate::output`]).
    pub fn candidates(self, min: f64, max: f64) -> Vec<Self> {
        let best = self.output(max);
        let mut rates = vec![best];
        rates.extend(
            (2..=16)
                .map(|k| best.divided(k))
                .take_while(|rate| rate.as_f64() >= min * 0.99),
        );
        rates
    }

    /// The `-r` / `fps=` argument (`30000/1001`).
    pub fn arg(self) -> String {
        if self.den == 1 {
            self.num.to_string()
        } else {
            format!("{}/{}", self.num, self.den)
        }
    }

    /// Display value: `60`, `29.97`.
    pub fn label(self) -> String {
        let fps = self.as_f64();
        if (fps - fps.round()).abs() < 0.005 {
            format!("{}", fps.round())
        } else {
            format!("{fps:.2}")
        }
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

/// A picture size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Size {
    /// The short side.
    pub fn short_side(self) -> u32 {
        self.width.min(self.height)
    }

    /// Pixels per frame.
    pub fn pixels(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// The name people use: `720p` for 1280x720, `1080p` for 1920x1080 or 1080x1920.
    pub fn label(self) -> String {
        format!("{}p", self.short_side())
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Rounds to the nearest even number, at least 2.
fn even(value: f64) -> u32 {
    ((value / 2.0).round() as u32 * 2).max(2)
}

/// The size of a `width` x `height` picture whose short side is at most `max_short`, keeping its aspect
/// ratio, never upscaled, both sides even.
pub fn fit(width: u32, height: u32, max_short: u32) -> Size {
    let short = width.min(height).max(1);
    let scale = (f64::from(max_short) / f64::from(short)).min(1.0);
    Size {
        width: even(f64::from(width) * scale),
        height: even(f64::from(height) * scale),
    }
}

/// What the encoder produces: a picture size and a frame rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Target {
    /// Output picture size.
    pub size: Size,
    /// Output frame rate.
    pub rate: FrameRate,
}

impl Target {
    /// Pixels per second, the measure of how much the encoder has to describe.
    pub fn pixel_rate(self) -> f64 {
        self.size.pixels() as f64 * self.rate.as_f64()
    }

    /// `720p30`, `1080p59.94`.
    pub fn label(self) -> String {
        format!("{}{}", self.size.label(), self.rate.label())
    }
}

/// Usual short sides tried between the profile's maximum and minimum.
const STEPS: [u32; 6] = [1440, 1080, 720, 540, 480, 360];

/// The targets the size fitting may use for `media`, best first: the profile's maximums, then lower frame
/// rates and smaller pictures down to the profile's minimums, by decreasing pixel rate.
pub fn ladder(width: u32, height: u32, rate: FrameRate, profile: &Profile) -> Vec<Target> {
    let source_short = width.min(height);
    let top = profile.max_short_side.min(source_short);
    let mut shorts = vec![top];
    shorts.extend(
        STEPS
            .into_iter()
            .filter(|&s| s < top && s >= profile.min_short_side),
    );
    let rates = rate.candidates(profile.min_fps, profile.max_fps);
    let mut targets: Vec<Target> = shorts
        .iter()
        .flat_map(|&short| {
            let size = fit(width, height, short);
            rates.iter().map(move |&rate| Target { size, rate })
        })
        .collect();
    targets.sort_by(|a, b| b.pixel_rate().total_cmp(&a.pixel_rate()));
    targets.dedup();
    targets
}

/// The best target of a profile for a source: [`ladder`]'s first.
pub fn best_target(media: &MediaInfo, profile: &Profile) -> Target {
    ladder(media.width, media.height, media.frame_rate, profile)[0]
}

/// MP4 headers and index, and the share of the file that is container rather than streams.
const CONTAINER_BYTES: f64 = 64_000.0;
const CONTAINER_SHARE: f64 = 0.01;
/// Kept free under the limit: the encoder's rate control lands within a few percent of its target.
const SAFETY: f64 = 0.04;

/// Bytes of AAC audio for `duration` seconds at `kbps`, with its framing.
pub fn audio_bytes(kbps: u32, duration: f64) -> f64 {
    f64::from(kbps) * 1000.0 / 8.0 * duration * 1.02
}

/// The video bitrate (bit/s) that fills `max_bytes` for a clip of `duration` seconds, once the audio and
/// the container are paid for. `None` when the audio alone does not fit.
pub fn video_budget(max_bytes: u64, duration: f64, audio_kbps: Option<u32>) -> Option<f64> {
    let duration = duration.max(0.1);
    let usable = max_bytes as f64 * (1.0 - SAFETY - CONTAINER_SHARE) - CONTAINER_BYTES;
    let audio = audio_kbps.map_or(0.0, |kbps| audio_bytes(kbps, duration));
    let video = usable - audio;
    (video > 0.0).then(|| video * 8.0 / duration)
}

/// The audio bitrate an output will have, `None` without sound.
pub fn audio_kbps(media: &MediaInfo, profile: &Profile) -> Option<u32> {
    let has_audio = match profile.audio {
        AudioChoice::None => false,
        AudioChoice::Track(index) => index < media.audio.len(),
        AudioChoice::First | AudioChoice::MixAll => !media.audio.is_empty(),
    };
    has_audio.then_some(profile.audio_bitrate_kbps)
}

/// The predicted size of an output: video at `video_bps` plus audio and container.
pub fn predicted_bytes(video_bps: f64, duration: f64, audio_kbps: Option<u32>) -> u64 {
    let streams = video_bps / 8.0 * duration + audio_kbps.map_or(0.0, |k| audio_bytes(k, duration));
    (streams * (1.0 + CONTAINER_SHARE) + CONTAINER_BYTES) as u64
}

/// Seconds between keyframes. Longer saves bits (a keyframe costs several predicted frames) at the price of
/// coarser seeking, which a chat player barely offers anyway.
pub const GOP_SECONDS: f64 = 4.0;

/// The keyframe interval in frames ([`GOP_SECONDS`]). The size estimate encodes samples of exactly one
/// interval, so they carry the same share of keyframes as the whole encode.
pub fn gop(rate: FrameRate) -> u32 {
    (rate.as_f64() * GOP_SECONDS).round().max(1.0) as u32
}

/// Sample length in seconds for the size estimate: one keyframe interval.
pub fn sample_seconds(rate: FrameRate) -> f64 {
    f64::from(gop(rate)) / rate.as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NTSC_60: FrameRate = FrameRate::new(60000, 1001);

    #[test]
    fn frame_rates_divide_evenly() {
        assert_eq!(FrameRate::new(60, 1).output(30.0), FrameRate::new(30, 1));
        assert_eq!(NTSC_60.output(30.0), FrameRate::new(30000, 1001));
        assert_eq!(NTSC_60.output(30.0).label(), "29.97");
        assert_eq!(FrameRate::new(24, 1).output(30.0), FrameRate::new(24, 1));
        assert_eq!(FrameRate::new(120, 1).output(50.0), FrameRate::new(40, 1));
        assert_eq!(
            FrameRate::new(6002, 100).output(60.0),
            FrameRate::new(3001, 50)
        );
        assert_eq!(FrameRate::new(60, 1).output(60.0).arg(), "60");
        assert_eq!(NTSC_60.divided(2).arg(), "30000/1001");
        assert_eq!(FrameRate::parse("60000/1001"), Some(NTSC_60));
        assert_eq!(FrameRate::parse("0/0"), None);
    }

    #[test]
    fn candidates_go_down_to_the_minimum() {
        let rates = FrameRate::new(60, 1).candidates(24.0, 60.0);
        assert_eq!(rates, [FrameRate::new(60, 1), FrameRate::new(30, 1)]);
        let rates = FrameRate::new(60, 1).candidates(10.0, 30.0);
        assert_eq!(
            rates,
            [
                FrameRate::new(30, 1),
                FrameRate::new(15, 1),
                FrameRate::new(10, 1)
            ]
        );
    }

    #[test]
    fn fits_the_short_side_without_upscaling() {
        assert_eq!(
            fit(3840, 2160, 720),
            Size {
                width: 1280,
                height: 720
            }
        );
        assert_eq!(
            fit(1080, 1920, 720),
            Size {
                width: 720,
                height: 1280
            }
        );
        assert_eq!(
            fit(1280, 720, 1080),
            Size {
                width: 1280,
                height: 720
            }
        );
        // 21:9 and odd results stay even.
        assert_eq!(
            fit(3440, 1440, 720),
            Size {
                width: 1720,
                height: 720
            }
        );
        assert_eq!(
            fit(1366, 768, 480),
            Size {
                width: 854,
                height: 480
            }
        );
    }

    #[test]
    fn the_ladder_starts_at_the_profile_and_goes_down_by_pixel_rate() {
        let profile = Profile {
            max_short_side: 1080,
            min_short_side: 720,
            max_fps: 60.0,
            min_fps: 30.0,
            ..Profile::messenger()
        };
        let labels: Vec<String> = ladder(3840, 2160, FrameRate::new(60, 1), &profile)
            .iter()
            .map(|t| t.label())
            .collect();
        assert_eq!(labels, ["1080p60", "1080p30", "720p60", "720p30"]);

        let messenger = ladder(1920, 1080, NTSC_60, &Profile::messenger());
        assert_eq!(messenger[0].label(), "720p29.97");
        assert_eq!(messenger.last().unwrap().label(), "360p29.97");
    }

    #[test]
    fn a_small_source_keeps_its_size() {
        let ladder = ladder(640, 360, FrameRate::new(30, 1), &Profile::messenger());
        assert_eq!(
            ladder[0].size,
            Size {
                width: 640,
                height: 360
            }
        );
        assert_eq!(ladder.len(), 1);
    }

    #[test]
    fn the_budget_pays_for_audio_and_container_first() {
        // 25 MB, 60 s, 128 kbit/s: about 3.1 Mbit/s of video.
        let bps = video_budget(25_000_000, 60.0, Some(128)).unwrap();
        assert!((3_000_000.0..3_200_000.0).contains(&bps), "{bps}");
        let silent = video_budget(25_000_000, 60.0, None).unwrap();
        assert!(silent > bps);
        assert_eq!(video_budget(1_000_000, 600.0, Some(128)), None);
        // What the budget predicts stays under the limit.
        assert!(predicted_bytes(bps, 60.0, Some(128)) < 25_000_000);
    }

    #[test]
    fn samples_last_one_keyframe_interval() {
        assert_eq!(gop(FrameRate::new(30, 1)), 120);
        assert_eq!(gop(NTSC_60.divided(2)), 120);
        assert!((sample_seconds(NTSC_60.divided(2)) - 4.004).abs() < 1e-9);
    }
}
