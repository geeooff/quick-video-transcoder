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
                .map(|spec| scope.spawn(move || (spec, test_encoder(spec))))
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

    /// The encoder for a profile's codecs (preferred first):
    /// - with a `preferred` backend (the user's choice in the settings), the first codec it encodes;
    /// - else the first codec with a hardware encoder, on its best hardware backend: a laptop encoding AV1
    ///   on NVENC and H.265 on Quick Sync gets AV1 on NVENC for `AV1 > H.265`;
    /// - else the first codec with a software encoder.
    ///
    /// Hardware wins over the list's order only through this fallback: a codec the destination plays and
    /// the GPU encodes is always taken over a slower CPU encode of a preferred one (an RTX 3080 has no AV1
    /// encoder, so `AV1 > H.265` gives H.265 on NVENC there).
    pub fn choose(
        &self,
        codecs: &[VideoCodec],
        preferred: Option<EncoderBackend>,
    ) -> Option<&'static EncoderSpec> {
        let on = |backend: EncoderBackend| {
            codecs
                .iter()
                .find(|&&c| self.has_encoder(c, backend))
                .map(|&c| encoders::find(c, backend))
        };
        if let Some(found) = preferred.and_then(on) {
            return Some(found);
        }
        codecs
            .iter()
            .find_map(|&codec| {
                EncoderBackend::ALL
                    .into_iter()
                    .filter(|&b| b != EncoderBackend::Software)
                    .find(|&b| self.has_encoder(codec, b))
                    .map(|b| encoders::find(codec, b))
            })
            .or_else(|| on(EncoderBackend::Software))
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

/// Tries an encoder with the options the estimate and the encode really use (constant QP, then the
/// bitrate rate control with its lookahead, B-frames and multipass), on a tiny picture. A bare `-c:v`
/// test passes on GPUs that then refuse those options (older NVENC generations have no B-frames for
/// H.265), and every estimate would fail.
fn test_encoder(spec: &EncoderSpec) -> bool {
    use super::encoders::Depth;
    let mut args: Vec<String> = ["-f", "lavfi", "-i", "testsrc2=s=320x240:r=30:d=0.5"]
        .map(String::from)
        .to_vec();
    args.extend(spec.constant_qp_args(26.0, 30, Depth::Sdr8));
    args.extend(["-f", "null", "-"].map(String::from));
    args.extend(spec.bitrate_args(1_000_000.0, 30, Depth::Sdr8));
    args.extend(["-f", "null", "-"].map(String::from));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run_test(&args, Duration::from_secs(15))
}

/// Most encodes the measurement tries at once.
pub const MAX_PARALLEL_ENCODES: usize = 4;

/// How many encodes at once to run, from the total frames per second measured with 1, 2, 3… encodes
/// running together (`totals[n - 1]` for `n` encodes): the fewest that reach 85 % of the best total. More
/// encodes than that only share the same engines.
///
/// The whole curve is needed, not the first step: the driver decides which engine a session runs on, and
/// it can put the first two on the same engine. An RTX 4090 (two NVENC engines) measured 399, 789, 852 and
/// 881 frames per second for 1 to 4 encodes on 2026-11-20 (2), then 147, 205, 295 on 2026-11-23 with the
/// same driver (3: the second engine only started with the third session).
pub fn engines_from_throughput(totals: &[f64]) -> usize {
    let best = totals.iter().copied().fold(0.0, f64::max);
    totals
        .iter()
        .position(|&total| total >= 0.85 * best)
        .map_or(1, |index| index + 1)
}

/// Measures how many encodes at once `spec` runs best: 1 to [`MAX_PARALLEL_ENCODES`] encodes of a synthetic
/// 1080p60 picture together, each count timed ([`engines_from_throughput`] reads the curve; about 12 s in
/// all, measured once per GPU, driver and encoder). `None` when the encoder fails. NVIDIA does not publish the engine count through FFmpeg or its
/// driver tools, and it varies within a generation (one NVENC on an RTX 3080, two on an RTX 4090), so it is
/// measured rather than looked up.
pub fn measure_parallel_encodes(spec: &EncoderSpec) -> Option<usize> {
    const FRAMES: u32 = 300;
    let mut args: Vec<String> = ["-f", "lavfi", "-i", "testsrc2=s=1920x1080:r=60"]
        .map(String::from)
        .to_vec();
    args.extend(["-frames:v".to_owned(), FRAMES.to_string()]);
    args.extend(spec.constant_qp_args(26.0, 240, super::encoders::Depth::Sdr8));
    args.extend(["-f", "null", "-"].map(String::from));
    let mut totals = Vec::new();
    for count in 1..=MAX_PARALLEL_ENCODES {
        let started = Instant::now();
        let all_ok = std::thread::scope(|scope| {
            let runs: Vec<_> = (0..count)
                .map(|_| {
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    scope.spawn(move || run_test(&args, Duration::from_secs(60)))
                })
                .collect();
            runs.into_iter().all(|r| r.join().unwrap_or(false))
        });
        if !all_ok {
            // The first run failing means the encoder does not work; a later one, a session limit.
            if count == 1 {
                return None;
            }
            break;
        }
        let seconds = started.elapsed().as_secs_f64().max(0.001);
        totals.push(f64::from(FRAMES) * count as f64 / seconds);
    }
    log::info!(
        "{}: {} frames/s for 1 to {} encodes at once",
        spec.name,
        totals
            .iter()
            .map(|t| format!("{t:.0}"))
            .collect::<Vec<_>>()
            .join(", "),
        totals.len()
    );
    Some(engines_from_throughput(&totals))
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
    fn chooses_the_first_codec_with_hardware_then_software() {
        use EncoderBackend::*;
        use VideoCodec::*;
        let caps = |pairs: &[(VideoCodec, EncoderBackend)]| Capabilities {
            encoders: pairs.to_vec(),
            placebo: true,
            detected: true,
        };
        let pick = |caps: &Capabilities, codecs: &[VideoCodec], preferred| {
            caps.choose(codecs, preferred).map(|s| s.name)
        };
        let discord = [Av1, H265, H264];
        // A laptop: AV1 on NVENC, H.265 on Quick Sync, everything on the CPU.
        let laptop = caps(&[
            (Av1, Nvenc),
            (H265, Qsv),
            (H264, Qsv),
            (Av1, Software),
            (H265, Software),
            (H264, Software),
        ]);
        assert_eq!(pick(&laptop, &discord, None), Some("av1_nvenc"));
        assert_eq!(pick(&laptop, &[H265, H264], None), Some("hevc_qsv"));
        // An RTX 3080: no AV1 encoder in hardware, so H.265 on NVENC rather than AV1 on the CPU.
        let rtx3080 = caps(&[
            (H265, Nvenc),
            (H264, Nvenc),
            (Av1, Software),
            (H265, Software),
        ]);
        assert_eq!(pick(&rtx3080, &discord, None), Some("hevc_nvenc"));
        // No hardware at all: the first codec the CPU encodes.
        let cpu = caps(&[(H265, Software), (H264, Software)]);
        assert_eq!(pick(&cpu, &discord, None), Some("libx265"));
        // The user's backend wins when it encodes one of the codecs, else the rule applies.
        assert_eq!(pick(&laptop, &discord, Some(Software)), Some("libsvtav1"));
        assert_eq!(pick(&laptop, &discord, Some(Qsv)), Some("hevc_qsv"));
        assert_eq!(pick(&rtx3080, &discord, Some(Amf)), Some("hevc_nvenc"));
        assert_eq!(pick(&caps(&[]), &discord, None), None);
    }

    #[test]
    fn counts_the_encodes_that_still_add_throughput() {
        // Measured on an RTX 4090: two NVENC engines.
        assert_eq!(engines_from_throughput(&[399.0, 789.0, 852.0, 881.0]), 2);
        // The same card later: the driver put the first two sessions on one engine.
        assert_eq!(engines_from_throughput(&[147.0, 205.0, 295.0, 300.0]), 3);
        // One engine shared: more encodes add a little.
        assert_eq!(engines_from_throughput(&[400.0, 450.0, 460.0, 465.0]), 1);
        // Three engines.
        assert_eq!(engines_from_throughput(&[300.0, 590.0, 870.0, 900.0]), 3);
        // A session limit stopped the measurement after two.
        assert_eq!(engines_from_throughput(&[300.0, 590.0]), 2);
        assert_eq!(engines_from_throughput(&[]), 1);
    }

    #[test]
    fn summarizes_for_the_log() {
        assert_eq!(
            nvidia().summary(),
            "encoders: NVENC H.265, CPU H.265/H.264; GPU tone mapping (libplacebo): yes"
        );
    }
}
