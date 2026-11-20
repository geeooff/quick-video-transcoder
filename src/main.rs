//! The `quick-video-transcoder` executable: every argument is a video to queue (Explorer's *Send to*
//! passes the selected files this way).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use eframe::egui;
use quick_video_transcoder::app::{APP_ID, TranscoderApp};

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Quick Video Transcoder")
            .with_app_id(APP_ID)
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([1000.0, 620.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |cc| Ok(Box::new(TranscoderApp::new(cc, files)))),
    )
}
