//! The encode of one video: the bitrate the estimate chose, the filters of the preview, then a check of
//! the file's size, re-encoding a little lower in the rare case the encoder overshot.
//!
//! FFmpeg writes a partial file ([`crate::naming::partial_path`]) renamed to the output only once the size
//! is checked; a failed or cancelled encode deletes it. Metadata of the source (titles, encoder tags,
//! chapters) is not copied: what is shared is the picture and the sound.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::estimate::{Estimate, EstimateRequest};
use crate::ffmpeg::filters::{self, GraphSpec, Sink};
use crate::ffmpeg::{StderrTail, base_args, kill_tree, tool};
use crate::naming::{check_partial, partial_path};
use crate::plan;
use crate::timecode;

/// Re-encodes allowed when the file comes out larger than the limit.
const RETRIES: usize = 2;

/// What an encode reports while it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    /// Fraction of the current attempt done, 0 to 1.
    pub fraction: f32,
    /// Frames encoded per second.
    pub fps: Option<f64>,
    /// Encoded media time per second of work.
    pub speed: Option<f64>,
    /// Attempt number, 1 for the first.
    pub attempt: usize,
}

/// A finished encode.
#[derive(Clone, Debug, PartialEq)]
pub struct Encoded {
    /// The output file.
    pub output: PathBuf,
    /// Its size in bytes.
    pub bytes: u64,
    /// Attempts it took.
    pub attempts: usize,
    /// Wall-clock time.
    pub elapsed: Duration,
}

/// The `ffmpeg` arguments writing `partial` at `video_bps`.
pub fn encode_args(
    request: &EstimateRequest,
    estimate: &Estimate,
    video_bps: f64,
    partial: &Path,
) -> Vec<String> {
    let target = estimate.target;
    let color = request.color();
    let graph = filters::graph(
        request.engine,
        &request.media,
        GraphSpec {
            size: target.size,
            rate: Some(target.rate),
            tone: request.profile.tone_mapping,
            sink: Sink::Encoder(color),
        },
    );
    let mut args = base_args();
    args.extend(["-nostats", "-progress", "pipe:1", "-y"].map(String::from));
    args.extend(graph.input_args);
    args.extend([
        "-ss".to_owned(),
        timecode::arg(request.start),
        "-t".to_owned(),
        timecode::arg(request.duration()),
        "-i".to_owned(),
        request.path.to_string_lossy().into_owned(),
    ]);
    args.extend([
        "-map".to_owned(),
        "0:v:0".to_owned(),
        "-vf".to_owned(),
        graph.filter,
    ]);
    let audio = plan::audio_kbps(&request.media, &request.profile);
    args.extend(crate::ffmpeg::audio::map_args(
        &request.profile.audio.tracks(&request.media.audio),
    ));
    args.extend(
        request
            .encoder
            .bitrate_args(video_bps, plan::gop(target.rate), color.depth()),
    );
    args.extend(color.tag_args());
    if let Some(kbps) = audio {
        args.extend([
            "-c:a".to_owned(),
            "aac".to_owned(),
            "-b:a".to_owned(),
            format!("{kbps}k"),
            "-ac".to_owned(),
            "2".to_owned(),
            "-ar".to_owned(),
            "48000".to_owned(),
        ]);
    }
    args.extend(
        [
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ]
        .map(String::from),
    );
    args.push(partial.to_string_lossy().into_owned());
    args
}

/// Parses a `key=value` line of FFmpeg's `-progress` report.
fn parse_progress(line: &str) -> Option<(&str, f64)> {
    let (key, value) = line.split_once('=')?;
    let number = |text: &str| {
        text.trim()
            .trim_end_matches('x')
            .parse::<f64>()
            .ok()
            .filter(|v| *v > 0.0)
    };
    match key {
        // `out_time_us` (and the historical misnomer `out_time_ms`) are in microseconds.
        "out_time_us" | "out_time_ms" => number(value).map(|us| ("time", us / 1e6)),
        "fps" => number(value).map(|v| ("fps", v)),
        "speed" => number(value).map(|v| ("speed", v)),
        _ => None,
    }
}

/// Encodes `request` to `output` at the estimate's bitrate, reporting progress through `report`. The
/// output must not exist (see [`crate::naming::unique_output`]).
pub fn run(
    request: &EstimateRequest,
    estimate: &Estimate,
    output: &Path,
    cancel: &Arc<AtomicBool>,
    mut report: impl FnMut(Progress),
) -> Result<Encoded> {
    let started = Instant::now();
    let partial = partial_path(output);
    check_partial(&partial, output, &request.path)?;
    if output.exists() {
        bail!("{} already exists", output.display());
    }
    let max_bytes = request.profile.max_bytes();
    let duration = request.duration();
    let mut video_bps = estimate.video_bps;
    for attempt in 1..=RETRIES + 1 {
        let result = run_once(
            request,
            estimate,
            video_bps,
            &partial,
            cancel,
            |done, fps, speed| {
                report(Progress {
                    fraction: (done / duration.max(0.01)).clamp(0.0, 1.0) as f32,
                    fps,
                    speed,
                    attempt,
                });
            },
        );
        if let Err(error) = result {
            let _ = remove_partial(&partial, output, &request.path);
            return Err(error);
        }
        let bytes = std::fs::metadata(&partial)
            .with_context(|| format!("cannot read {}", partial.display()))?
            .len();
        if bytes <= max_bytes {
            std::fs::rename(&partial, output)
                .with_context(|| format!("cannot rename the encode to {}", output.display()))?;
            log::info!(
                "encoded {} -> {} ({bytes} bytes, attempt {attempt})",
                request.path.display(),
                output.display()
            );
            return Ok(Encoded {
                output: output.to_path_buf(),
                bytes,
                attempts: attempt,
                elapsed: started.elapsed(),
            });
        }
        // The stream sizes scale with the video bitrate; aim 3 % under the limit.
        let overshoot = bytes as f64 / (max_bytes as f64 * 0.97);
        log::warn!(
            "{} came out at {bytes} bytes, over {max_bytes}: re-encoding at {:.0} kbit/s",
            output.display(),
            video_bps / overshoot / 1000.0
        );
        video_bps /= overshoot;
    }
    let _ = remove_partial(&partial, output, &request.path);
    bail!(
        "the encoder overshot {:.1} MB {} times: lower the quality floor or shorten the clip",
        request.profile.max_size_mb,
        RETRIES + 1
    )
}

fn remove_partial(partial: &Path, output: &Path, source: &Path) -> Result<()> {
    check_partial(partial, output, source)?;
    match std::fs::remove_file(partial) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

fn run_once(
    request: &EstimateRequest,
    estimate: &Estimate,
    video_bps: f64,
    partial: &Path,
    cancel: &Arc<AtomicBool>,
    mut report: impl FnMut(f64, Option<f64>, Option<f64>),
) -> Result<()> {
    let args = encode_args(request, estimate, video_bps, partial);
    log::info!("encode: ffmpeg {}", args.join(" "));
    let mut child = tool("ffmpeg")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("cannot start ffmpeg (is FFmpeg on the PATH?)")?;
    let mut stderr = StderrTail::collect(&mut child);
    let stdout = child.stdout.take().context("no ffmpeg stdout")?;
    let (tx, rx) = crossbeam_channel::unbounded();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(value) = parse_progress(&line)
                && tx.send((value.0.to_owned(), value.1)).is_err()
            {
                return;
            }
        }
    });
    let (mut done, mut fps, mut speed) = (0.0, None, None);
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            kill_tree(&mut child);
            let _ = child.wait();
            let _ = reader.join();
            bail!("cancelled");
        }
        let mut changed = false;
        while let Ok((key, value)) = rx.try_recv() {
            changed = true;
            match key.as_str() {
                "time" => done = f64::max(done, value),
                "fps" => fps = Some(value),
                _ => speed = Some(value),
            }
        }
        if changed {
            report(done, fps, speed);
        }
        if let Some(status) = child.try_wait().context("ffmpeg did not exit")? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = reader.join();
    if !status.success() {
        let detail = stderr.finish();
        bail!(
            "ffmpeg failed ({status}): {}",
            if detail.is_empty() {
                "no message"
            } else {
                &detail
            }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estimate::Verdict;
    use crate::ffmpeg::encoders::{self, EncoderBackend, VideoCodec};
    use crate::ffmpeg::filters::Engine;
    use crate::ffmpeg::probe::parse_probe_json;
    use crate::plan::{FrameRate, Size, Target};
    use crate::profile::AudioChoice;
    use crate::profile::Profile;

    fn request(audio: AudioChoice) -> EstimateRequest {
        let media = parse_probe_json(
            r#"{"streams": [
                {"codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
                 "r_frame_rate": "60/1", "duration": "60"},
                {"codec_type": "audio", "codec_name": "aac", "channels": 2},
                {"codec_type": "audio", "codec_name": "aac", "channels": 1}]}"#,
        )
        .unwrap();
        EstimateRequest {
            path: PathBuf::from("in.mp4"),
            media,
            profile: Profile {
                audio,
                ..Profile::messenger()
            },
            start: 5.0,
            end: 25.0,
            engine: Engine::SOFTWARE,
            encoder: encoders::find(VideoCodec::H265, EncoderBackend::Nvenc),
        }
    }

    const ESTIMATE: Estimate = Estimate {
        target: Target {
            size: Size {
                width: 1280,
                height: 720,
            },
            rate: FrameRate::new(30, 1),
        },
        video_bps: 3e6,
        quality: 25.0,
        predicted_bytes: 0,
        verdict: Verdict::Fitted,
        measurements: 2,
    };

    #[test]
    fn writes_the_trimmed_range_with_tags_and_without_metadata() {
        let args = encode_args(
            &request(AudioChoice::Auto),
            &ESTIMATE,
            3e6,
            Path::new("o.mp4.qvt-partial"),
        )
        .join(" ");
        assert!(
            args.contains("-ss 5.000 -t 20.000 -i in.mp4 -map 0:v:0 -vf fps=30,setparams="),
            "{args}"
        );
        assert!(args.contains(",scale=1280:720:flags=lanczos"), "{args}");
        assert!(args.contains("-map 0:a:0 -c:v hevc_nvenc"), "{args}");
        assert!(args.contains("-b:v 3000000"), "{args}");
        assert!(args.contains("-colorspace:v bt709"), "{args}");
        assert!(
            args.contains("-c:a aac -b:a 128k -ac 2 -ar 48000"),
            "{args}"
        );
        assert!(
            args.ends_with(
                "-map_metadata -1 -map_chapters -1 -movflags +faststart -f mp4 o.mp4.qvt-partial"
            ),
            "{args}"
        );
    }

    #[test]
    fn mixes_or_drops_the_sound_on_request() {
        let mix = encode_args(
            &request(AudioChoice::MixAll),
            &ESTIMATE,
            3e6,
            Path::new("o"),
        )
        .join(" ");
        assert!(
            mix.contains("-filter_complex [0:a:0][0:a:1]amix=inputs=2"),
            "{mix}"
        );
        assert!(mix.contains("-map [mix]"), "{mix}");
        let second = encode_args(
            &request(AudioChoice::Track(1)),
            &ESTIMATE,
            3e6,
            Path::new("o"),
        )
        .join(" ");
        assert!(second.contains("-map 0:a:1"), "{second}");
        let none =
            encode_args(&request(AudioChoice::None), &ESTIMATE, 3e6, Path::new("o")).join(" ");
        assert!(none.contains(" -an ") && !none.contains("-c:a"), "{none}");
        // A track that does not exist means no sound rather than a failed encode.
        let missing = encode_args(
            &request(AudioChoice::Track(5)),
            &ESTIMATE,
            3e6,
            Path::new("o"),
        )
        .join(" ");
        assert!(missing.contains(" -an "), "{missing}");
    }

    #[test]
    fn parses_the_progress_report() {
        assert_eq!(parse_progress("out_time_us=2050000"), Some(("time", 2.05)));
        assert_eq!(parse_progress("fps=87.45"), Some(("fps", 87.45)));
        assert_eq!(parse_progress("speed=1.46x"), Some(("speed", 1.46)));
        assert_eq!(parse_progress("speed=N/A"), None);
        assert_eq!(parse_progress("progress=continue"), None);
    }
}
