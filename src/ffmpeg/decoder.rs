//! Decodes a video to RGBA frames at the preview's size through an `ffmpeg` sub-process, with the filters
//! of the encode ([`super::filters`]): an HDR source is shown tone mapped exactly as it will be encoded.

use std::io::{ErrorKind, Read};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use super::filters::{self, Engine, GraphSpec, Sink};
use super::probe::MediaInfo;
use super::{StderrTail, base_args, tool};
use crate::plan::Size;
use crate::profile::ToneMapping;

/// What to decode.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodeConfig {
    /// Source file.
    pub path: PathBuf,
    /// Its media information.
    pub media: MediaInfo,
    /// How the filters run.
    pub engine: Engine,
    /// Size of the decoded frames.
    pub size: Size,
    /// HDR to SDR curve.
    pub tone: ToneMapping,
}

impl DecodeConfig {
    /// Bytes of one RGBA frame.
    pub fn frame_bytes(&self) -> usize {
        (self.size.width * self.size.height * 4) as usize
    }

    /// `ffmpeg` arguments decoding from `start` seconds. The frame rate is made constant at the source's
    /// nominal rate, so frame `n` is at `start + n / fps` even for a variable rate capture.
    pub fn args(&self, start: f64) -> Vec<String> {
        let graph = filters::graph(
            self.engine,
            &self.media,
            GraphSpec {
                size: self.size,
                rate: Some(self.media.frame_rate),
                tone: self.tone,
                sink: Sink::Preview,
            },
        );
        let mut args = base_args();
        args.extend(graph.input_args);
        if start > 0.0 {
            args.extend(["-ss".to_owned(), crate::timecode::arg(start)]);
        }
        args.extend(["-i".to_owned(), self.path.to_string_lossy().into_owned()]);
        args.extend(["-map", "0:v:0", "-an", "-sn", "-dn"].map(String::from));
        args.extend(["-vf".to_owned(), graph.filter]);
        args.extend(["-f", "rawvideo", "pipe:1"].map(String::from));
        args
    }
}

/// A decoded RGBA frame.
#[derive(Debug)]
pub struct Frame {
    /// Presentation time in source seconds.
    pub pts: f64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub data: Vec<u8>,
}

/// Messages produced by a [`FrameStream`].
#[derive(Debug)]
pub enum StreamEvent {
    /// A frame.
    Frame(Frame),
    /// The decoder stopped: end of file, or an error.
    End {
        /// What ffmpeg printed (its errors), empty at a normal end of file.
        detail: String,
    },
}

/// A running `ffmpeg` process streaming frames from a start time.
pub struct FrameStream {
    child: Child,
    events: Receiver<StreamEvent>,
    recycle: Sender<Vec<u8>>,
    reader: Option<JoinHandle<()>>,
    /// What it decodes.
    pub config: DecodeConfig,
    /// Where it started.
    pub start: f64,
}

impl FrameStream {
    /// Decoded frames buffered ahead of the display.
    const QUEUE: usize = 4;

    /// Starts decoding at `start` seconds.
    pub fn spawn(config: DecodeConfig, start: f64) -> Result<Self> {
        let args = config.args(start);
        log::debug!("preview: ffmpeg {}", args.join(" "));
        let mut child = tool("ffmpeg")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| crate::tr!("media-ffmpeg-start-failed"))?;
        let mut stderr = StderrTail::collect(&mut child);
        let mut stdout = child.stdout.take().context("no ffmpeg stdout")?;
        let (event_tx, events) = bounded(Self::QUEUE);
        let (recycle, recycle_rx) = unbounded::<Vec<u8>>();
        let frame_bytes = config.frame_bytes();
        let Size { width, height } = config.size;
        let fps = config.media.fps().max(1.0);
        let reader = std::thread::Builder::new()
            .name("preview decoder".to_owned())
            .spawn(move || {
                let mut index = 0u64;
                loop {
                    let mut data = recycle_rx.try_recv().unwrap_or_default();
                    data.resize(frame_bytes, 0);
                    if let Err(error) = stdout.read_exact(&mut data) {
                        if error.kind() != ErrorKind::UnexpectedEof {
                            log::warn!("preview read error: {error}");
                        }
                        let detail = stderr.finish();
                        let _ = event_tx.send(StreamEvent::End { detail });
                        return;
                    }
                    let frame = Frame {
                        pts: start + index as f64 / fps,
                        width,
                        height,
                        data,
                    };
                    index += 1;
                    if event_tx.send(StreamEvent::Frame(frame)).is_err() {
                        return;
                    }
                }
            })?;
        Ok(Self {
            child,
            events,
            recycle,
            reader: Some(reader),
            config,
            start,
        })
    }

    /// The next event, if one is ready.
    pub fn try_next(&self) -> Option<StreamEvent> {
        self.events.try_recv().ok()
    }

    /// The next event, waiting at most `timeout`.
    pub fn next_timeout(&self, timeout: Duration) -> Option<StreamEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    /// Gives a frame buffer back to the reader to avoid reallocating it.
    pub fn recycle(&self, data: Vec<u8>) {
        let _ = self.recycle.send(data);
    }
}

impl Drop for FrameStream {
    fn drop(&mut self) {
        super::kill_tree(&mut self.child);
        // Dropping the receiver unblocks a reader waiting on a full queue.
        self.events = crossbeam_channel::never();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg::probe::parse_probe_json;

    #[test]
    fn decodes_rgba_from_the_start_time() {
        let media = parse_probe_json(
            r#"{"streams": [{"codec_type": "video", "codec_name": "h264", "width": 1920,
                "height": 1080, "r_frame_rate": "60000/1001", "color_space": "bt709"}]}"#,
        )
        .unwrap();
        let config = DecodeConfig {
            path: PathBuf::from("in.mp4"),
            media,
            engine: Engine::SOFTWARE,
            size: Size {
                width: 960,
                height: 540,
            },
            tone: ToneMapping::Auto,
        };
        let args = config.args(12.5).join(" ");
        assert!(
            args.contains("-hwaccel auto -ss 12.500 -i in.mp4 -map 0:v:0 -an"),
            "{args}"
        );
        assert!(
            args.contains("-vf fps=60000/1001,scale=960:540:flags=bilinear"),
            "{args}"
        );
        assert!(args.ends_with("format=rgba -f rawvideo pipe:1"), "{args}");
        assert!(!config.args(0.0).join(" ").contains("-ss"));
        assert_eq!(config.frame_bytes(), 960 * 540 * 4);
    }
}
