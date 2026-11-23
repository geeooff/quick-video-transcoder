//! Explorer's *Send to* menu: one shortcut, `Quick Video Transcoder.lnk`, in the user's own *Send to*
//! folder, pointing at this executable. Select captures, right-click, *Send to*, and they open in the
//! queue.
//!
//! Done from the application, so nobody needs to run a script: Windows blocks downloaded PowerShell
//! scripts by default. The shortcut is written by Windows' own `WScript.Shell` object through an inline
//! PowerShell command (a command is not subject to the script policy); the paths travel in environment
//! variables, never inside the command text, so no quoting can break. Only this one file is ever written
//! or deleted, and only when the user asks.

use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Context, Result, bail};

/// The shortcut's file name.
pub const SHORTCUT_NAME: &str = "Quick Video Transcoder.lnk";

/// Where the shortcut goes: `%APPDATA%\Microsoft\Windows\SendTo`. `None` off Windows or without
/// `APPDATA`.
pub fn shortcut_path() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let roaming = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(roaming)
            .join("Microsoft")
            .join("Windows")
            .join("SendTo")
            .join(SHORTCUT_NAME),
    )
}

/// `true` when the shortcut exists (wherever it points).
pub fn is_installed() -> bool {
    shortcut_path().is_some_and(|p| p.is_file())
}

/// Writes the shortcut to the running executable, replacing an older one (after a move of the folder).
pub fn install() -> Result<PathBuf> {
    let link = shortcut_path().context("no Send to folder on this system")?;
    let exe = std::env::current_exe().context("cannot find this executable")?;
    let folder = exe.parent().context("the executable has no folder")?;
    let script = "$link = (New-Object -ComObject WScript.Shell).CreateShortcut($env:QVT_LINK); \
                  $link.TargetPath = $env:QVT_TARGET; \
                  $link.WorkingDirectory = $env:QVT_FOLDER; \
                  $link.Description = 'Trim and re-encode videos to fit a size limit'; \
                  $link.Save()";
    let output = crate::ffmpeg::tool("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("QVT_LINK", &link)
        .env("QVT_TARGET", &exe)
        .env("QVT_FOLDER", folder)
        .stdin(Stdio::null())
        .output()
        .context("cannot start PowerShell")?;
    if !output.status.success() || !link.is_file() {
        bail!(
            "cannot create {}: {}",
            link.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(link)
}

/// Deletes the shortcut, if it is there.
pub fn remove() -> Result<()> {
    let link = shortcut_path().context("no Send to folder on this system")?;
    if link.is_file() {
        std::fs::remove_file(&link).with_context(|| format!("cannot delete {}", link.display()))?;
    }
    Ok(())
}
