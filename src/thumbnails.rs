//! The thumbnail strip under the trim timeline: frames spread evenly over a video, decoded by short
//! parallel seeks (`-ss T -frames:v 1`) at a low priority, with the preview's filters (HDR shown tone
//! mapped).

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{Context, Result, bail};
use crossbeam_channel::Sender;

use crate::ffmpeg::filters::{self, Engine, GraphSpec, Sink};
use crate::ffmpeg::probe::MediaInfo;
use crate::ffmpeg::{background_tool, base_args};
use crate::plan::{Size, fit};
use crate::profile::ToneMapping;
use crate::queue::JobId;

/// Thumbnails per video.
pub const COUNT: usize = 24;
/// Short side of a thumbnail in pixels.
const SHORT_SIDE: u32 = 90;
/// Seeks running at once for one video.
const WORKERS: usize = 3;

/// One decoded thumbnail.
#[derive(Debug)]
pub struct Thumbnail {
    /// The video it belongs to.
    pub job: JobId,
    /// Its place in the strip.
    pub index: usize,
    /// Its time in source seconds.
    pub time: f64,
    /// Picture size.
    pub size: Size,
    /// RGBA pixels.
    pub rgba: Vec<u8>,
}

/// The times of the strip: the middle of `count` equal parts of the video.
pub fn times(duration: f64, count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| duration * (i as f64 + 0.5) / count as f64)
        .collect()
}

/// The video whose strip to decode.
#[derive(Clone, Debug)]
pub struct StripRequest {
    /// The queued video.
    pub job: JobId,
    /// Its file.
    pub path: PathBuf,
    /// Its media information.
    pub media: MediaInfo,
    /// How its filters run.
    pub engine: Engine,
    /// HDR to SDR curve.
    pub tone: ToneMapping,
}

/// Decodes the strip of a video in the background, sending each thumbnail as it arrives. Stops when
/// `cancel` is set.
pub fn spawn(
    request: StripRequest,
    sender: Sender<Thumbnail>,
    notify: Arc<dyn Fn() + Send + Sync>,
    cancel: Arc<AtomicBool>,
) {
    let StripRequest {
        job,
        path,
        media,
        engine,
        tone,
    } = request;
    std::thread::spawn(move || {
        let size = fit(media.width, media.height, SHORT_SIDE);
        let times = times(media.duration, COUNT);
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..WORKERS {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= times.len() || cancel.load(Ordering::Relaxed) {
                            return;
                        }
                        match decode(&path, &media, engine, tone, size, times[index]) {
                            Ok(rgba) => {
                                let thumbnail = Thumbnail {
                                    job,
                                    index,
                                    time: times[index],
                                    size,
                                    rgba,
                                };
                                if sender.send(thumbnail).is_err() {
                                    return;
                                }
                                notify();
                            }
                            Err(error) => {
                                log::debug!("thumbnail at {:.1} s: {error:#}", times[index])
                            }
                        }
                    }
                });
            }
        });
    });
}

fn decode(
    path: &std::path::Path,
    media: &MediaInfo,
    engine: Engine,
    tone: ToneMapping,
    size: Size,
    time: f64,
) -> Result<Vec<u8>> {
    let graph = filters::graph(
        engine,
        media,
        GraphSpec {
            size,
            rate: None,
            tone,
            sink: Sink::Preview,
        },
    );
    let mut args = base_args();
    args.extend(graph.input_args);
    args.extend(["-ss".to_owned(), crate::timecode::arg(time)]);
    args.extend(["-i".to_owned(), path.to_string_lossy().into_owned()]);
    args.extend(["-map", "0:v:0", "-an", "-sn", "-dn", "-frames:v", "1"].map(String::from));
    args.extend(["-vf".to_owned(), graph.filter]);
    args.extend(["-f", "rawvideo", "pipe:1"].map(String::from));
    let output = background_tool("ffmpeg")
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .context("cannot run ffmpeg")?;
    let expected = (size.width * size.height * 4) as usize;
    if output.stdout.len() < expected {
        bail!(
            "no frame ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut rgba = output.stdout;
    rgba.truncate(expected);
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreads_the_strip_evenly() {
        assert_eq!(times(10.0, 4), [1.25, 3.75, 6.25, 8.75]);
        assert!(times(0.0, 3).iter().all(|&t| t == 0.0));
    }
}
