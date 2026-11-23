//! Media information through `ffprobe`.
//!
//! Whether a video is HDR is decided from its own metadata only (the transfer function: PQ or HLG), never
//! from where it comes from: NVIDIA app captures are 10-bit AV1 PQ in full range, while Xbox captures
//! synced through OneDrive are 8-bit H.264 SDR without any color tag. A missing tag means BT.709 in
//! limited range, the H.264/H.265 default.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::plan::FrameRate;

/// How the picture's light is encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dynamic {
    /// Standard dynamic range (BT.709, sRGB, or no tag at all).
    Sdr,
    /// HDR10 / PQ (SMPTE ST 2084).
    Pq,
    /// Hybrid log-gamma (ARIB STD-B67).
    Hlg,
}

impl Dynamic {
    /// `true` for PQ and HLG.
    pub fn is_hdr(self) -> bool {
        self != Self::Sdr
    }

    /// Short label for the queue.
    pub fn label(self) -> String {
        match self {
            Self::Sdr => crate::tr!("media-dynamic-sdr"),
            Self::Pq => crate::tr!("media-dynamic-pq"),
            Self::Hlg => crate::tr!("media-dynamic-hlg"),
        }
    }
}

/// An audio stream of the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioTrack {
    /// Index among the audio streams (`0:a:N`).
    pub index: usize,
    /// Codec name (`aac`, `opus`…).
    pub codec: String,
    /// Channel count.
    pub channels: u32,
    /// The track's description, when the file has one ("System sounds", "Microphone").
    pub title: Option<String>,
}

impl AudioTrack {
    /// `true` when the description names a microphone or a voice chat rather than the game.
    pub fn is_voice(&self) -> bool {
        self.title.as_deref().is_some_and(|title| {
            let title = title.to_lowercase();
            [
                "mic",
                "voice",
                "chat",
                "commentary",
                "comment",
                "narration",
                "party",
            ]
            .iter()
            .any(|word| title.contains(word))
        })
    }

    /// How the track reads in a menu: `2 · Microphone (AAC, stereo)`.
    pub fn label(&self) -> String {
        let layout = crate::tr!("media-audio-layout", channels = self.channels);
        let codec = codec_label(&self.codec);
        let number = self.index + 1;
        match &self.title {
            Some(title) => crate::tr!(
                "media-audio-track-titled",
                number = number,
                title = title.clone(),
                codec = codec,
                layout = layout
            ),
            None => crate::tr!(
                "media-audio-track",
                number = number,
                codec = codec,
                layout = layout
            ),
        }
    }
}

/// The track that carries the game: the first one whose description does not name a microphone or a
/// voice chat, else the first one. `None` without audio.
pub fn game_track(audio: &[AudioTrack]) -> Option<usize> {
    audio
        .iter()
        .find(|t| !t.is_voice())
        .or(audio.first())
        .map(|t| t.index)
}

/// What the application needs to know about a source video.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaInfo {
    /// Picture width in pixels.
    pub width: u32,
    /// Picture height in pixels.
    pub height: u32,
    /// FFmpeg codec name of the video stream (`av1`, `hevc`, `h264`…).
    pub codec: String,
    /// FFmpeg pixel format (`yuv420p10le`…).
    pub pix_fmt: String,
    /// Nominal frame rate (`r_frame_rate`): game captures are often slightly variable, and their average
    /// rate (`avg_frame_rate`) is then an odd fraction near the nominal one.
    pub frame_rate: FrameRate,
    /// Duration of the video stream in seconds.
    pub duration: f64,
    /// First timestamp of the file in seconds: times of the application are relative to it, like `-ss`.
    pub start_time: f64,
    /// Transfer function family.
    pub dynamic: Dynamic,
    /// `true` when the file says full range (`pc`), as NVIDIA app HDR captures do.
    pub full_range: bool,
    /// Color tags as ffprobe names them, for the log (`bt2020nc/smpte2084/bt2020/pc`).
    pub color_tags: String,
    /// No matrix, transfer nor primaries tag at all: the filters then say BT.709 explicitly.
    pub untagged: bool,
    /// The audio streams, in file order.
    pub audio: Vec<AudioTrack>,
    /// File size in bytes.
    pub size: u64,
}

impl MediaInfo {
    /// Frames per second as a float.
    pub fn fps(&self) -> f64 {
        self.frame_rate.as_f64()
    }

    /// Snaps a time to the closest frame start.
    pub fn snap(&self, seconds: f64) -> f64 {
        crate::timecode::snap(seconds, self.fps(), self.duration)
    }

    /// One line describing the source, for the queue (`3840x2160 60 fps AV1 HDR10`).
    pub fn summary(&self) -> String {
        crate::tr!(
            "media-summary",
            width = self.width.to_string(),
            height = self.height.to_string(),
            fps = self.frame_rate.label(),
            codec = codec_label(&self.codec),
            dynamic = self.dynamic.label()
        )
    }
}

/// Display name of an FFmpeg codec name.
pub fn codec_label(codec: &str) -> String {
    match codec {
        "hevc" => "H.265".to_owned(),
        "h264" => "H.264".to_owned(),
        "av1" => "AV1".to_owned(),
        other => other.to_uppercase(),
    }
}

#[derive(Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    color_space: Option<String>,
    color_range: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
    channels: Option<u32>,
    duration: Option<String>,
    #[serde(default)]
    disposition: HashMap<String, i64>,
    /// Matroska stores the stream duration as a `DURATION` tag (`00:02:13.400000000`).
    #[serde(default)]
    tags: HashMap<String, String>,
}

impl ProbeStream {
    fn duration(&self) -> Option<f64> {
        let tag = || {
            self.tags
                .iter()
                .find(|(key, _)| key.to_ascii_uppercase().starts_with("DURATION"))
                .and_then(|(_, value)| crate::timecode::parse(value))
        };
        self.duration
            .as_deref()
            .and_then(|d| d.parse::<f64>().ok())
            .or_else(tag)
            .filter(|d| *d > 0.0)
    }

    fn tag(&self, name: &str) -> Option<String> {
        self.tags
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
            .filter(|value| !value.trim().is_empty())
    }

    /// The description of a track, as players show it: the `title` tag (Matroska), the MP4 `name` tag
    /// (NVIDIA app writes "System sounds" and "Microphone" there), or a handler name that is not one of
    /// the generic ones muxers write ("SoundHandler", "SoundHandle").
    fn track_name(&self) -> Option<String> {
        self.tag("title").or_else(|| self.tag("name")).or_else(|| {
            self.tag("handler_name").filter(|h| {
                let generic = h.trim().to_ascii_lowercase();
                !matches!(
                    generic.as_str(),
                    "soundhandler"
                        | "soundhandle"
                        | "core media audio"
                        | "isomedia file produced by google"
                ) && !generic.starts_with("sound handler")
            })
        })
    }
}

#[derive(Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
    start_time: Option<String>,
    size: Option<String>,
}

/// The frame rates capture devices and players use.
const STANDARD_RATES: [FrameRate; 17] = [
    FrameRate::new(24000, 1001),
    FrameRate::new(24, 1),
    FrameRate::new(25, 1),
    FrameRate::new(30000, 1001),
    FrameRate::new(30, 1),
    FrameRate::new(48, 1),
    FrameRate::new(50, 1),
    FrameRate::new(60000, 1001),
    FrameRate::new(60, 1),
    FrameRate::new(72, 1),
    FrameRate::new(90, 1),
    FrameRate::new(100, 1),
    FrameRate::new(120000, 1001),
    FrameRate::new(120, 1),
    FrameRate::new(144, 1),
    FrameRate::new(165, 1),
    FrameRate::new(240, 1),
];

/// The standard rate nearest to `rate` and its relative distance.
fn nearest_standard(rate: FrameRate) -> (FrameRate, f64) {
    STANDARD_RATES
        .iter()
        .map(|&standard| (standard, (rate.as_f64() / standard.as_f64() - 1.0).abs()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("the list is not empty")
}

/// The nominal frame rate of a stream from ffprobe's `r_frame_rate` (its guess of the base rate) and
/// `avg_frame_rate` (frames over duration).
///
/// Game captures have a variable frame rate, and ffprobe's guess can be far off: an Xbox HDR capture
/// reports `r_frame_rate=56/1` for an average of 59.3 fps, which would make 28 fps outputs. So: the guess
/// when it is a standard rate; else the standard rate within 3 % of the average (59.3 → 59.94); else the
/// guess when it agrees with the average within 5 %, else the average.
fn nominal_rate(guess: Option<FrameRate>, average: Option<FrameRate>) -> FrameRate {
    if let Some(guess) = guess
        && nearest_standard(guess).1 < 0.001
    {
        return guess;
    }
    if let Some(average) = average {
        let (standard, distance) = nearest_standard(average);
        if distance < 0.03 {
            return standard;
        }
    }
    match (guess, average) {
        (Some(guess), Some(average)) if (guess.as_f64() / average.as_f64() - 1.0).abs() < 0.05 => {
            guess
        }
        (_, Some(average)) => FrameRate::new((average.as_f64() * 1000.0).round() as u32, 1000),
        (Some(guess), None) => guess,
        (None, None) => FrameRate::new(30, 1),
    }
}

/// Parses the JSON printed by `ffprobe -print_format json -show_format -show_streams`.
pub fn parse_probe_json(json: &str) -> Result<MediaInfo> {
    let output: ProbeOutput =
        serde_json::from_str(json).with_context(|| crate::tr!("media-probe-invalid"))?;
    // Cover art is a video stream too (`attached_pic`): skip it.
    let video = output
        .streams
        .iter()
        .find(|s| {
            s.codec_type.as_deref() == Some("video")
                && s.disposition.get("attached_pic").copied().unwrap_or(0) == 0
        })
        .with_context(|| crate::tr!("media-no-video"))?;
    let (Some(width), Some(height)) = (video.width, video.height) else {
        bail!(crate::tr!("media-no-dimensions"));
    };
    let parse = |rate: &Option<String>| {
        rate.as_deref()
            .and_then(FrameRate::parse)
            .filter(|r| r.as_f64() <= 1000.0)
    };
    let frame_rate = nominal_rate(parse(&video.r_frame_rate), parse(&video.avg_frame_rate));
    // The video stream's own duration: the audio often lasts a little longer.
    let duration = video
        .duration()
        .or_else(|| {
            let format = output.format.as_ref()?.duration.as_deref()?;
            format.parse::<f64>().ok()
        })
        .unwrap_or(0.0);
    let format = output.format.as_ref();
    let start_time = format
        .and_then(|f| f.start_time.as_deref()?.parse::<f64>().ok())
        .filter(|t| t.is_finite())
        .unwrap_or(0.0);
    let size = format
        .and_then(|f| f.size.as_deref()?.parse::<u64>().ok())
        .unwrap_or(0);
    let known = |value: &Option<String>| value.clone().filter(|v| v != "unknown");
    let transfer = known(&video.color_transfer);
    let dynamic = match transfer.as_deref() {
        Some("smpte2084") => Dynamic::Pq,
        Some("arib-std-b67") => Dynamic::Hlg,
        _ => Dynamic::Sdr,
    };
    let tag = |value: &Option<String>| known(value).unwrap_or_else(|| "?".to_owned());
    let color_tags = format!(
        "{}/{}/{}/{}",
        tag(&video.color_space),
        tag(&video.color_transfer),
        tag(&video.color_primaries),
        tag(&video.color_range)
    );
    let audio = output
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("audio"))
        .enumerate()
        .map(|(index, s)| AudioTrack {
            index,
            codec: s.codec_name.clone().unwrap_or_default(),
            channels: s.channels.unwrap_or(2),
            title: s.track_name(),
        })
        .collect();
    Ok(MediaInfo {
        width,
        height,
        codec: video.codec_name.clone().unwrap_or_default(),
        pix_fmt: video.pix_fmt.clone().unwrap_or_default(),
        frame_rate,
        duration,
        start_time,
        dynamic,
        full_range: matches!(video.color_range.as_deref(), Some("pc" | "jpeg")),
        untagged: known(&video.color_space).is_none()
            && transfer.is_none()
            && known(&video.color_primaries).is_none(),
        color_tags,
        audio,
        size,
    })
}

/// Runs `ffprobe` on a file.
pub fn probe(path: &Path) -> Result<MediaInfo> {
    let output = super::tool("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdin(std::process::Stdio::null())
        .output()
        .with_context(|| crate::tr!("media-ffprobe-start-failed"))?;
    if !output.status.success() {
        bail!(crate::tr!(
            "media-ffprobe-failed",
            detail = String::from_utf8_lossy(&output.stderr).trim().to_owned()
        ));
    }
    parse_probe_json(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The shape of an NVIDIA app HDR capture: AV1 10-bit PQ in full range, two audio tracks.
    const HDR_CAPTURE: &str = r#"{
        "streams": [
            {"codec_type": "video", "codec_name": "av1", "width": 3840, "height": 2160,
             "pix_fmt": "yuv420p10le", "r_frame_rate": "60/1", "avg_frame_rate": "125730000/2095409",
             "color_space": "bt2020nc", "color_range": "pc", "color_transfer": "smpte2084",
             "color_primaries": "bt2020", "duration": "23.282322"},
            {"codec_type": "audio", "codec_name": "aac", "channels": 2},
            {"codec_type": "audio", "codec_name": "aac", "channels": 2, "tags": {"title": "Mic"}}
        ],
        "format": {"duration": "23.3", "start_time": "0.000000", "size": "132711803"}
    }"#;

    // The shape of an Xbox capture synced through OneDrive: H.264 SDR, variable rate, no color tag.
    const XBOX_CAPTURE: &str = r#"{
        "streams": [
            {"codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
             "pix_fmt": "yuv420p", "r_frame_rate": "60000/1001", "avg_frame_rate": "52215000/886891",
             "color_range": "unknown", "color_space": "unknown", "duration": "59.178563"},
            {"codec_type": "audio", "codec_name": "aac", "channels": 2}
        ],
        "format": {"duration": "59.2", "size": "140201295"}
    }"#;

    #[test]
    fn reads_an_hdr_capture() {
        let info = parse_probe_json(HDR_CAPTURE).unwrap();
        assert_eq!((info.width, info.height), (3840, 2160));
        assert_eq!(info.dynamic, Dynamic::Pq);
        assert!(info.full_range);
        assert_eq!(info.frame_rate, FrameRate::new(60, 1));
        assert_eq!(info.audio.len(), 2);
        assert_eq!(info.audio[1].title.as_deref(), Some("Mic"));
        assert_eq!(info.size, 132_711_803);
        assert_eq!(info.summary(), "3840x2160 60 fps AV1 HDR10");
    }

    #[test]
    fn an_untagged_capture_is_sdr_in_limited_range() {
        let info = parse_probe_json(XBOX_CAPTURE).unwrap();
        assert_eq!(info.dynamic, Dynamic::Sdr);
        assert!(!info.full_range);
        assert_eq!(info.frame_rate, FrameRate::new(60000, 1001));
        assert_eq!(info.color_tags, "?/?/?/?");
        assert!(info.untagged);
        assert!(!parse_probe_json(HDR_CAPTURE).unwrap().untagged);
        assert!((info.duration - 59.178563).abs() < 1e-9);
    }

    #[test]
    fn finds_the_nominal_rate_of_variable_rate_captures() {
        let rate = |text: &str| FrameRate::parse(text);
        // Xbox HDR capture: ffprobe guesses 56 fps for an average of 59.3.
        assert_eq!(
            nominal_rate(rate("56/1"), rate("52440000/884321")),
            FrameRate::new(60000, 1001)
        );
        // Xbox SDR capture: the guess is standard, the average is lower.
        assert_eq!(
            nominal_rate(rate("60000/1001"), rate("52215000/886891")),
            FrameRate::new(60000, 1001)
        );
        // NVIDIA app capture: 60 guessed, 60.0 on average.
        assert_eq!(
            nominal_rate(rate("60/1"), rate("125730000/2095409")),
            FrameRate::new(60, 1)
        );
        // A real, non standard rate is kept.
        assert_eq!(
            nominal_rate(rate("56/1"), rate("56/1")),
            FrameRate::new(56, 1)
        );
        assert_eq!(nominal_rate(None, None), FrameRate::new(30, 1));
    }

    #[test]
    fn names_the_tracks_and_finds_the_game_one() {
        // NVIDIA app: the description is the MP4 `name` tag, the handler name is generic. Here the
        // microphone comes first, to check the game track is found by its name, not its place.
        let json = r#"{"streams": [
            {"codec_type": "video", "width": 10, "height": 10},
            {"codec_type": "audio", "codec_name": "aac", "channels": 2,
             "tags": {"handler_name": "SoundHandle", "name": "Microphone"}},
            {"codec_type": "audio", "codec_name": "aac", "channels": 2,
             "tags": {"handler_name": "SoundHandle", "name": "System sounds"}},
            {"codec_type": "audio", "codec_name": "opus", "channels": 6,
             "tags": {"handler_name": "SoundHandler"}}]}"#;
        let info = parse_probe_json(json).unwrap();
        let titles: Vec<Option<&str>> = info.audio.iter().map(|t| t.title.as_deref()).collect();
        assert_eq!(titles, [Some("Microphone"), Some("System sounds"), None]);
        assert_eq!(game_track(&info.audio), Some(1));
        assert_eq!(info.audio[0].label(), "1 · Microphone (AAC, stereo)");
        assert_eq!(info.audio[2].label(), "3 · OPUS, 5.1");
        // Nothing named: the first track.
        assert_eq!(game_track(&info.audio[2..]), Some(2));
        assert_eq!(game_track(&[]), None);
    }

    #[test]
    fn skips_cover_art_and_reads_matroska_durations() {
        let json = r#"{"streams": [
            {"codec_type": "video", "codec_name": "mjpeg", "width": 600, "height": 600,
             "disposition": {"attached_pic": 1}},
            {"codec_type": "video", "codec_name": "hevc", "width": 10, "height": 10,
             "color_transfer": "arib-std-b67", "tags": {"DURATION": "00:00:10.000000000"}}],
            "format": {"duration": "12.023000"}}"#;
        let info = parse_probe_json(json).unwrap();
        assert_eq!(info.codec, "hevc");
        assert_eq!(info.duration, 10.0);
        assert_eq!(info.dynamic, Dynamic::Hlg);
        assert!(info.audio.is_empty());
    }
}
