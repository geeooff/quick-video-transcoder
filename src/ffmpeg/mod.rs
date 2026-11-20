//! Everything that talks to the FFmpeg command line tools.
//!
//! Rules every caller follows:
//! - spawn through [`tool`] (no console window) or [`background_tool`] (below normal priority, for the
//!   size estimate and the thumbnails, which must not slow the preview down);
//! - collect the standard error with [`StderrTail`] and read it with [`StderrTail::finish`] once the
//!   process has exited, never with a sleep;
//! - stop a process with [`kill_tree`]: `ffmpeg` on the `PATH` may be a Chocolatey or Scoop shim that runs
//!   the real one as its child.

pub mod audio;
pub mod decoder;
pub mod encoders;
pub mod filters;
pub mod hardware;
pub mod probe;

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A [`Command`] for an FFmpeg tool that does not open a console window on Windows.
pub fn tool(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
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
