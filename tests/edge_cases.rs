//! Videos unlike a game capture, end to end: no sound, shorter than a sample, portrait, an odd size, in a
//! folder whose name has accents, spaces and an apostrophe (and given as typed, not canonicalized). Each is
//! estimated and encoded with the Messenger profile and probed back. Skipped when FFmpeg is missing.
//!
//! Every clip is synthetic and written inside a fresh temporary folder.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use quick_video_transcoder::encode;
use quick_video_transcoder::estimate::{self, EstimateRequest};
use quick_video_transcoder::ffmpeg::filters::Engine;
use quick_video_transcoder::ffmpeg::hardware::Capabilities;
use quick_video_transcoder::ffmpeg::probe::probe;
use quick_video_transcoder::naming;
use quick_video_transcoder::profile::Profile;
use quick_video_transcoder::queue::Slots;

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

struct Clip {
    name: &'static str,
    size: (u32, u32),
    rate: u32,
    seconds: f64,
    sound: bool,
}

fn write_clip(dir: &Path, clip: &Clip) -> PathBuf {
    let path = dir.join(clip.name);
    assert!(path.starts_with(dir));
    let (width, height) = clip.size;
    let mut command = Command::new("ffmpeg");
    command
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args(["-f", "lavfi", "-i"])
        .arg(format!(
            "testsrc2=s={width}x{height}:r={}:d={}",
            clip.rate, clip.seconds
        ));
    if clip.sound {
        command
            .args(["-f", "lavfi", "-i"])
            .arg(format!("sine=frequency=440:d={}", clip.seconds));
    }
    let status = command
        .args(["-vf", "noise=alls=8:allf=t,format=yuv420p"])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac"])
        .arg(&path)
        .status()
        .expect("ffmpeg runs");
    assert!(status.success(), "cannot write {}", clip.name);
    path
}

fn encode_clip(dir: &Path, clip: &Clip, capabilities: &Capabilities) {
    let source = write_clip(dir, clip);
    let media = probe(&source).unwrap_or_else(|e| panic!("{}: {e:#}", clip.name));
    assert_eq!(media.audio.len(), usize::from(clip.sound), "{}", clip.name);
    let profile = Profile {
        max_size_mb: 2.0,
        ..Profile::messenger()
    };
    let request = EstimateRequest {
        path: source.clone(),
        media: media.clone(),
        profile: profile.clone(),
        start: 0.0,
        end: media.duration,
        // The software filters: the same on every machine, CI included.
        engine: Engine::SOFTWARE,
        encoder: capabilities
            .choose(&profile.codecs, None)
            .expect("an encoder works"),
    };
    let slots = Slots::new(2);
    let cancel = Arc::new(AtomicBool::new(false));
    let estimate = estimate::run(&request, &slots, &cancel)
        .unwrap_or_else(|e| panic!("estimate of {}: {e:#}", clip.name));
    let output = naming::unique_output(dir, &format!("out-{}", clip.name), &source, &[]);
    assert!(output.starts_with(dir));
    let encoded = encode::run(&request, &estimate, &output, &cancel, |_| {})
        .unwrap_or_else(|e| panic!("encode of {}: {e:#}", clip.name));
    assert!(encoded.bytes <= profile.max_bytes(), "{}", clip.name);

    let result = probe(&output).unwrap();
    let (width, height) = (result.width, result.height);
    assert!(
        width % 2 == 0 && height % 2 == 0,
        "{}: {width}x{height}",
        clip.name
    );
    assert!(width.min(height) <= 720, "{}: {width}x{height}", clip.name);
    let portrait = clip.size.1 > clip.size.0;
    assert_eq!(height > width, portrait, "{}: orientation kept", clip.name);
    assert_eq!(
        result.audio.len(),
        usize::from(clip.sound),
        "{}: sound",
        clip.name
    );
    assert!(
        (result.duration - media.duration).abs() < 0.15,
        "{}: {} s for {} s",
        clip.name,
        result.duration,
        media.duration
    );
    eprintln!(
        "{}: {} -> {width}x{height}, {} bytes",
        clip.name,
        estimate.target.label(),
        encoded.bytes
    );
}

#[test]
fn unusual_videos_encode_within_the_limit() {
    if !ffmpeg_available() {
        eprintln!("FFmpeg is not on the PATH: skipped");
        return;
    }
    let capabilities = Capabilities::detect();
    let temporary = tempfile::tempdir().unwrap();
    // As a user's folder would be typed: not canonicalized, with accents, spaces and an apostrophe.
    let dir = temporary.path().join("Vidéos de l'été");
    std::fs::create_dir(&dir).unwrap();
    for clip in [
        Clip {
            name: "no sound.mp4",
            size: (1280, 720),
            rate: 30,
            seconds: 6.0,
            sound: false,
        },
        Clip {
            name: "shorter than a sample.mp4",
            size: (1920, 1080),
            rate: 60,
            seconds: 2.5,
            sound: true,
        },
        Clip {
            name: "portrait.mp4",
            size: (1080, 1920),
            rate: 60,
            seconds: 5.0,
            sound: true,
        },
        Clip {
            name: "odd size é.mp4",
            size: (854, 478),
            rate: 25,
            seconds: 5.0,
            sound: true,
        },
    ] {
        encode_clip(&dir, &clip, &capabilities);
    }
}
