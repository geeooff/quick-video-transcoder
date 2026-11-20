//! Output file names, and the rules that keep an encode from ever replacing a file it did not write.
//!
//! - The output is `<source stem><profile suffix>.mp4`, next to the source or in a chosen folder. When
//!   that name is taken, ` (2)`, ` (3)`… is added: an existing file is never overwritten.
//! - FFmpeg writes `<output>.qvt-partial`, renamed to the output once the encode succeeded and the size
//!   checked. Only a file carrying that marker is ever deleted or replaced by the application.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// The extension of a file being encoded.
pub const PARTIAL_EXTENSION: &str = "qvt-partial";

/// The output file name for `source`: `<stem><suffix>.mp4`.
pub fn output_name(source: &Path, suffix: &str) -> String {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "video".to_owned());
    format!("{stem}{}.mp4", sanitize(suffix))
}

/// Removes the characters Windows refuses in a file name.
fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()
        })
        .collect()
}

/// The first free path among `<dir>/<name>`, `<dir>/<stem> (2).mp4`, `(3)`… that is neither an existing
/// file, nor `source`, nor in `taken` (outputs already promised to other videos of the queue).
pub fn unique_output(dir: &Path, name: &str, source: &Path, taken: &[PathBuf]) -> PathBuf {
    let base = Path::new(name);
    let stem = base
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = base
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mp4".to_owned());
    (1..)
        .map(|n| {
            if n == 1 {
                dir.join(name)
            } else {
                dir.join(format!("{stem} ({n}).{extension}"))
            }
        })
        .find(|candidate| {
            !candidate.exists() && !same_file(candidate, source) && !taken.contains(candidate)
        })
        .expect("an unbounded range always finds a free name")
}

/// `true` when two paths name the same file (compared case-insensitively on Windows).
pub fn same_file(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// The file FFmpeg writes for `output`.
pub fn partial_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".");
    name.push(PARTIAL_EXTENSION);
    PathBuf::from(name)
}

/// Checks that `partial` is a partial file of `output` (same folder, marker extension) and is not the
/// source: the only kind of path the encode lets FFmpeg overwrite.
pub fn check_partial(partial: &Path, output: &Path, source: &Path) -> Result<()> {
    let marked = partial
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(PARTIAL_EXTENSION));
    if !marked || partial != partial_path(output) || partial.parent() != output.parent() {
        bail!(
            "refusing to write {}: not a partial file",
            partial.display()
        );
    }
    if same_file(partial, source) || same_file(output, source) {
        bail!("refusing to write over the source {}", source.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_output_after_the_source() {
        assert_eq!(
            output_name(
                Path::new("captures/Game 2026.01.02 - 03.04.05.01.mp4"),
                "_messenger"
            ),
            "Game 2026.01.02 - 03.04.05.01_messenger.mp4"
        );
        assert_eq!(output_name(Path::new("clip.mkv"), "_a:b?"), "clip_ab.mp4");
    }

    #[test]
    fn never_reuses_an_existing_or_promised_name() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("clip.mp4");
        std::fs::write(&source, b"source").unwrap();
        // The output name equals the source name: it is skipped.
        let first = unique_output(dir.path(), "clip.mp4", &source, &[]);
        assert_eq!(first, dir.path().join("clip (2).mp4"));
        std::fs::write(dir.path().join("clip_m.mp4"), b"older output").unwrap();
        let taken = vec![dir.path().join("clip_m (2).mp4")];
        let next = unique_output(dir.path(), "clip_m.mp4", &source, &taken);
        assert_eq!(next, dir.path().join("clip_m (3).mp4"));
    }

    #[test]
    fn only_marked_partials_may_be_written() {
        let output = Path::new("out/clip_m.mp4");
        let partial = partial_path(output);
        assert_eq!(partial, Path::new("out/clip_m.mp4.qvt-partial"));
        let source = Path::new("in/clip.mp4");
        assert!(check_partial(&partial, output, source).is_ok());
        assert!(check_partial(output, output, source).is_err());
        assert!(check_partial(&partial, output, output).is_err());
        assert!(
            check_partial(
                Path::new("elsewhere/clip_m.mp4.qvt-partial"),
                output,
                source
            )
            .is_err()
        );
    }
}
