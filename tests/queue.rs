//! The queue as the UI drives it: a synthetic clip is added, probed, estimated once its settings settle,
//! trimmed, encoded into a temporary folder, and named without overwriting. Skipped without FFmpeg.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use quick_video_transcoder::ffmpeg::hardware::Capabilities;
use quick_video_transcoder::profile::Profile;
use quick_video_transcoder::queue::{JobId, Phase, Queue, QueueSettings};

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn wait(queue: &mut Queue, what: &str, mut done: impl FnMut(&Queue) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(180);
    while !done(queue) {
        for (is_error, line) in queue.poll() {
            eprintln!("{}{line}", if is_error { "error: " } else { "" });
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn phase(queue: &Queue, id: JobId) -> Phase {
    queue.get(id).unwrap().phase.clone()
}

#[test]
fn a_clip_goes_from_the_queue_to_a_trimmed_encode() {
    if !ffmpeg_available() {
        eprintln!("FFmpeg is not on the PATH: skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().canonicalize().unwrap();
    let clip = dir.join("clip.mp4");
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args(["-f", "lavfi", "-i", "testsrc2=s=1280x720:r=60:d=10"])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:d=10"])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac"])
        .arg(&clip)
        .status()
        .unwrap();
    assert!(status.success());

    let settings = QueueSettings {
        output_dir: Some(dir.clone()),
        ..QueueSettings::default()
    };
    let mut queue = Queue::new(Capabilities::detect(), settings, Arc::new(|| {}));
    let profile = Profile {
        max_size_mb: 1.0,
        ..Profile::messenger()
    };
    let id = queue.add(clip.clone(), &profile);
    wait(&mut queue, "the probe", |q| phase(q, id) == Phase::Idle);
    wait(&mut queue, "the first estimate", |q| {
        let job = q.get(id).unwrap();
        job.current_estimate(q.key(job).as_ref()).is_some()
    });

    // Trimming makes the estimate stale; encoding waits for the new one.
    let job = queue.get_mut(id).unwrap();
    job.trim.start = 2.0;
    job.trim.end = 6.0;
    job.touch();
    queue.encode(&[id]);
    wait(&mut queue, "the encode", |q| {
        matches!(phase(q, id), Phase::Done(_) | Phase::Failed(_))
    });
    let Phase::Done(encoded) = phase(&queue, id) else {
        panic!("{:?}", phase(&queue, id));
    };
    assert_eq!(encoded.output, dir.join("clip_messenger.mp4"));
    assert!(encoded.bytes <= 1_000_000, "{}", encoded.bytes);
    let media = quick_video_transcoder::ffmpeg::probe::probe(&encoded.output).unwrap();
    assert!((media.duration - 4.0).abs() < 0.1, "{}", media.duration);

    // A second encode never overwrites the first.
    queue.encode(&[id]);
    wait(&mut queue, "the second encode", |q| {
        matches!(phase(q, id), Phase::Done(_))
    });
    let Phase::Done(second) = phase(&queue, id) else {
        unreachable!()
    };
    assert_eq!(second.output, dir.join("clip_messenger (2).mp4"));
    assert!(Path::new(&encoded.output).exists());
}
