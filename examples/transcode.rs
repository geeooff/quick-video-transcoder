//! Estimates (and with an output folder, encodes) one video with the built-in Messenger profile, without
//! the UI: for measuring the size fitting on real captures.
//!
//! ```text
//! cargo run --release --example transcode -- <video> [output folder or -] [size limit in MB or -] [built-in profile name]
//! ```
//!
//! The video is only read. The output folder must already exist and differ from the video's folder, so a
//! measurement never writes next to a capture.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use quick_video_transcoder::encode;
use quick_video_transcoder::estimate::{self, EstimateRequest, quality_word};
use quick_video_transcoder::ffmpeg::filters::Engine;
use quick_video_transcoder::ffmpeg::hardware::{self, Capabilities};
use quick_video_transcoder::ffmpeg::probe::probe;
use quick_video_transcoder::naming;
use quick_video_transcoder::profile::Profile;
use quick_video_transcoder::queue::Slots;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut args = std::env::args_os().skip(1);
    let video = PathBuf::from(
        args.next()
            .context("usage: transcode <video> [output folder or -] [MB or -] [profile]")?,
    );
    let out_dir = args
        .next()
        .map(PathBuf::from)
        .filter(|p| p.as_os_str() != "-");
    let limit: Option<f64> = args.next().and_then(|a| a.to_string_lossy().parse().ok());
    let profile_name = args.next().map(|a| a.to_string_lossy().into_owned());

    let capabilities = Capabilities::detect();
    println!("{}", capabilities.summary());
    let media = probe(&video)?;
    println!(
        "source: {}, {:.2} s, color {}",
        media.summary(),
        media.duration,
        media.color_tags
    );
    let engine = Engine {
        placebo: capabilities.placebo,
        vulkan_decode: capabilities.placebo && hardware::vulkan_decodes(&video),
    };
    let mut profile = match &profile_name {
        Some(name) => Profile::built_ins()
            .into_iter()
            .find(|p| p.name == *name)
            .with_context(|| format!("no built-in profile named {name}"))?,
        None => Profile::messenger(),
    };
    if let Some(limit) = limit {
        profile.max_size_mb = limit;
    }
    let request = EstimateRequest {
        path: video.clone(),
        media: media.clone(),
        profile: profile.clone(),
        start: 0.0,
        end: media.duration,
        engine,
        encoder: capabilities
            .choose(&profile.codecs, None)
            .context("no H.265 encoder")?,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let estimate = estimate::run(&request, &Slots::new(4), &cancel)?;
    println!(
        "estimate ({}, {}, {:.1} s, {} measurements): {} at {:.0} kbit/s, QP {:.1} ({}), {:?}, predicted {} bytes",
        request.encoder.name,
        engine.label(),
        started.elapsed().as_secs_f64(),
        estimate.measurements,
        estimate.target.label(),
        estimate.video_bps / 1000.0,
        estimate.quality,
        quality_word(estimate.quality),
        estimate.verdict,
        estimate.predicted_bytes
    );
    let Some(out_dir) = out_dir else {
        return Ok(());
    };
    let out_dir = out_dir
        .canonicalize()
        .context("the output folder must exist")?;
    let source_dir = video.canonicalize()?.parent().map(PathBuf::from);
    if source_dir.as_deref() == Some(out_dir.as_path()) {
        bail!("the output folder must not be the video's folder");
    }
    let name = naming::output_name(&video, &profile.suffix);
    let output = naming::unique_output(&out_dir, &name, &video, &[]);
    assert!(output.starts_with(&out_dir));
    let encoded = encode::run(&request, &estimate, &output, &cancel, |_| {})?;
    println!(
        "encoded {} bytes ({:.1} % of the limit) in {:.1} s, {} attempt(s): {}",
        encoded.bytes,
        encoded.bytes as f64 / profile.max_bytes() as f64 * 100.0,
        encoded.elapsed.as_secs_f64(),
        encoded.attempts,
        output.display()
    );
    Ok(())
}
