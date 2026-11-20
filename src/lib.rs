//! quick-video-transcoder: queue game captures, trim them and re-encode each one to fit a size-limited
//! destination (Messenger by default), with HDR to SDR tone mapping.
//!
//! Everything that touches a video goes through the FFmpeg command line tools (`ffmpeg`, `ffprobe`) on the
//! `PATH`: there is no binding to the FFmpeg libraries. The preview, the size estimate and the encode build
//! their filters with the same function ([`ffmpeg::filters`]), so what the preview shows is what the encode
//! writes.

pub mod app;
pub mod audio;
pub mod clipboard;
pub mod encode;
pub mod estimate;
pub mod ffmpeg;
pub mod naming;
pub mod plan;
pub mod player;
pub mod profile;
pub mod queue;
pub mod thumbnails;
pub mod timecode;
