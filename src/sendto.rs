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
    let link = shortcut_path().with_context(|| crate::tr!("sendto-no-folder"))?;
    let exe = std::env::current_exe().with_context(|| crate::tr!("sendto-no-executable"))?;
    let folder = exe
        .parent()
        .with_context(|| crate::tr!("sendto-no-exe-folder"))?;
    // The description travels in an environment variable too, like the paths: no quoting can break.
    let script = "$link = (New-Object -ComObject WScript.Shell).CreateShortcut($env:QVT_LINK); \
                  $link.TargetPath = $env:QVT_TARGET; \
                  $link.WorkingDirectory = $env:QVT_FOLDER; \
                  $link.Description = $env:QVT_DESCRIPTION; \
                  $link.Save()";
    let output = crate::ffmpeg::tool("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("QVT_LINK", &link)
        .env("QVT_TARGET", &exe)
        .env("QVT_FOLDER", folder)
        .env("QVT_DESCRIPTION", crate::tr!("sendto-shortcut-description"))
        .stdin(Stdio::null())
        .output()
        .with_context(|| crate::tr!("sendto-start-failed"))?;
    if !output.status.success() || !link.is_file() {
        bail!(crate::tr!(
            "sendto-create-failed",
            path = link.display().to_string(),
            detail = String::from_utf8_lossy(&output.stderr).trim().to_owned()
        ));
    }
    Ok(link)
}

/// Deletes the shortcut, if it is there.
pub fn remove() -> Result<()> {
    let link = shortcut_path().with_context(|| crate::tr!("sendto-no-folder"))?;
    if link.is_file() {
        std::fs::remove_file(&link).with_context(|| {
            crate::tr!("sendto-delete-failed", path = link.display().to_string())
        })?;
    }
    Ok(())
}
