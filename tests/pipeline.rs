//! End to end: synthetic clips (never real captures) estimated and encoded with every engine that works
//! on this machine, checked against the size limit and probed back. Skipped when FFmpeg is missing.
//!
//! Every file is written inside a fresh temporary folder; the encode refuses any other path.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use quick_video_transcoder::encode;
use quick_video_transcoder::estimate::{self, EstimateRequest};
use quick_video_transcoder::ffmpeg::filters::Engine;
use quick_video_transcoder::ffmpeg::hardware::{self, Capabilities};
use quick_video_transcoder::ffmpeg::probe::{Dynamic, probe};
use quick_video_transcoder::naming;
use quick_video_transcoder::plan::FrameRate;
use quick_video_transcoder::profile::{AudioChoice, Profile};
use quick_video_transcoder::queue::Slots;

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Writes a busy synthetic clip (moving test pattern plus noise, so it does not compress to nothing)
/// with two audio tracks into `dir`.
fn synthetic_clip(dir: &Path, name: &str, hdr: bool) -> PathBuf {
    assert!(dir.is_absolute());
    let path = dir.join(name);
    assert!(path.starts_with(dir));
    let video = if hdr {
        "noise=alls=12:allf=t,format=yuv420p10le,\
         setparams=color_trc=smpte2084:color_primaries=bt2020:colorspace=bt2020nc:range=pc"
    } else {
        "noise=alls=12:allf=t,format=yuv420p"
    };
    let codec: &[&str] = if hdr {
        &[
            "-c:v",
            "libx265",
            "-preset",
            "ultrafast",
            "-x265-params",
            "log-level=error",
        ]
    } else {
        &["-c:v", "libx264", "-preset", "ultrafast"]
    };
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args(["-f", "lavfi", "-i", "testsrc2=s=1920x1080:r=60:d=12"])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:d=12"])
        .args(["-f", "lavfi", "-i", "sine=frequency=880:d=12"])
        .args(["-map", "0:v", "-map", "1:a", "-map", "2:a", "-vf", video])
        .args(codec)
        .args(["-c:a", "aac", "-b:a", "96k"])
        .arg(&path)
        .status()
        .expect("ffmpeg runs");
    assert!(status.success(), "cannot write the synthetic clip");
    path
}

/// Average luma (0–255) of the first frame of a video, through `signalstats`.
fn first_frame_luma(video: &Path) -> f64 {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostdin", "-loglevel", "info", "-i"])
        .arg(video)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "signalstats,metadata=print:key=lavfi.signalstats.YAVG",
        ])
        .args(["-f", "null", "-"])
        .output()
        .expect("ffmpeg runs");
    let text = String::from_utf8_lossy(&output.stderr);
    text.lines()
        .find_map(|line| line.split("lavfi.signalstats.YAVG=").nth(1))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or_else(|| panic!("no luma in: {text}"))
}

fn engines(capabilities: &Capabilities, clip: &Path) -> Vec<Engine> {
    let mut engines = vec![Engine::SOFTWARE];
    if capabilities.placebo {
        engines.push(Engine {
            placebo: true,
            vulkan_decode: hardware::vulkan_decodes(clip),
        });
    }
    engines
}

fn check(clip: &Path, dir: &Path, capabilities: &Capabilities, hdr: bool) {
    let media = probe(clip).unwrap();
    assert_eq!(media.dynamic, if hdr { Dynamic::Pq } else { Dynamic::Sdr });
    assert_eq!(media.audio.len(), 2);
    let profile = Profile {
        max_size_mb: 2.0,
        audio: AudioChoice::MixAll,
        ..Profile::messenger()
    };
    let encoder = capabilities
        .choose(&profile.codecs, None)
        .expect("an H.265 encoder works");
    let slots = Slots::new(4);
    let cancel = Arc::new(AtomicBool::new(false));
    for (index, engine) in engines(capabilities, clip).into_iter().enumerate() {
        let request = EstimateRequest {
            path: clip.to_path_buf(),
            media: media.clone(),
            profile: profile.clone(),
            // A frame start rounded to milliseconds, as the timeline sets it (61/60 s): the seek lands
            // just after a frame, where libplacebo's frame rate option used to output a blank frame.
            start: 1.017,
            end: 9.017,
            engine,
            encoder,
        };
        let estimate = estimate::run(&request, &slots, &cancel)
            .unwrap_or_else(|e| panic!("estimate with {}: {e:#}", engine.label()));
        assert!(
            estimate.predicted_bytes <= profile.max_bytes(),
            "{estimate:?}"
        );
        assert_eq!(estimate.target.rate, FrameRate::new(30, 1));

        let name = format!("out-{}-{index}.mp4", if hdr { "hdr" } else { "sdr" });
        let output = naming::unique_output(dir, &name, clip, &[]);
        assert!(output.starts_with(dir));
        let encoded = encode::run(&request, &estimate, &output, &cancel, |_| {})
            .unwrap_or_else(|e| panic!("encode with {}: {e:#}", engine.label()));
        assert!(
            encoded.bytes <= profile.max_bytes(),
            "{} bytes",
            encoded.bytes
        );
        assert!(!naming::partial_path(&output).exists());

        let result = probe(&output).unwrap();
        assert_eq!(
            (result.width, result.height),
            (estimate.target.size.width, estimate.target.size.height)
        );
        assert_eq!(result.codec, "hevc");
        assert_eq!(result.frame_rate, FrameRate::new(30, 1));
        assert_eq!(result.dynamic, Dynamic::Sdr, "tone mapped to SDR");
        assert!(!result.full_range);
        assert_eq!(result.color_tags, "bt709/bt709/bt709/tv");
        assert_eq!(result.audio.len(), 1, "the two tracks are mixed");
        assert!((result.duration - 8.0).abs() < 0.1, "{}", result.duration);
        // libplacebo's own frame rate option once made the first frame after a seek blank.
        let luma = first_frame_luma(&output);
        assert!(
            luma > 40.0,
            "the first frame is dark ({luma}) with {}",
            engine.label()
        );
        eprintln!(
            "{}: {} {:.0} kbit/s QP {:.1} -> {} bytes (predicted {})",
            engine.label(),
            estimate.target.label(),
            estimate.video_bps / 1000.0,
            estimate.quality,
            encoded.bytes,
            estimate.predicted_bytes
        );
    }
}

#[test]
fn estimates_and_encodes_sdr_and_hdr_clips_within_the_limit() {
    if !ffmpeg_available() {
        eprintln!("FFmpeg is not on the PATH: skipped");
        return;
    }
    let capabilities = Capabilities::detect();
    eprintln!("{}", capabilities.summary());
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().canonicalize().unwrap();
    let sdr = synthetic_clip(&dir, "sdr.mp4", false);
    check(&sdr, &dir, &capabilities, false);
    let hdr = synthetic_clip(&dir, "hdr.mkv", true);
    check(&hdr, &dir, &capabilities, true);
}
