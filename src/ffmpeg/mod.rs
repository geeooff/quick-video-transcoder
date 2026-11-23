//! Everything that talks to the FFmpeg command line tools.
//!
//! Rules every caller follows:
//! - spawn through [`tool`] (no console window) or [`background_tool`] (below normal priority, for the
//!   size estimate and the thumbnails, which must not slow the preview down);
//! - collect the standard error with [`StderrTail`] and read it with [`StderrTail::finish`] once the
//!   process has exited, never with a sleep;
//! - stop a process with [`kill_tree`]: `ffmpeg` on the `PATH` may be a Chocolatey or Scoop shim that runs
//!   the real one as its child.
//!
//! FFmpeg installed while the application (or Explorer, which starts it) was running is not on the `PATH`
//! the process inherited: [`Install::check`] then reads the current `PATH` from the user's and the
//! system's settings, and every tool started afterwards gets it.

pub mod audio;
pub mod decoder;
pub mod encoders;
pub mod filters;
pub mod hardware;
pub mod probe;

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread::JoinHandle;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The `PATH` read from the settings by [`refresh_path`], when the inherited one lacks FFmpeg.
static FRESH_PATH: RwLock<Option<OsString>> = RwLock::new(None);

/// A [`Command`] for an FFmpeg tool that does not open a console window on Windows.
pub fn tool(program: &str) -> Command {
    let mut command = Command::new(program);
    // Set on the command, the PATH is also where Rust looks the program up.
    if let Some(path) = FRESH_PATH
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Like [`tool`], with a below normal CPU priority on Windows.
pub fn background_tool(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut command = tool(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    command
}

/// The arguments every `ffmpeg` run starts with: no banner, no keyboard, errors only.
pub fn base_args() -> Vec<String> {
    ["-hide_banner", "-nostdin", "-loglevel", "error"]
        .map(String::from)
        .to_vec()
}

/// Kills a child process and, on Windows, the processes it started.
pub fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::process::Stdio;
        let _ = tool("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

/// The last lines a child process prints on its standard error, collected by a background thread.
pub struct StderrTail {
    lines: Arc<Mutex<VecDeque<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl StderrTail {
    const LINES: usize = 20;

    /// Starts collecting the standard error of `child`, spawned with `Stdio::piped()`.
    pub fn collect(child: &mut Child) -> Self {
        let lines = Arc::new(Mutex::new(VecDeque::new()));
        let thread = child.stderr.take().map(|stderr| {
            let lines = Arc::clone(&lines);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut line = Vec::new();
                while reader
                    .read_until(b'\n', &mut line)
                    .is_ok_and(|read| read > 0)
                {
                    let text = String::from_utf8_lossy(&line).trim_end().to_owned();
                    line.clear();
                    if text.trim().is_empty() {
                        continue;
                    }
                    let mut lines = lines.lock().unwrap_or_else(PoisonError::into_inner);
                    if lines.len() == Self::LINES {
                        lines.pop_front();
                    }
                    lines.push_back(text);
                }
            })
        });
        Self { lines, thread }
    }

    /// The last lines printed. Call it once the process has exited or was killed: it waits for the end of
    /// the stream.
    pub fn finish(&mut self) -> String {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        Vec::from(lines.clone()).join("\n")
    }
}

/// The oldest FFmpeg release the filters and encoder options are written for.
pub const MIN_MAJOR_VERSION: u32 = 7;

/// Whether the FFmpeg tools the application needs are on the `PATH`, and recent enough.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Install {
    /// `ffmpeg` and `ffprobe` run; the version line `ffmpeg -version` prints.
    Ready(String),
    /// This tool does not start: not installed, or not on the `PATH` this process inherited.
    Missing(&'static str),
    /// Older than [`MIN_MAJOR_VERSION`]; the version line.
    TooOld(String),
}

impl Install {
    /// Runs `ffmpeg -version` and `ffprobe -version`, with the current `PATH` from the settings if the
    /// inherited one has no FFmpeg.
    pub fn check() -> Self {
        // Debug builds: `QVT_PRETEND_NO_FFMPEG=1` shows what a machine without FFmpeg gets.
        #[cfg(debug_assertions)]
        if std::env::var_os("QVT_PRETEND_NO_FFMPEG").is_some() {
            return Self::Missing("ffmpeg");
        }
        let install = Self::check_once();
        if matches!(install, Self::Missing(_)) && refresh_path() {
            return Self::check_once();
        }
        install
    }

    fn check_once() -> Self {
        let version = |program: &'static str| {
            let output = tool(program)
                .arg("-version")
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())?;
            Some(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
            )
        };
        let Some(line) = version("ffmpeg") else {
            return Self::Missing("ffmpeg");
        };
        if version("ffprobe").is_none() {
            return Self::Missing("ffprobe");
        }
        match major_version(&line) {
            Some(major) if major < MIN_MAJOR_VERSION => Self::TooOld(line),
            _ => Self::Ready(line),
        }
    }
}

/// Reads the `PATH` as the user's and the system's settings now define it (what a new sign-in would
/// get) and gives it to the tools started from now on. `false` when it cannot be read.
pub fn refresh_path() -> bool {
    if !cfg!(windows) {
        return false;
    }
    let script = concat!(
        "[Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + ",
        "[Environment]::GetEnvironmentVariable('Path', 'User')"
    );
    let Ok(output) = tool("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return false;
    };
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || path.trim_matches(';').is_empty() {
        return false;
    }
    log::info!("FFmpeg not on the inherited PATH: using the PATH from the settings");
    *FRESH_PATH.write().unwrap_or_else(PoisonError::into_inner) = Some(path.into());
    true
}

/// The winget package of the FFmpeg build the application is tested with (gyan.dev's static full build:
/// libplacebo, Vulkan, NVENC, Quick Sync, AMF).
pub const WINGET_PACKAGE: &str = "Gyan.FFmpeg";

/// `true` when Windows' package manager, winget, runs here.
pub fn winget_available() -> bool {
    cfg!(windows)
        && tool("winget")
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
}

/// Installs FFmpeg with winget, for the current user, accepting the package's agreements (the user asked
/// for it from the *FFmpeg is needed* window, which says so). Takes a minute or two; the error carries
/// winget's last lines.
pub fn install_with_winget() -> anyhow::Result<()> {
    use anyhow::Context;
    let output = tool("winget")
        .args(["install", "--id", WINGET_PACKAGE, "--exact", "--silent"])
        .args(["--accept-package-agreements", "--accept-source-agreements"])
        .arg("--disable-interactivity")
        .stdin(std::process::Stdio::null())
        .output()
        .with_context(|| crate::tr!("media-winget-start-failed"))?;
    if output.status.success() {
        return Ok(());
    }
    // winget redraws its progress with carriage returns: keep the last real lines.
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|l| l.chars().any(char::is_alphanumeric))
        .collect();
    let tail = lines[lines.len().saturating_sub(3)..].join(" / ");
    anyhow::bail!(crate::tr!(
        "media-winget-failed",
        status = output.status.to_string(),
        detail = tail
    ))
}

/// The major version in `ffmpeg -version`'s first line (`ffmpeg version 9.0.2-full_build-www.gyan.dev
/// Copyright…`, `ffmpeg version n7.1`). `None` for a build from git (`N-118000-g…`, `2026-11-01-git-…`),
/// which is newer than any release.
pub fn major_version(line: &str) -> Option<u32> {
    let word = line
        .split_whitespace()
        .skip_while(|w| *w != "version")
        .nth(1)?;
    let word = word.strip_prefix('n').unwrap_or(word);
    let (major, rest) = word.split_once('.')?;
    let major: u32 = major.parse().ok()?;
    // A date-named git build has no dot after its year, and its "major" would be the year.
    (rest.chars().next().is_some_and(|c| c.is_ascii_digit()) && major < 1000).then_some(major)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_major_version() {
        let gyan = "ffmpeg version 9.0.2-full_build-www.gyan.dev Copyright (c) 2000-2026 the FFmpeg developers";
        assert_eq!(major_version(gyan), Some(9));
        assert_eq!(major_version("ffmpeg version n7.1 Copyright"), Some(7));
        assert_eq!(
            major_version("ffmpeg version 6.1.1-3ubuntu5 Copyright"),
            Some(6)
        );
        assert_eq!(
            major_version("ffmpeg version N-118000-g1234abcd Copyright"),
            None
        );
        assert_eq!(
            major_version("ffmpeg version 2026-11-01-git-abcdef-full_build"),
            None
        );
        assert_eq!(major_version(""), None);
    }
}
