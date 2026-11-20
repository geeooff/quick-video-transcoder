//! What works with the installed FFmpeg on this machine: which encoders, and whether libplacebo runs on a
//! Vulkan device (the GPU tone mapping and scaling of [`super::filters`]).
//!
//! An encoder compiled into FFmpeg does not mean it works (no GPU of that brand, driver, OS), so every
//! candidate is tried with a tiny encode, all in parallel (about 0.5 s).

use std::collections::HashSet;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::background_tool;
use super::encoders::{self, EncoderBackend, EncoderSpec, VideoCodec};

/// Encoders and filters that work on this machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Working codec / backend pairs.
    pub encoders: Vec<(VideoCodec, EncoderBackend)>,
    /// libplacebo runs on a Vulkan device: GPU scaling and tone mapping.
    pub placebo: bool,
    /// `false` until the detection has run (the values are then only likely).
    pub detected: bool,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::assumed()
    }
}

impl Capabilities {
    /// What is likely available until the detection finishes: NVENC and the CPU encoders, libplacebo.
    pub fn assumed() -> Self {
        Self {
            encoders: encoders::all()
                .iter()
                .filter(|s| matches!(s.backend, EncoderBackend::Nvenc | EncoderBackend::Software))
                .map(|s| (s.codec, s.backend))
                .collect(),
            placebo: true,
            detected: false,
        }
    }

    /// Tries every encoder compiled into FFmpeg and libplacebo, in parallel.
    pub fn detect() -> Self {
        let compiled = compiled_encoders();
        let candidates: Vec<&EncoderSpec> = encoders::all()
            .iter()
            .filter(|s| compiled.contains(s.name))
            .collect();
        std::thread::scope(|scope| {
            let tests: Vec<_> = candidates
                .iter()
                .map(|spec| scope.spawn(move || (spec, test_encoder(spec.name))))
                .collect();
            let placebo = scope.spawn(test_placebo);
            Self {
                encoders: tests
                    .into_iter()
                    .filter_map(|t| t.join().ok())
                    .filter(|(_, works)| *works)
                    .map(|(spec, _)| (spec.codec, spec.backend))
                    .collect(),
                placebo: placebo.join().unwrap_or(false),
                detected: true,
            }
        })
    }

    /// `true` when this pair works.
    pub fn has_encoder(&self, codec: VideoCodec, backend: EncoderBackend) -> bool {
        self.encoders.contains(&(codec, backend))
    }

    /// The best working encoder of a codec: `preferred` when it works, else the first working backend in
    /// [`EncoderBackend::ALL`] order. `None` when nothing encodes this codec.
    pub fn encoder(
        &self,
        codec: VideoCodec,
        preferred: Option<EncoderBackend>,
    ) -> Option<&'static EncoderSpec> {
        preferred
            .filter(|&b| self.has_encoder(codec, b))
            .or_else(|| {
                EncoderBackend::ALL
                    .into_iter()
                    .find(|&b| self.has_encoder(codec, b))
            })
            .map(|backend| encoders::find(codec, backend))
    }

    /// Working backends for a codec, best first.
    pub fn backends(&self, codec: VideoCodec) -> Vec<EncoderBackend> {
        EncoderBackend::ALL
            .into_iter()
            .filter(|&b| self.has_encoder(codec, b))
            .collect()
    }

    /// One line for the log.
    pub fn summary(&self) -> String {
        let pairs: Vec<String> = EncoderBackend::ALL
            .into_iter()
            .filter_map(|backend| {
                let codecs: Vec<&str> = VideoCodec::ALL
                    .into_iter()
                    .filter(|&c| self.has_encoder(c, backend))
                    .map(VideoCodec::label)
                    .collect();
                (!codecs.is_empty()).then(|| format!("{} {}", backend.label(), codecs.join("/")))
            })
            .collect();
        format!(
            "encoders: {}; GPU tone mapping (libplacebo): {}",
            if pairs.is_empty() {
                "none".to_owned()
            } else {
                pairs.join(", ")
            },
            if self.placebo { "yes" } else { "no" }
        )
    }
}

/// Names of the encoders compiled into FFmpeg (`ffmpeg -encoders`).
fn compiled_encoders() -> HashSet<String> {
    let Ok(output) = background_tool("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .stdin(Stdio::null())
        .output()
    else {
        return HashSet::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let flags = words.next()?;
            // Encoder lines start with capability flags such as "V....D".
            (flags.len() == 6 && flags.starts_with(['V', 'A', 'S'])).then(|| words.next())?
        })
        .map(str::to_owned)
        .collect()
}

/// Runs ffmpeg with `args`; `true` if it succeeds within `timeout`.
pub fn run_test(args: &[&str], timeout: Duration) -> bool {
    let Ok(mut child) = background_tool("ffmpeg")
        .args(["-hide_banner", "-nostdin", "-loglevel", "quiet"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                super::kill_tree(&mut child);
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn test_encoder(name: &str) -> bool {
    run_test(
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=gray:s=256x256:r=30:d=0.1",
            "-c:v",
            name,
            "-f",
            "null",
            "-",
        ],
        Duration::from_secs(10),
    )
}

/// The device options libplacebo needs, before `-i`.
pub const VULKAN_DEVICE: [&str; 4] = ["-init_hw_device", "vulkan=vk", "-filter_hw_device", "vk"];

fn test_placebo() -> bool {
    let mut args: Vec<&str> = VULKAN_DEVICE.to_vec();
    args.extend([
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=128x72:d=0.1",
        "-vf",
        "hwupload,libplacebo=w=64:h=36:format=nv12,hwdownload,format=nv12",
        "-f",
        "null",
        "-",
    ]);
    run_test(&args, Duration::from_secs(15))
}

/// Whether the Vulkan decoder reads `path` (its first frame, through libplacebo, as the filters use it).
/// About 0.3 s; the answer is kept per video.
pub fn vulkan_decodes(path: &Path) -> bool {
    let path = path.to_string_lossy();
    let mut args: Vec<&str> = VULKAN_DEVICE.to_vec();
    args.extend([
        "-hwaccel",
        "vulkan",
        "-hwaccel_output_format",
        "vulkan",
        "-i",
        &path,
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-vf",
        "libplacebo=w=64:h=36:format=nv12,hwdownload,format=nv12",
        "-f",
        "null",
        "-",
    ]);
    run_test(&args, Duration::from_secs(15))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nvidia() -> Capabilities {
        Capabilities {
            encoders: vec![
                (VideoCodec::H265, EncoderBackend::Nvenc),
                (VideoCodec::H265, EncoderBackend::Software),
                (VideoCodec::H264, EncoderBackend::Software),
            ],
            placebo: true,
            detected: true,
        }
    }

    #[test]
    fn picks_the_preferred_backend_when_it_works() {
        let caps = nvidia();
        assert_eq!(
            caps.encoder(VideoCodec::H265, None).unwrap().name,
            "hevc_nvenc"
        );
        assert_eq!(
            caps.encoder(VideoCodec::H265, Some(EncoderBackend::Software))
                .unwrap()
                .name,
            "libx265"
        );
        assert_eq!(
            caps.encoder(VideoCodec::H264, Some(EncoderBackend::Nvenc))
                .unwrap()
                .name,
            "libx264"
        );
        assert_eq!(caps.encoder(VideoCodec::Av1, None), None);
        assert_eq!(
            caps.backends(VideoCodec::H265),
            [EncoderBackend::Nvenc, EncoderBackend::Software]
        );
    }

    #[test]
    fn summarizes_for_the_log() {
        assert_eq!(
            nvidia().summary(),
            "encoders: NVENC H.265, CPU H.265/H.264; GPU tone mapping (libplacebo): yes"
        );
    }
}
