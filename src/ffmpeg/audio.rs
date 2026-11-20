//! Decodes the sound of a video to interleaved stereo `f32` samples through an `ffmpeg` sub-process, with
//! the audio choice of the video's profile (one track, or every track mixed).

use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Child, Stdio};
use std::thread::JoinHandle;

use anyhow::{Context, Result};

use super::{StderrTail, base_args, tool};
use crate::profile::AudioChoice;

/// Channels of the decoded audio (the output is stereo too).
pub const CHANNELS: usize = 2;

/// `ffmpeg` arguments decoding the chosen sound of a video with `tracks` audio streams from `start`
/// seconds at `rate` Hz. `None` when there is nothing to play.
pub fn audio_args(
    path: &Path,
    tracks: usize,
    choice: AudioChoice,
    start: f64,
    rate: u32,
) -> Option<Vec<String>> {
    let mut args = base_args();
    if start > 0.0 {
        args.extend(["-ss".to_owned(), crate::timecode::arg(start)]);
    }
    args.extend(["-i".to_owned(), path.to_string_lossy().into_owned()]);
    match choice {
        AudioChoice::None => return None,
        AudioChoice::MixAll if tracks > 1 => {
            let inputs: String = (0..tracks).map(|i| format!("[0:a:{i}]")).collect();
            args.extend([
                "-filter_complex".to_owned(),
                format!("{inputs}amix=inputs={tracks}:duration=first:normalize=0,alimiter=limit=0.97[mix]"),
                "-map".to_owned(),
                "[mix]".to_owned(),
            ]);
        }
        AudioChoice::Track(index) if index < tracks => {
            args.extend(["-map".to_owned(), format!("0:a:{index}")]);
        }
        AudioChoice::Track(_) => return None,
        AudioChoice::First | AudioChoice::MixAll => {
            if tracks == 0 {
                return None;
            }
            args.extend(["-map", "0:a:0"].map(String::from));
        }
    }
    args.extend(["-vn", "-sn", "-dn", "-ac"].map(String::from));
    args.push(CHANNELS.to_string());
    args.extend(["-ar".to_owned(), rate.to_string()]);
    args.extend(["-f", "f32le", "pipe:1"].map(String::from));
    Some(args)
}

/// A running `ffmpeg` process streaming audio samples to a sink.
pub struct AudioDecoder {
    child: Child,
    reader: Option<JoinHandle<()>>,
}

impl AudioDecoder {
    /// Starts decoding with `args` ([`audio_args`]). `sink` receives interleaved stereo samples (whole
    /// frames) on a background thread and returns `false` to stop; it receives `None` once at the end.
    pub fn spawn(
        args: Vec<String>,
        mut sink: impl FnMut(Option<&[f32]>) -> bool + Send + 'static,
    ) -> Result<Self> {
        log::debug!("audio: ffmpeg {}", args.join(" "));
        let mut child = tool("ffmpeg")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("cannot start ffmpeg (is FFmpeg on the PATH?)")?;
        let mut stderr = StderrTail::collect(&mut child);
        let mut stdout = child.stdout.take().context("no ffmpeg stdout")?;
        let reader = std::thread::Builder::new()
            .name("audio decoder".to_owned())
            .spawn(move || {
                const FRAME_BYTES: usize = CHANNELS * size_of::<f32>();
                let mut bytes = vec![0u8; 4096 * FRAME_BYTES];
                let mut samples = Vec::with_capacity(bytes.len() / size_of::<f32>());
                let mut filled = 0;
                loop {
                    match stdout.read(&mut bytes[filled..]) {
                        Ok(0) => break,
                        Ok(read) => filled += read,
                        Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                        Err(error) => {
                            log::warn!("audio read error: {error}");
                            break;
                        }
                    }
                    let whole = filled / FRAME_BYTES * FRAME_BYTES;
                    samples.clear();
                    samples.extend(
                        bytes[..whole]
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .map(|b| f32::from_le_bytes(*b)),
                    );
                    bytes.copy_within(whole..filled, 0);
                    filled -= whole;
                    if !samples.is_empty() && !sink(Some(&samples)) {
                        return;
                    }
                }
                let detail = stderr.finish();
                if !detail.is_empty() {
                    log::warn!("audio decoder: {detail}");
                }
                sink(None);
            })?;
        Ok(Self {
            child,
            reader: Some(reader),
        })
    }
}

impl Drop for AudioDecoder {
    fn drop(&mut self) {
        // Killing the process ends the pipe, so the reader stops unless the sink keeps it busy (the sink
        // must return `false` once it is no longer wanted).
        super::kill_tree(&mut self.child);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_chosen_sound() {
        let path = Path::new("in.mp4");
        let first = audio_args(path, 2, AudioChoice::First, 12.5, 48000)
            .unwrap()
            .join(" ");
        assert_eq!(
            first,
            "-hide_banner -nostdin -loglevel error -ss 12.500 -i in.mp4 \
             -map 0:a:0 -vn -sn -dn -ac 2 -ar 48000 -f f32le pipe:1"
        );
        let mix = audio_args(path, 2, AudioChoice::MixAll, 0.0, 44100)
            .unwrap()
            .join(" ");
        assert!(mix.contains("[0:a:0][0:a:1]amix=inputs=2"), "{mix}");
        assert!(!mix.contains("-ss"));
        assert_eq!(audio_args(path, 0, AudioChoice::First, 0.0, 48000), None);
        assert_eq!(audio_args(path, 2, AudioChoice::Track(3), 0.0, 48000), None);
        assert_eq!(audio_args(path, 2, AudioChoice::None, 0.0, 48000), None);
    }
}
