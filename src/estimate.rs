//! The size estimate: which resolution, frame rate and bitrate give the best picture that fits the
//! profile's size limit, found by encoding samples of the video.
//!
//! How it works ([`solve`]):
//! 1. The byte budget gives the video bitrate that fills the limit ([`crate::plan::video_budget`]).
//! 2. Samples spread over the trimmed range (one keyframe interval each, so they carry the same share of
//!    keyframes as the encode) are encoded at a constant QP: their size measures how hard the picture is.
//!    The bitrate of a QP follows `bitrate(qp) = r0 * 2^(-(qp - q0) / k)` (it halves every `k` QP, about
//!    6 for H.265); a second measurement near the answer fits `k` for this video.
//! 3. The QP that spends exactly the budget is the predicted quality. Better than the profile's ceiling:
//!    the encode targets the ceiling's bitrate and the file ends up smaller than the limit. Worse than its
//!    floor: the next target of the ladder (a lower frame rate, then a smaller picture) is tried.
//!
//! The encode then runs at the chosen bitrate ([`crate::encode`]), and checks the result.

use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, bail};

use crate::ffmpeg::encoders::{EncoderSpec, VideoCodec};
use crate::ffmpeg::filters::{self, Engine, GraphSpec, OutputColor, Sink};
use crate::ffmpeg::probe::MediaInfo;
use crate::ffmpeg::{StderrTail, background_tool, base_args, kill_tree};
use crate::plan::{self, Target};
use crate::profile::Profile;
use crate::queue::Slots;
use crate::timecode;

/// How the bitrate of a video changes with the QP.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateModel {
    /// Reference QP.
    pub q0: f64,
    /// Bitrate at `q0`, in bit/s.
    pub r0: f64,
    /// QP steps that halve the bitrate.
    pub k: f64,
}

impl RateModel {
    /// The usual slope of H.264/H.265 encoders, used until a second measurement fits this video's.
    pub const DEFAULT_K: f64 = 6.0;

    /// The bitrate at `qp`.
    pub fn bitrate(&self, qp: f64) -> f64 {
        self.r0 * 2f64.powf(-(qp - self.q0) / self.k)
    }

    /// The QP whose bitrate is `bps`.
    pub fn qp_for(&self, bps: f64) -> f64 {
        self.q0 + self.k * (self.r0 / bps.max(1.0)).log2()
    }

    /// The model through two measurements, its slope kept between 3 and 12 QP per halving.
    pub fn through(a: (f64, f64), b: (f64, f64)) -> Self {
        let ratio = (a.1 / b.1).log2();
        let k = if (a.0 - b.0).abs() < 0.5 || !ratio.is_finite() || ratio.abs() < 1e-6 {
            Self::DEFAULT_K
        } else {
            ((b.0 - a.0) / ratio).clamp(3.0, 12.0)
        };
        Self {
            q0: a.0,
            r0: a.1,
            k,
        }
    }
}

/// How the estimate placed the video against the profile's quality bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The ceiling's quality fits: the file will be smaller than the limit.
    BestQuality,
    /// The limit decides: the file will fill it, at a quality between the ceiling and the floor.
    Fitted,
    /// Even the smallest target falls below the floor: shorten the clip, or accept a soft picture.
    BelowFloor,
}

/// The result of an estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    /// The resolution and frame rate to encode.
    pub target: Target,
    /// Video bitrate to encode at, in bit/s.
    pub video_bps: f64,
    /// Predicted quality, as an H.265 QP.
    pub quality: f64,
    /// Predicted output size in bytes.
    pub predicted_bytes: u64,
    /// Where the video stands against the profile.
    pub verdict: Verdict,
    /// Constant QP encodes measured.
    pub measurements: usize,
}

/// A word for an H.265 QP, for people who do not think in QPs.
pub fn quality_word(qp: f64) -> String {
    match qp {
        q if q < 22.0 => crate::tr!("estimate-quality-excellent"),
        q if q < 26.0 => crate::tr!("estimate-quality-very-good"),
        q if q < 30.0 => crate::tr!("estimate-quality-good"),
        q if q < 34.0 => crate::tr!("estimate-quality-fair"),
        _ => crate::tr!("estimate-quality-poor"),
    }
}

/// Pixel rate exponent of the bitrate, to predict a smaller target from a measured one (bitrates grow
/// slower than pixel counts: larger pictures are smoother at the pixel level).
const PIXEL_RATE_EXPONENT: f64 = 0.75;

/// The bounds and budget [`solve`] works with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Video bitrate that fills the limit.
    pub budget_bps: f64,
    /// Best quality worth spending on (H.265 QP).
    pub ceiling: f64,
    /// Worst acceptable quality (H.265 QP).
    pub floor: f64,
}

/// Finds the best target of `ladder` and its bitrate. `measure(target, qp)` encodes the samples at a
/// constant QP and returns their bitrate in bit/s.
///
/// The top target is measured first. When it does not fit, the next candidate is the best target
/// predicted to fit, from the bitrate's growth with the pixel rate — first `PIXEL_RATE_EXPONENT`, then
/// the exponent fitted on this video's own measurements (noise-like pictures grow almost linearly). A
/// candidate that fits with room to spare makes the search go back up between it and the last failure, so
/// the answer is the best target that fits, not merely one that does.
pub fn solve(
    ladder: &[Target],
    bounds: Bounds,
    mut measure: impl FnMut(Target, f64) -> Result<f64>,
) -> Result<(Target, f64, f64, Verdict, usize)> {
    if ladder.is_empty() {
        bail!(crate::tr!("estimate-no-size"));
    }
    let start_qp = ((bounds.ceiling + bounds.floor) / 2.0).round();
    let mut measurements = 0;
    // The rate model of each measured target.
    let mut models: Vec<Option<RateModel>> = vec![None; ladder.len()];
    let mut model_of = |index: usize, models: &mut Vec<Option<RateModel>>| -> Result<RateModel> {
        if let Some(model) = models[index] {
            return Ok(model);
        }
        let target = ladder[index];
        measurements += 1;
        let a = (start_qp, measure(target, start_qp)?.max(1.0));
        let rough = RateModel::through(a, a);
        let guess = rough
            .qp_for(bounds.budget_bps)
            .clamp(bounds.ceiling, 51.0)
            .round();
        let model = if (guess - a.0).abs() >= 2.0 {
            measurements += 1;
            let b = (guess, measure(target, guess)?.max(1.0));
            RateModel::through(a, b)
        } else {
            rough
        };
        models[index] = Some(model);
        Ok(model)
    };
    let needed = |model: RateModel| model.qp_for(bounds.budget_bps);
    let top = model_of(0, &mut models)?;
    // Targets below `low` are known not to fit; `fit` is the best target known to fit.
    let mut low = 1;
    let mut fit: Option<usize> = (needed(top) <= bounds.floor).then_some(0);
    while fit != Some(low - 1) && low < ladder.len() {
        let upper = fit.unwrap_or(ladder.len());
        let exponent = fitted_exponent(ladder, &models);
        let predicted = |index: usize| {
            let scale = (ladder[index].pixel_rate() / ladder[0].pixel_rate()).powf(exponent);
            needed(RateModel {
                r0: top.r0 * scale,
                ..top
            })
        };
        let candidate = (low..upper)
            .find(|&index| predicted(index) <= bounds.floor)
            .unwrap_or(upper.min(ladder.len() - 1));
        if Some(candidate) == fit {
            break;
        }
        let model = model_of(candidate, &mut models)?;
        if needed(model) <= bounds.floor {
            fit = Some(candidate);
        } else {
            low = candidate + 1;
        }
    }
    let index = fit.unwrap_or(ladder.len() - 1);
    let model = model_of(index, &mut models)?;
    let quality = needed(model);
    let target = ladder[index];
    Ok(if quality <= bounds.ceiling {
        (
            target,
            model.bitrate(bounds.ceiling),
            bounds.ceiling,
            Verdict::BestQuality,
            measurements,
        )
    } else if quality <= bounds.floor {
        (
            target,
            bounds.budget_bps,
            quality,
            Verdict::Fitted,
            measurements,
        )
    } else {
        (
            target,
            bounds.budget_bps,
            quality,
            Verdict::BelowFloor,
            measurements,
        )
    })
}

/// The exponent of the bitrate's growth with the pixel rate, fitted between the top target and the
/// smallest measured one, or [`PIXEL_RATE_EXPONENT`] while only the top is measured.
fn fitted_exponent(ladder: &[Target], models: &[Option<RateModel>]) -> f64 {
    let Some(top) = models[0] else {
        return PIXEL_RATE_EXPONENT;
    };
    let Some((index, model)) = models
        .iter()
        .enumerate()
        .skip(1)
        .rev()
        .find_map(|(i, m)| m.map(|m| (i, m)))
    else {
        return PIXEL_RATE_EXPONENT;
    };
    let pixels = (ladder[index].pixel_rate() / ladder[0].pixel_rate()).ln();
    let bitrate = (model.bitrate(top.q0) / top.r0).ln();
    if pixels.abs() < 1e-6 || !bitrate.is_finite() {
        return PIXEL_RATE_EXPONENT;
    }
    (bitrate / pixels).clamp(0.4, 1.3)
}

/// Where the samples start, in source seconds, and how long each lasts: `count` intervals of
/// `length` spread evenly over `[start, end)`, or the whole range when it is barely longer.
pub fn sample_positions(start: f64, end: f64, length: f64) -> Vec<(f64, f64)> {
    let duration = (end - start).max(0.0);
    let count = (duration / 20.0).ceil().clamp(2.0, 6.0) as usize;
    if duration <= length * count as f64 * 1.5 {
        return vec![(start, duration)];
    }
    let spacing = duration / count as f64;
    (0..count)
        .map(|i| {
            let center = start + spacing * (i as f64 + 0.5);
            ((center - length / 2.0).clamp(start, end - length), length)
        })
        .collect()
}

/// Everything an estimate needs about one queued video.
#[derive(Clone, Debug)]
pub struct EstimateRequest {
    /// Source file.
    pub path: PathBuf,
    /// Its media information.
    pub media: MediaInfo,
    /// The video's own profile.
    pub profile: Profile,
    /// Trimmed range in source seconds.
    pub start: f64,
    /// End of the trimmed range in source seconds.
    pub end: f64,
    /// How the filters run.
    pub engine: Engine,
    /// The encoder.
    pub encoder: &'static EncoderSpec,
}

impl EstimateRequest {
    /// Duration of the output in seconds.
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }

    /// The output's color.
    pub fn color(&self) -> OutputColor {
        OutputColor::choose(
            &self.media,
            self.profile.sdr_only,
            self.encoder.codec.supports_hdr(),
        )
    }
}

/// Runs an estimate: samples encoded through `slots` (each holds one encoder session), stopped early
/// when `cancel` is set.
pub fn run(request: &EstimateRequest, slots: &Slots, cancel: &Arc<AtomicBool>) -> Result<Estimate> {
    let duration = request.duration();
    if duration < 0.2 {
        bail!(crate::tr!("estimate-empty-range"));
    }
    let audio = plan::audio_kbps(&request.media, &request.profile);
    let Some(budget_bps) = plan::video_budget(request.profile.max_bytes(), duration, audio) else {
        bail!(crate::tr!(
            "estimate-too-small",
            mb = request.profile.max_size_mb,
            duration = timecode::format_short(duration, false)
        ));
    };
    let ladder = plan::ladder(
        request.media.width,
        request.media.height,
        request.media.frame_rate,
        &request.profile,
    );
    let bounds = Bounds {
        budget_bps,
        ceiling: f64::from(request.profile.quality_ceiling),
        floor: f64::from(request.profile.quality_floor),
    };
    let (target, video_bps, quality, verdict, measurements) =
        solve(&ladder, bounds, |target, qp| {
            measure(request, target, qp, slots, cancel)
        })?;
    Ok(Estimate {
        target,
        video_bps,
        quality,
        predicted_bytes: plan::predicted_bytes(video_bps, duration, audio),
        verdict,
        measurements,
    })
}

/// The raw elementary stream format of a codec, so a sample's size is its bytes on standard output.
fn elementary_format(codec: VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H265 => "hevc",
        VideoCodec::H264 => "h264",
        VideoCodec::Av1 => "ivf",
    }
}

/// Encodes every sample of `target` at `qp` in parallel and returns their bitrate in bit/s.
fn measure(
    request: &EstimateRequest,
    target: Target,
    qp: f64,
    slots: &Slots,
    cancel: &Arc<AtomicBool>,
) -> Result<f64> {
    let length = plan::sample_seconds(target.rate);
    let samples = sample_positions(request.start, request.end, length);
    let results: Vec<Result<u64>> = std::thread::scope(|scope| {
        let handles: Vec<_> = samples
            .iter()
            .map(|&(start, length)| {
                scope.spawn(move || {
                    let _slot = slots.acquire();
                    if cancel.load(Ordering::Relaxed) {
                        bail!("cancelled");
                    }
                    encode_sample(request, target, qp, start, length, cancel)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("sample thread panicked")))
            })
            .collect()
    });
    let mut bytes = 0;
    for result in results {
        bytes += result?;
    }
    let seconds: f64 = samples.iter().map(|(_, length)| length).sum();
    log::debug!(
        "estimate {}: {} at QP {qp}: {:.0} kbit/s over {} samples",
        request.path.display(),
        target.label(),
        bytes as f64 * 8.0 / seconds / 1000.0,
        samples.len()
    );
    Ok(bytes as f64 * 8.0 / seconds.max(0.01))
}

/// The `ffmpeg` arguments of one sample, writing the elementary stream to standard output.
pub fn sample_args(
    request: &EstimateRequest,
    target: Target,
    qp: f64,
    start: f64,
    length: f64,
) -> Vec<String> {
    let graph = filters::graph(
        request.engine,
        &request.media,
        GraphSpec {
            size: target.size,
            rate: Some(target.rate),
            tone: request.profile.tone_mapping,
            sink: Sink::Encoder(request.color()),
        },
    );
    let mut args = base_args();
    args.extend(graph.input_args);
    args.extend([
        "-ss".to_owned(),
        timecode::arg(start),
        "-t".to_owned(),
        timecode::arg(length),
    ]);
    args.extend(["-i".to_owned(), request.path.to_string_lossy().into_owned()]);
    args.extend(["-map", "0:v:0", "-an", "-sn", "-dn"].map(String::from));
    args.extend(["-vf".to_owned(), graph.filter]);
    args.extend(request.encoder.constant_qp_args(
        qp,
        plan::gop(target.rate),
        request.color().depth(),
    ));
    args.extend([
        "-f".to_owned(),
        elementary_format(request.encoder.codec).to_owned(),
        "pipe:1".to_owned(),
    ]);
    args
}

fn encode_sample(
    request: &EstimateRequest,
    target: Target,
    qp: f64,
    start: f64,
    length: f64,
    cancel: &Arc<AtomicBool>,
) -> Result<u64> {
    let args = sample_args(request, target, qp, start, length);
    let mut child = background_tool("ffmpeg")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| crate::tr!("media-ffmpeg-start-failed"))?;
    let mut stderr = StderrTail::collect(&mut child);
    let mut stdout = child.stdout.take().context("no ffmpeg stdout")?;
    let mut buffer = vec![0u8; 1 << 16];
    let mut bytes = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            kill_tree(&mut child);
            let _ = child.wait();
            bail!("cancelled");
        }
        match stdout.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => bytes += read as u64,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error).context("cannot read the sample"),
        }
    }
    let status = child.wait().context("ffmpeg did not exit")?;
    if !status.success() || bytes == 0 {
        let detail = stderr.finish();
        let status = status.to_string();
        bail!(if detail.is_empty() {
            crate::tr!("estimate-sample-failed-silent", status = status)
        } else {
            crate::tr!("estimate-sample-failed", status = status, detail = detail)
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{FrameRate, Size};

    fn target(short: u32, fps: u32) -> Target {
        Target {
            size: Size {
                width: short * 16 / 9,
                height: short,
            },
            rate: FrameRate::new(fps, 1),
        }
    }

    /// A fake video whose bitrate at QP 20 for 1080p60 is `r20`, halving every 6 QP, scaling with the
    /// pixel rate like the solver assumes.
    fn video(r20: f64) -> impl FnMut(Target, f64) -> Result<f64> {
        move |t: Target, qp: f64| {
            let scale = (t.pixel_rate() / target(1080, 60).pixel_rate()).powf(0.75);
            Ok(r20 * scale * 2f64.powf(-(qp - 20.0) / 6.0))
        }
    }

    const BOUNDS: Bounds = Bounds {
        budget_bps: 3_000_000.0,
        ceiling: 20.0,
        floor: 32.0,
    };

    #[test]
    fn the_model_inverts_and_fits_two_points() {
        let model = RateModel::through((20.0, 8e6), (26.0, 4e6));
        assert!((model.k - 6.0).abs() < 1e-9);
        assert!((model.bitrate(32.0) - 2e6).abs() < 1.0);
        assert!((model.qp_for(2e6) - 32.0).abs() < 1e-9);
        // A degenerate pair keeps the default slope; an absurd one is clamped.
        assert_eq!(RateModel::through((20.0, 8e6), (20.0, 8e6)).k, 6.0);
        assert_eq!(RateModel::through((20.0, 8e6), (21.0, 1e6)).k, 3.0);
    }

    #[test]
    fn an_easy_clip_stops_at_the_ceiling() {
        let ladder = [target(1080, 60), target(720, 30)];
        let (t, bps, q, verdict, _) = solve(&ladder, BOUNDS, video(2e6)).unwrap();
        assert_eq!((t, verdict, q), (ladder[0], Verdict::BestQuality, 20.0));
        assert!((bps - 2e6).abs() < 1e3, "{bps}");
    }

    #[test]
    fn a_hard_clip_fills_the_budget() {
        let ladder = [target(720, 30)];
        // 720p30 is 0.32x the bitrate of 1080p60: 9.7 Mbit/s at QP 20, so 3 Mbit/s at QP 30.2.
        let (t, bps, q, verdict, measurements) = solve(&ladder, BOUNDS, video(30e6)).unwrap();
        assert_eq!((t, verdict, bps), (ladder[0], Verdict::Fitted, 3e6));
        assert!((q - 30.16).abs() < 0.05, "{q}");
        assert_eq!(measurements, 2);
    }

    #[test]
    fn a_very_hard_clip_steps_down_the_ladder() {
        // QP 37.9 at 1080p60, 33.9 at 1080p30 (both below the floor), 28.6 at 720p30.
        let ladder = [target(1080, 60), target(1080, 30), target(720, 30)];
        let (t, _, q, verdict, _) = solve(&ladder, BOUNDS, video(25e6)).unwrap();
        assert_eq!((t, verdict), (ladder[2], Verdict::Fitted));
        assert!((q - 28.6).abs() < 0.1, "{q}");
    }

    #[test]
    fn a_noise_like_clip_goes_back_up_to_the_best_target_that_fits() {
        // Bitrate proportional to the pixel rate: QP 36.4 at 720p, 31.4 at 540p, 29.4 at 480p. The
        // default exponent first points at 480p; the fitted one finds that 540p fits too.
        let ladder = [
            target(720, 30),
            target(540, 30),
            target(480, 30),
            target(360, 30),
        ];
        let video = |t: Target, qp: f64| {
            let scale = t.pixel_rate() / ladder[0].pixel_rate();
            Ok(20e6 * scale * 2f64.powf(-(qp - 20.0) / 6.0))
        };
        let (t, _, q, verdict, _) = solve(&ladder, BOUNDS, video).unwrap();
        assert_eq!((t, verdict), (ladder[1], Verdict::Fitted));
        assert!((q - 31.6).abs() < 0.3, "{q}");
    }

    #[test]
    fn an_impossible_clip_takes_the_smallest_target() {
        let ladder = [target(720, 30), target(360, 30)];
        let (t, bps, _, verdict, _) = solve(&ladder, BOUNDS, video(2e9)).unwrap();
        assert_eq!((t, verdict, bps), (ladder[1], Verdict::BelowFloor, 3e6));
    }

    #[test]
    fn samples_cover_the_range_or_take_it_whole() {
        assert_eq!(sample_positions(10.0, 18.0, 4.0), [(10.0, 8.0)]);
        let samples = sample_positions(0.0, 120.0, 4.0);
        assert_eq!(samples.len(), 6);
        assert_eq!(samples[0], (8.0, 4.0));
        assert!(samples.iter().all(|(s, l)| *s >= 0.0 && s + l <= 120.0));
    }

    #[test]
    fn describes_qualities_in_words() {
        assert_eq!(quality_word(20.0), "excellent");
        assert_eq!(quality_word(27.5), "good");
        assert_eq!(quality_word(40.0), "poor");
    }
}
