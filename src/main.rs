//! The `quick-video-transcoder` executable: every argument is a video to queue (Explorer's *Send to*
//! passes the selected files this way), except `--version` and `--licenses` (the third-party notices),
//! which print and exit.
//!
//! A release build has no console, so what would go to one goes to `quick-video-transcoder.log` in the
//! data folder (the previous run's log is kept beside it), and a crash or a window that cannot open is
//! told in a message box naming that log. A second launch hands its files to the open window
//! ([`instance`]).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use eframe::egui;
use quick_video_transcoder::app::{APP_ID, TranscoderApp};
use quick_video_transcoder::{build_info, instance, profile};

/// The log of this run, in the data folder.
const LOG_FILE: &str = "quick-video-transcoder.log";
/// The log of the run before.
const PREVIOUS_LOG_FILE: &str = "quick-video-transcoder.previous.log";

fn main() {
    let arguments: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    // A release build has no console of its own, but a redirected output (a file, a pipe) works. A reader
    // that already left (PowerShell does not wait for a windowed program) is no error.
    let print = |text: &str| {
        let _ = writeln!(std::io::stdout(), "{text}");
    };
    match arguments.first().and_then(|a| a.to_str()) {
        Some("--version" | "-V") => return print(&build_info::version_text()),
        Some("--licenses") => return print(build_info::NOTICES),
        _ => {}
    }
    // The message boxes below run before the window: they follow the OS language (the app applies the
    // stored choice once it starts).
    quick_video_transcoder::i18n::apply(None);
    let data = profile::data_dir();
    // Before the log: a launch that only hands its files over must not rotate the open window's log.
    let incoming = match data.as_deref().map(|dir| instance::claim(dir, &arguments)) {
        Some(instance::Launch::Forwarded) => return,
        Some(instance::Launch::Primary(incoming)) => Some(incoming),
        None => None,
    };
    let log_path = data.as_deref().and_then(start_log);
    install_panic_hook(log_path.clone());
    log::info!("{}", build_info::version_text().replace('\n', ", "));
    if let Some(problem) = incoming.as_ref().and_then(|i| i.problem()) {
        log::warn!("videos sent later will open another window: {problem}");
    }

    let icon = eframe::icon_data::from_png_bytes(include_bytes!(
        "../assets/icon/quick-video-transcoder-256.png"
    ))
    .expect("the icon is a valid PNG");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Quick Video Transcoder")
            .with_app_id(APP_ID)
            .with_icon(icon)
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([1000.0, 620.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    let result = eframe::run_native(
        APP_ID,
        options,
        Box::new(move |cc| Ok(Box::new(TranscoderApp::new(cc, arguments, incoming)))),
    );
    if let Err(error) = result {
        log::error!("the window could not open: {error}");
        show_error(
            &quick_video_transcoder::tr!("startup-window-failed-title"),
            &with_log_hint(
                quick_video_transcoder::tr!("startup-window-failed", detail = error.to_string()),
                log_path.as_deref(),
            ),
        );
        std::process::exit(1);
    }
}

/// Logs to the data folder (and the console in debug builds); returns the log's path.
fn start_log(dir: &Path) -> Option<PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(LOG_FILE);
    let _ = std::fs::rename(&path, dir.join(PREVIOUS_LOG_FILE));
    let file = File::create(&path).ok()?;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(Tee(Mutex::new(file)))))
        .init();
    Some(path)
}

/// Writes to the log file, and to standard error where there is one (debug builds).
struct Tee(Mutex<File>);

impl Write for Tee {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if cfg!(debug_assertions) {
            let _ = std::io::stderr().write_all(bytes);
        }
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        file.write_all(bytes)?;
        file.flush()?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).flush()
    }
}

/// A panic is logged with its backtrace, and one on the window's thread is told in a message box: the
/// window then closes, and without the box it would vanish without a word.
fn install_panic_hook(log_path: Option<PathBuf>) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("{info}\n{backtrace}");
        default(info);
        if std::thread::current().name() == Some("main") {
            show_error(
                &quick_video_transcoder::tr!("startup-stopped-title"),
                &with_log_hint(
                    quick_video_transcoder::tr!("startup-stopped", detail = info.to_string()),
                    log_path.as_deref(),
                ),
            );
        }
    }));
}

/// The message with a last paragraph naming the log file, when there is one.
fn with_log_hint(text: String, path: Option<&Path>) -> String {
    match path {
        Some(path) => format!(
            "{text}\n\n{}",
            quick_video_transcoder::tr!("startup-log-hint", path = path.display().to_string())
        ),
        None => text,
    }
}

fn show_error(title: &str, text: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(text)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}
