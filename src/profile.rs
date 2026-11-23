//! Output profiles: what a destination accepts (size, resolution, frame rate, codec, HDR or not) and how
//! hard the encoder may try.
//!
//! Profiles live in `profiles.toml` in the application's data folder, editable by hand or in the
//! application. A queued video holds its own copy of a profile ([`Profile`]), so changing the settings of
//! one video never changes the profile, nor the other videos.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::ffmpeg::encoders::VideoCodec;
use crate::ffmpeg::probe::{AudioTrack, game_track};

/// The libplacebo tone-mapping curve used to turn HDR into SDR.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToneMapping {
    /// libplacebo's own choice (its "spline" curve in recent versions): the recommended default.
    #[default]
    Auto,
    /// The ITU-R BT.2390 EETF, the broadcast reference.
    Bt2390,
    /// A perceptually smooth spline.
    Spline,
    /// The filmic curve from Uncharted 2: punchy, darker mid-tones.
    Hable,
    /// Keeps in-range colors exact and compresses only the highlights.
    Mobius,
}

impl ToneMapping {
    /// Every curve, in menu order.
    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::Bt2390,
        Self::Spline,
        Self::Hable,
        Self::Mobius,
    ];

    /// The value of libplacebo's `tonemapping` option.
    pub fn placebo(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Bt2390 => "bt.2390",
            Self::Spline => "spline",
            Self::Hable => "hable",
            Self::Mobius => "mobius",
        }
    }

    /// The curve of FFmpeg's CPU `tonemap` filter, used when libplacebo is not available.
    pub fn software(self) -> &'static str {
        match self {
            Self::Auto | Self::Bt2390 | Self::Spline | Self::Hable => "hable",
            Self::Mobius => "mobius",
        }
    }

    /// Name shown in the UI.
    pub fn label(self) -> String {
        match self {
            Self::Auto => crate::tr!("profile-tone-auto"),
            Self::Bt2390 => crate::tr!("profile-tone-bt2390"),
            Self::Spline => crate::tr!("profile-tone-spline"),
            Self::Hable => crate::tr!("profile-tone-hable"),
            Self::Mobius => crate::tr!("profile-tone-mobius"),
        }
    }
}

/// Which audio streams go into the output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "track")]
pub enum AudioChoice {
    /// The game's track: the first whose description does not name a microphone or a voice chat
    /// ([`crate::ffmpeg::probe::game_track`]). Written `first` by version 0.1 profiles.
    #[default]
    #[serde(alias = "first")]
    Auto,
    /// One stream by its index among the audio streams.
    Track(usize),
    /// Every audio stream mixed together (game and a separate microphone track).
    MixAll,
    /// No sound.
    None,
}

impl AudioChoice {
    /// The audio streams (indexes among the audio streams) this choice takes from a video's tracks:
    /// one, all of them mixed, or none.
    pub fn tracks(self, audio: &[AudioTrack]) -> Vec<usize> {
        match self {
            Self::Auto => game_track(audio).into_iter().collect(),
            Self::Track(index) => audio
                .iter()
                .filter(|t| t.index == index)
                .map(|t| t.index)
                .collect(),
            Self::MixAll => audio.iter().map(|t| t.index).collect(),
            Self::None => Vec::new(),
        }
    }
}

/// A destination's limits and the encoding choices made for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Name shown in the menus.
    pub name: String,
    /// Added to the source's file name for the output (`clip_messenger.mp4`).
    pub suffix: String,
    /// Largest output file, in megabytes of 1,000,000 bytes (the smaller reading of "MB").
    pub max_size_mb: f64,
    /// Largest short side of the picture (720 for 1280x720 or 720x1280). Never upscaled.
    pub max_short_side: u32,
    /// Smallest short side the size fitting may go down to before giving up.
    pub min_short_side: u32,
    /// Highest frame rate. The output takes the source rate divided by a whole number, so 60 fps becomes
    /// 30 fps and 59.94 fps becomes 29.97 fps, without judder.
    pub max_fps: f64,
    /// Lowest frame rate the size fitting may go down to.
    pub min_fps: f64,
    /// The codecs the destination plays, preferred first. The encode takes the first one with a hardware
    /// encoder on this machine, else the first one with a software encoder
    /// ([`crate::ffmpeg::hardware::Capabilities::choose`]). Version 0.1 files wrote a single `codec`.
    #[serde(alias = "codec", deserialize_with = "one_or_many")]
    pub codecs: Vec<VideoCodec>,
    /// Convert HDR sources to SDR. Off: an HDR source stays HDR (10-bit H.265 or AV1 only).
    pub sdr_only: bool,
    /// Curve used to convert HDR to SDR.
    pub tone_mapping: ToneMapping,
    /// AAC bitrate in kbit/s.
    pub audio_bitrate_kbps: u32,
    /// Which audio streams go into the output.
    pub audio: AudioChoice,
    /// Best quality worth spending bits on, as an H.265 QP (lower is better): an easy clip ends up smaller
    /// than the limit rather than wasting bytes.
    pub quality_ceiling: u32,
    /// Worst acceptable quality, as an H.265 QP: past it the size fitting lowers the frame rate, then the
    /// resolution.
    pub quality_floor: u32,
}

impl Default for Profile {
    fn default() -> Self {
        Self::messenger()
    }
}

impl Profile {
    /// The built-in profile, from the settings used for years with a `Send to` script: 720p 30 fps H.265
    /// (else H.264) under 25 MB, SDR, 128 kbit/s AAC. No AV1: whether Messenger re-encodes it worse is not
    /// measured yet (docs/messenger.md).
    pub fn messenger() -> Self {
        Self {
            name: "Messenger".to_owned(),
            suffix: "_messenger".to_owned(),
            max_size_mb: 25.0,
            max_short_side: 720,
            min_short_side: 360,
            max_fps: 30.0,
            min_fps: 24.0,
            codecs: vec![VideoCodec::H265, VideoCodec::H264],
            sdr_only: true,
            tone_mapping: ToneMapping::Auto,
            audio_bitrate_kbps: 128,
            audio: AudioChoice::Auto,
            quality_ceiling: 20,
            quality_floor: 32,
        }
    }

    /// A Discord profile: AV1, else H.265, else H.264, up to 1080p60 (2160p60 with `max_short_side`),
    /// SDR, under `max_size_mb`. Discord plays MP4 in H.264, HEVC or AV1 and checks the size before any
    /// compression; the desktop app sends the file as is. See docs/discord.md.
    fn discord(name: &str, max_size_mb: f64, max_short_side: u32, audio_bitrate_kbps: u32) -> Self {
        Self {
            name: name.to_owned(),
            suffix: "_discord".to_owned(),
            max_size_mb,
            max_short_side,
            min_short_side: 360,
            max_fps: 60.0,
            min_fps: 24.0,
            codecs: vec![VideoCodec::Av1, VideoCodec::H265, VideoCodec::H264],
            audio_bitrate_kbps,
            ..Self::messenger()
        }
    }

    /// Every built-in profile, the default first: Messenger, then Discord for each upload limit (free
    /// accounts, Nitro Basic or a level 2 server, a level 3 server, Nitro).
    pub fn built_ins() -> Vec<Self> {
        vec![
            Self::messenger(),
            Self::discord("Discord (20 MB)", 20.0, 1080, 128),
            Self::discord(
                "Discord Nitro Basic / server level 2 (50 MB)",
                50.0,
                1080,
                160,
            ),
            Self::discord("Discord server level 3 (100 MB)", 100.0, 1080, 160),
            Self::discord("Discord Nitro (1 GB)", 1000.0, 2160, 192),
        ]
    }

    /// Largest output size in bytes.
    pub fn max_bytes(&self) -> u64 {
        (self.max_size_mb.max(0.1) * 1_000_000.0) as u64
    }

    /// Fixes values that cannot work (a floor better than the ceiling, a minimum above the maximum…) and
    /// returns what was changed.
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut changes = Vec::new();
        if self.name.trim().is_empty() {
            self.name = "Unnamed".to_owned();
            changes.push("empty name".to_owned());
        }
        if !(self.max_size_mb.is_finite() && self.max_size_mb >= 0.5) {
            self.max_size_mb = 0.5;
            changes.push("size limit below 0.5 MB".to_owned());
        }
        self.max_short_side = self.max_short_side.clamp(144, 4320);
        self.min_short_side = self.min_short_side.clamp(144, self.max_short_side);
        if !(self.max_fps.is_finite() && self.max_fps >= 1.0) {
            self.max_fps = 30.0;
            changes.push("invalid frame rate".to_owned());
        }
        if !(self.min_fps.is_finite() && self.min_fps >= 1.0) || self.min_fps > self.max_fps {
            self.min_fps = self.max_fps.min(24.0);
        }
        self.audio_bitrate_kbps = self.audio_bitrate_kbps.clamp(32, 512);
        self.quality_ceiling = self.quality_ceiling.clamp(10, 45);
        self.quality_floor = self.quality_floor.clamp(self.quality_ceiling, 51);
        let mut seen = Vec::new();
        self.codecs.retain(|codec| {
            let first = !seen.contains(codec);
            seen.push(*codec);
            first
        });
        if self.codecs.is_empty() {
            self.codecs = vec![VideoCodec::H265];
            changes.push("no codec".to_owned());
        }
        if !self.sdr_only && !self.codecs.iter().any(|c| c.supports_hdr()) {
            self.sdr_only = true;
            changes.push("HDR needs H.265 or AV1: SDR only".to_owned());
        }
        changes
    }

    /// The codec list as the UI shows it: `AV1 > H.265 > H.264`.
    pub fn codecs_label(&self) -> String {
        self.codecs
            .iter()
            .map(|c| c.label())
            .collect::<Vec<_>>()
            .join(" > ")
    }
}

/// Reads a codec list written as one value (`codec = "h265"`, version 0.1) or as a list.
fn one_or_many<'de, D>(deserializer: D) -> Result<Vec<VideoCodec>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(VideoCodec),
        Many(Vec<VideoCodec>),
    }
    Ok(match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(codec) => vec![codec],
        OneOrMany::Many(codecs) => codecs,
    })
}

/// The profiles file: an ordered list, the first one being the default for new videos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileSet {
    /// The profiles, in menu order.
    #[serde(rename = "profile", default)]
    pub profiles: Vec<Profile>,
}

impl Default for ProfileSet {
    fn default() -> Self {
        Self {
            profiles: Profile::built_ins(),
        }
    }
}

/// The user-facing "cannot read" text for a profiles file.
fn cannot_read(path: &Path) -> String {
    crate::tr!("profile-cannot-read", path = path.display().to_string())
}

impl ProfileSet {
    /// Name of the file in the data folder.
    pub const FILE_NAME: &'static str = "profiles.toml";

    /// Parses the TOML text of a profiles file. Missing fields take the built-in profile's values.
    pub fn parse(text: &str) -> Result<Self> {
        let mut set: Self =
            toml::from_str(text).with_context(|| crate::tr!("profile-invalid-file"))?;
        if set.profiles.is_empty() {
            set = Self::default();
        }
        for profile in &mut set.profiles {
            profile.sanitize();
        }
        Ok(set)
    }

    /// The TOML text of this set.
    pub fn to_toml(&self) -> String {
        let body = toml::to_string_pretty(self).unwrap_or_default();
        format!(
            "# quick-video-transcoder output profiles. The first one is used for new videos.\n\
             # Sizes are in MB of 1,000,000 bytes; qualities are H.265 QPs (lower is better).\n\n{body}"
        )
    }

    /// Reads `path`, or the built-in set when the file does not exist yet.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).with_context(|| cannot_read(path)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| cannot_read(path)),
        }
    }

    /// Writes the set to `path` through a temporary file, so a crash never leaves half a file.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| {
                crate::tr!("profile-cannot-create", path = dir.display().to_string())
            })?;
        }
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, self.to_toml()).with_context(|| {
            crate::tr!(
                "profile-cannot-write",
                path = temporary.display().to_string()
            )
        })?;
        std::fs::rename(&temporary, path).with_context(|| {
            crate::tr!("profile-cannot-replace", path = path.display().to_string())
        })
    }

    /// The profile named `name`, if there is one.
    pub fn find(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    /// The default profile for new videos.
    pub fn first(&self) -> Profile {
        self.profiles.first().cloned().unwrap_or_default()
    }
}

/// The application's data folder (`%APPDATA%\quick-video-transcoder\data` on Windows), where eframe keeps
/// its own settings too.
pub fn data_dir() -> Option<PathBuf> {
    eframe::storage_dir(crate::app::APP_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_profile_round_trips() {
        let set = ProfileSet::default();
        let text = set.to_toml();
        assert!(text.contains("[[profile]]"), "{text}");
        assert!(text.contains("name = \"Messenger\""), "{text}");
        assert_eq!(ProfileSet::parse(&text).unwrap(), set);
    }

    #[test]
    fn missing_fields_take_the_built_in_values() {
        let set = ProfileSet::parse(
            "[[profile]]\nname = \"Discord\"\nmax_size_mb = 10\n\n[profile.audio]\nkind = \"mix-all\"\n",
        )
        .unwrap();
        let discord = set.find("Discord").unwrap();
        assert_eq!(discord.max_bytes(), 10_000_000);
        assert_eq!(discord.max_short_side, 720);
        assert_eq!(discord.audio, AudioChoice::MixAll);
        // Version 0.1 wrote the game track choice as `first`.
        let old =
            ProfileSet::parse("[[profile]]\nname = \"Old\"\n[profile.audio]\nkind = \"first\"\n")
                .unwrap();
        assert_eq!(old.profiles[0].audio, AudioChoice::Auto);
        assert_eq!(ProfileSet::parse("").unwrap(), ProfileSet::default());
    }

    #[test]
    fn sanitizing_fixes_impossible_values() {
        let mut profile = Profile {
            quality_ceiling: 30,
            quality_floor: 20,
            min_short_side: 2000,
            sdr_only: false,
            codecs: vec![VideoCodec::H264, VideoCodec::H264],
            ..Profile::messenger()
        };
        let changes = profile.sanitize();
        assert_eq!(profile.quality_floor, 30);
        assert_eq!(profile.min_short_side, 720);
        assert_eq!(profile.codecs, [VideoCodec::H264]);
        assert!(profile.sdr_only);
        assert_eq!(changes.len(), 1, "{changes:?}");
        let mut empty = Profile {
            codecs: Vec::new(),
            ..Profile::messenger()
        };
        empty.sanitize();
        assert_eq!(empty.codecs, [VideoCodec::H265]);
    }

    #[test]
    fn reads_a_single_codec_from_older_files() {
        let set = ProfileSet::parse("[[profile]]\nname = \"Old\"\ncodec = \"av1\"\n").unwrap();
        assert_eq!(set.profiles[0].codecs, [VideoCodec::Av1]);
        let set = ProfileSet::parse("[[profile]]\nname = \"New\"\ncodecs = [\"av1\", \"h264\"]\n")
            .unwrap();
        assert_eq!(set.profiles[0].codecs, [VideoCodec::Av1, VideoCodec::H264]);
        assert_eq!(set.profiles[0].codecs_label(), "AV1 > H.264");
    }

    #[test]
    fn the_readme_example_is_the_built_in_profile() {
        let readme = include_str!("../README.md");
        let start = readme.find("```toml\n").expect("a TOML example") + "```toml\n".len();
        let end = start + readme[start..].find("```").expect("its end");
        let set = ProfileSet::parse(&readme[start..end]).unwrap();
        assert_eq!(set.profiles, [Profile::messenger()]);
    }

    #[test]
    fn the_built_in_profiles_are_valid_and_distinct() {
        let built_ins = Profile::built_ins();
        assert_eq!(built_ins[0], Profile::messenger());
        for profile in &built_ins {
            let mut sanitized = profile.clone();
            assert!(sanitized.sanitize().is_empty(), "{}", profile.name);
            assert_eq!(&sanitized, profile, "{}", profile.name);
        }
        let mut names: Vec<&str> = built_ins.iter().map(|p| p.name.as_str()).collect();
        names.dedup();
        assert_eq!(names.len(), built_ins.len());
        let discord = &built_ins[1];
        assert_eq!(
            (discord.max_bytes(), discord.max_short_side, discord.max_fps),
            (20_000_000, 1080, 60.0)
        );
    }

    #[test]
    fn saves_through_a_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(ProfileSet::FILE_NAME);
        assert_eq!(ProfileSet::load(&path).unwrap(), ProfileSet::default());
        let mut set = ProfileSet::default();
        set.profiles[0].max_size_mb = 99.0;
        set.save(&path).unwrap();
        assert_eq!(ProfileSet::load(&path).unwrap(), set);
        assert!(!path.with_extension("toml.tmp").exists());
    }
}
