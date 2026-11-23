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

/// Most encodes the measurement tries at once.
pub const MAX_PARALLEL_ENCODES: usize = 4;

/// How many encodes at once still add throughput, from the total frames per second measured with 1, 2,
/// 3… encodes running together (`totals[n - 1]` for `n` encodes). One more encode counts when it adds at
/// least half of what one encode does alone: a second hardware engine roughly doubles the total, while
/// sharing one engine adds a few percent. An RTX 4090 (two NVENC engines) measured 399, 789, 852 and 881
/// frames per second for 1 to 4 encodes: 2.
pub fn engines_from_throughput(totals: &[f64]) -> usize {
    let Some(&alone) = totals.first() else {
        return 1;
    };
    let mut engines = 1;
    for pair in totals.windows(2) {
        if pair[1] >= pair[0] + 0.5 * alone {
            engines += 1;
        } else {
            break;
        }
    }
    engines
}

/// Measures how many encodes at once `spec` runs without them slowing each other down: 1, 2, 3…
/// encodes of a synthetic 1080p60 picture together, timed, until one more adds little (a few seconds in
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
            return (count > 1).then(|| engines_from_throughput(&totals));
        }
        let seconds = started.elapsed().as_secs_f64().max(0.001);
        totals.push(f64::from(FRAMES) * count as f64 / seconds);
        if totals.len() >= 2 && engines_from_throughput(&totals) < totals.len() {
            break;
        }
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
    fn counts_the_encodes_that_still_add_throughput() {
        // Measured on an RTX 4090: two NVENC engines.
        assert_eq!(engines_from_throughput(&[399.0, 789.0, 852.0, 881.0]), 2);
        // One engine shared: a second encode adds a little.
        assert_eq!(engines_from_throughput(&[400.0, 450.0]), 1);
        // Three engines.
        assert_eq!(engines_from_throughput(&[300.0, 590.0, 870.0, 900.0]), 3);
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
