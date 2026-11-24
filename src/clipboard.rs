//! Copies encoded files to the clipboard as a file list, the way Explorer's *Copy* does, so they paste
//! straight into a chat window (Messenger's web and desktop apps accept a pasted file).

use std::path::PathBuf;

use anyhow::Result;

/// Puts `files` on the clipboard as a file list.
#[cfg(windows)]
pub fn copy_files(files: &[PathBuf]) -> Result<()> {
    let paths: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    fn fail(error: impl std::fmt::Display) -> anyhow::Error {
        anyhow::anyhow!(crate::tr!(
            "clipboard-copy-failed",
            detail = error.to_string()
        ))
    }
    let _open = clipboard_win::Clipboard::new_attempts(10).map_err(fail)?;
    // Emptied first: a text copied earlier must not be pasted instead of the files.
    clipboard_win::raw::set_file_list_with(&paths, clipboard_win::options::DoClear).map_err(fail)
}

/// Puts `files` on the clipboard as a file list (Windows only).
#[cfg(not(windows))]
pub fn copy_files(_files: &[PathBuf]) -> Result<()> {
    anyhow::bail!(crate::tr!("clipboard-unsupported"))
}
