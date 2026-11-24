//! Downloading and verifying a release beside the executable, the swap at exit, and the verdict at the
//! next start.
//!
//! The swap is GameModeExecutor's, measured there on real updates (2026-09-18/23): the process never
//! touches its own image. It writes `pending.txt` (the version it installs), starts a hidden Windows
//! PowerShell and closes. The script waits for this exact process to end (`Wait-Process -Id`, 30 s at
//! most), renames `<exe>` to `<exe>.old` and `<exe>.new` to `<exe>` (renames on the same folder: no copy;
//! retried a few times, since OneDrive or an antivirus may hold a file just written), starts the new
//! executable, and on any failure writes `result.txt`, puts the old executable back and starts it. The next
//! start compares `pending.txt` with its own version ([`settle`]).
//!
//! Write rules (no other path is ever written): `<exe>.new` and `<exe>.old` beside the running executable,
//! named from it; `pending.txt` and `result.txt` in the data folder's `updates`. Paths reach the script only
//! as PowerShell single-quoted literals (`'` doubled) and through `-LiteralPath`, so spaces, quotes and
//! brackets in a path cannot change the command.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use sha2::{Digest, Sha256};

use super::feed::{EXE_ASSET, Feed};
use super::{Fault, Phase, Release, Version};

/// The files an update touches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    /// The running executable.
    pub exe: PathBuf,
    /// The downloaded one, beside it.
    pub new: PathBuf,
    /// The previous one, kept until the new version has started.
    pub old: PathBuf,
    /// `pending.txt`: the version being installed.
    pub pending: PathBuf,
    /// `result.txt`: why the swap failed, written by the script.
    pub result: PathBuf,
}

impl Paths {
    /// The paths for this executable and data folder.
    pub fn new(exe: &Path, data_dir: &Path) -> Self {
        let sibling = |suffix: &str| {
            let mut name = exe.file_name().unwrap_or_default().to_os_string();
            name.push(suffix);
            exe.with_file_name(name)
        };
        let updates = data_dir.join("updates");
        Self {
            exe: exe.to_path_buf(),
            new: sibling(".new"),
            old: sibling(".old"),
            pending: updates.join("pending.txt"),
            result: updates.join("result.txt"),
        }
    }

    /// The paths of this process, if its executable and data folder are known.
    pub fn current() -> Option<Self> {
        let exe = std::env::current_exe().ok()?;
        Some(Self::new(&exe, &crate::profile::data_dir()?))
    }

    /// `true` when `path` is one of the two files this module may write beside the executable.
    fn is_sibling(&self, path: &Path) -> bool {
        path.parent() == self.exe.parent() && (path == self.new || path == self.old)
    }
}

/// Writes through to a file while hashing what was written.
struct Hashing<'a> {
    file: &'a mut File,
    hasher: Sha256,
}

impl Write for Hashing<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let written = self.file.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// The SHA-256 `SHA256SUMS.txt` gives for `name` (`<hex>  <name>` lines).
pub fn published_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name && hash.len() == 64)
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Downloads `release`'s executable to `<exe>.new` and verifies it against both witnesses; a file that does
/// not verify is deleted. `progress` receives the bytes downloaded so far.
pub fn download(
    feed: &dyn Feed,
    release: &Release,
    paths: &Paths,
    progress: &mut dyn FnMut(u64),
) -> Result<(), Fault> {
    assert!(
        paths.is_sibling(&paths.new),
        "the download goes beside the executable"
    );
    let sums = feed.text(&release.sums_url)?;
    let published = published_hash(&sums, EXE_ASSET)
        .ok_or_else(|| Fault::Unexpected(format!("{EXE_ASSET} is not in the checksums")))?;
    if let Some(digest) = &release.exe_digest
        && *digest != published
    {
        return Err(Fault::Unexpected(
            "GitHub's digest and the checksums disagree".to_owned(),
        ));
    }
    // Creating it is also the test that the folder can be written.
    let mut file = File::create(&paths.new)
        .map_err(|error| Fault::CannotWrite(format!("{}: {error}", paths.new.display())))?;
    let mut writer = Hashing {
        file: &mut file,
        hasher: Sha256::new(),
    };
    let result = feed.download(&release.exe_url, &mut writer, progress);
    let actual = hex(&writer.hasher.finalize());
    let flushed = file.sync_all();
    drop(file);
    let verdict = match (result, flushed) {
        (Err(fault), _) => Err(fault),
        (Ok(_), Err(error)) => Err(Fault::CannotWrite(error.to_string())),
        (Ok(size), Ok(())) if size != release.exe_size => Err(Fault::Unexpected(format!(
            "{size} bytes received, {} published",
            release.exe_size
        ))),
        (Ok(_), Ok(())) if actual != published => {
            log::warn!("update: downloaded {actual}, published {published}");
            Err(Fault::NotVerified)
        }
        (Ok(_), Ok(())) => Ok(()),
    };
    if verdict.is_err() {
        let _ = std::fs::remove_file(&paths.new);
    }
    verdict
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A PowerShell single-quoted literal: nothing inside is interpreted, `'` is doubled.
fn quoted(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

/// The script run after this process (`pid`) exits.
///
/// Frozen into each release: the script a version carries is the one that installs the next, so every
/// failure path ends with an executable in place and running (reviewed by GameModeExecutor's session,
/// 2026-11-24):
/// - the wait checks the process is still this program, by name (a process id can be reused);
/// - the swap renames `exe` → `.old`, then `.new` → `exe` (retried for 30 s: OneDrive may hold a file it is
///   uploading), then starts it;
/// - on failure, each recovery step has its own `try`, restore first: the new file set aside (back to
///   `.new`) if the old one was moved, the old one put back, then the restart, and `result.txt` written
///   last, so a failure to write it never skips the restore.
pub fn swap_script(pid: u32, paths: &Paths) -> String {
    let (exe, new, old, result) = (
        quoted(&paths.exe),
        quoted(&paths.new),
        quoted(&paths.old),
        quoted(&paths.result),
    );
    let name = quoted(Path::new(paths.exe.file_stem().unwrap_or_default()));
    format!(
        "$ErrorActionPreference = 'Stop'
$exe = {exe}
$new = {new}
$old = {old}
$result = {result}
function Retry([scriptblock] $Step, [int] $Times) {{
    for ($i = 1; ; $i++) {{
        try {{ & $Step; return }} catch {{ if ($i -ge $Times) {{ throw }}; Start-Sleep -Milliseconds 500 }}
    }}
}}
function Still-Running {{
    $process = Get-Process -Id {pid} -ErrorAction SilentlyContinue
    $process -and $process.ProcessName -eq {name}
}}
if (Still-Running) {{ Wait-Process -Id {pid} -Timeout 30 -ErrorAction SilentlyContinue }}
if (Still-Running) {{
    try {{ Set-Content -LiteralPath $result -Value 'the application did not close' -Encoding UTF8 }} catch {{}}
    exit 1
}}
$moved = $false
$reason = $null
try {{
    if (Test-Path -LiteralPath $old) {{ Retry {{ Remove-Item -LiteralPath $old -Force }} 10 }}
    Retry {{ Move-Item -LiteralPath $exe -Destination $old -Force }} 10
    $moved = $true
    Retry {{ Move-Item -LiteralPath $new -Destination $exe }} 60
    Start-Process -FilePath $exe
    exit 0
}} catch {{
    $reason = $_.Exception.Message
}}
if ($moved) {{
    try {{
        if (Test-Path -LiteralPath $exe) {{ Retry {{ Move-Item -LiteralPath $exe -Destination $new -Force }} 10 }}
    }} catch {{ $reason += ' | setting the new file aside: ' + $_.Exception.Message }}
    try {{
        Retry {{ Move-Item -LiteralPath $old -Destination $exe -Force }} 20
    }} catch {{ $reason += ' | restoring the previous version: ' + $_.Exception.Message }}
}}
try {{ Start-Process -FilePath $exe }} catch {{ $reason += ' | restarting: ' + $_.Exception.Message }}
try {{ Set-Content -LiteralPath $result -Value $reason -Encoding UTF8 }} catch {{}}
"
    )
}

/// Writes `pending.txt` and starts the swap script. The caller closes the application at once.
pub fn launch(release: &Release, paths: &Paths) -> Result<(), Fault> {
    let updates = paths.pending.parent().expect("pending.txt is in a folder");
    std::fs::create_dir_all(updates)
        .and_then(|()| std::fs::write(&paths.pending, release.version.to_string()))
        .map_err(|error| Fault::CannotWrite(format!("{}: {error}", paths.pending.display())))?;
    let _ = std::fs::remove_file(&paths.result);
    let script = swap_script(std::process::id(), paths);
    log::info!(
        "update: installing {}; the application closes and comes back on it",
        release.version
    );
    crate::ffmpeg::powershell()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
        ])
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|error| {
            let _ = std::fs::remove_file(&paths.pending);
            Fault::Launch(error.to_string())
        })
}

/// At start: the outcome of an update the previous run started, if any, and tidying. `None` when no update
/// was pending. A leftover `<exe>.new` without a pending update (a download never installed) is deleted.
pub fn settle(paths: &Paths, current: Version) -> Option<Phase> {
    let Ok(text) = std::fs::read_to_string(&paths.pending) else {
        if paths.new.is_file() {
            let _ = std::fs::remove_file(&paths.new);
        }
        return None;
    };
    let pending = Version::parse(text.trim_start_matches('\u{feff}').trim());
    let reason = std::fs::read_to_string(&paths.result)
        .ok()
        .map(|r| r.trim_start_matches('\u{feff}').trim().to_owned());
    let _ = std::fs::remove_file(&paths.pending);
    let _ = std::fs::remove_file(&paths.result);
    if pending == Some(current) {
        log::info!("update: updated to {current}");
        if paths.old.is_file() {
            let _ = std::fs::remove_file(&paths.old);
        }
        Some(Phase::Updated(current))
    } else {
        // The download that was not installed goes: a new attempt downloads again.
        if paths.new.is_file() {
            let _ = std::fs::remove_file(&paths.new);
        }
        let reason = reason.unwrap_or_else(|| "did not take".to_owned());
        log::warn!(
            "update: the update to {} failed ({reason}); still {current}",
            text.trim()
        );
        Some(Phase::Failed {
            fault: Fault::Launch(reason),
            release: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// A scripted feed: fixed checksums and file, no network.
    struct Scripted {
        sums: String,
        file: Vec<u8>,
        seen: Mutex<Vec<String>>,
    }

    impl Feed for Scripted {
        fn latest(&self) -> Result<Release, Fault> {
            unreachable!()
        }
        fn text(&self, url: &str) -> Result<String, Fault> {
            self.seen.lock().unwrap().push(url.to_owned());
            Ok(self.sums.clone())
        }
        fn download(
            &self,
            url: &str,
            out: &mut dyn Write,
            progress: &mut dyn FnMut(u64),
        ) -> Result<u64, Fault> {
            self.seen.lock().unwrap().push(url.to_owned());
            out.write_all(&self.file).unwrap();
            progress(self.file.len() as u64);
            Ok(self.file.len() as u64)
        }
    }

    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn release(digest: Option<&str>) -> Release {
        Release {
            version: Version(1, 1, 0),
            page: String::new(),
            notes: String::new(),
            exe_url: "exe-url".to_owned(),
            exe_size: 5,
            exe_digest: digest.map(str::to_owned),
            sums_url: "sums-url".to_owned(),
        }
    }

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir
            .path()
            .join("app folder")
            .join("quick-video-transcoder.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"old").unwrap();
        let paths = Paths::new(&exe, &dir.path().join("data"));
        (dir, paths)
    }

    #[test]
    fn names_its_files_beside_the_executable_and_in_the_data_folder() {
        let paths = Paths::new(
            Path::new(r"C:\Tools\quick-video-transcoder.exe"),
            Path::new(r"C:\Data"),
        );
        assert_eq!(
            paths.new,
            Path::new(r"C:\Tools\quick-video-transcoder.exe.new")
        );
        assert_eq!(
            paths.old,
            Path::new(r"C:\Tools\quick-video-transcoder.exe.old")
        );
        assert_eq!(paths.pending, Path::new(r"C:\Data\updates\pending.txt"));
        assert!(paths.is_sibling(&paths.new));
        assert!(!paths.is_sibling(&paths.exe));
    }

    #[test]
    fn reads_the_checksums_file() {
        let sums = format!(
            "{}  quick-video-transcoder.exe\n",
            HELLO_SHA256.to_uppercase()
        );
        assert_eq!(
            published_hash(&sums, "quick-video-transcoder.exe").as_deref(),
            Some(HELLO_SHA256)
        );
        assert_eq!(published_hash(&sums, "other.exe"), None);
        assert_eq!(published_hash("<html>", "quick-video-transcoder.exe"), None);
    }

    #[test]
    fn keeps_a_verified_download_and_deletes_a_bad_one() {
        let (_dir, paths) = setup();
        let good = Scripted {
            sums: format!("{HELLO_SHA256}  quick-video-transcoder.exe"),
            file: b"hello".to_vec(),
            seen: Mutex::default(),
        };
        download(&good, &release(Some(HELLO_SHA256)), &paths, &mut |_| {}).unwrap();
        assert_eq!(std::fs::read(&paths.new).unwrap(), b"hello");
        assert_eq!(
            std::fs::read(&paths.exe).unwrap(),
            b"old",
            "the executable is untouched"
        );

        let tampered = Scripted {
            file: b"hellO".to_vec(),
            ..good
        };
        assert_eq!(
            download(&tampered, &release(Some(HELLO_SHA256)), &paths, &mut |_| {}),
            Err(Fault::NotVerified)
        );
        assert!(!paths.new.exists(), "a file that did not verify is deleted");
        let disagreeing = release(Some(&"0".repeat(64)));
        assert!(matches!(
            download(&tampered, &disagreeing, &paths, &mut |_| {}),
            Err(Fault::Unexpected(_))
        ));
    }

    #[test]
    fn the_script_quotes_every_path_and_puts_the_old_executable_back() {
        let paths = Paths::new(
            Path::new(r"C:\It's [a] folder\quick-video-transcoder.exe"),
            Path::new(r"C:\Data"),
        );
        let script = swap_script(4242, &paths);
        assert!(script.contains("Wait-Process -Id 4242 -Timeout 30"));
        assert!(script.contains(r"$exe = 'C:\It''s [a] folder\quick-video-transcoder.exe'"));
        assert!(script.contains("ProcessName -eq 'quick-video-transcoder'"));
        assert!(!script.contains('"'), "no double quote reaches PowerShell");
        let at = |needle: &str| script.find(needle).unwrap_or_else(|| panic!("{needle}"));
        // The swap: old aside, new in place (30 s of retries), then started.
        assert!(at("-Destination $old -Force } 10") < at("-Destination $exe } 60"));
        assert!(at("-Destination $exe } 60") < at("exit 0"));
        // The recovery: the new file aside, the old one back, the restart, the report last.
        let aside = at("-Destination $new -Force } 10");
        let restore = at("Move-Item -LiteralPath $old -Destination $exe -Force } 20");
        let restart = script.rfind("Start-Process -FilePath $exe").unwrap();
        let report = script.rfind("Set-Content -LiteralPath $result").unwrap();
        assert!(aside < restore && restore < restart && restart < report);
    }

    /// Windows PowerShell's own parser accepts the script (it is never run here).
    #[cfg(windows)]
    #[test]
    fn the_script_parses_in_windows_powershell() {
        let paths = Paths::new(
            Path::new(r"C:\It's [a] folder\quick-video-transcoder.exe"),
            Path::new(r"C:\Data"),
        );
        let output = crate::ffmpeg::powershell()
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg("[void][ScriptBlock]::Create($env:QVT_SCRIPT); 'parsed'")
            .env("QVT_SCRIPT", swap_script(4242, &paths))
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("parsed"),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn the_next_start_says_whether_the_update_took() {
        let (_dir, paths) = setup();
        assert_eq!(settle(&paths, Version(1, 1, 0)), None);

        std::fs::create_dir_all(paths.pending.parent().unwrap()).unwrap();
        std::fs::write(&paths.pending, "\u{feff}1.1.0\r\n").unwrap();
        std::fs::write(&paths.old, b"old").unwrap();
        assert_eq!(
            settle(&paths, Version(1, 1, 0)),
            Some(Phase::Updated(Version(1, 1, 0)))
        );
        assert!(!paths.old.exists() && !paths.pending.exists());

        std::fs::write(&paths.pending, "1.1.0").unwrap();
        std::fs::write(&paths.result, "\u{feff}access denied").unwrap();
        assert_eq!(
            settle(&paths, Version(1, 0, 0)),
            Some(Phase::Failed {
                fault: Fault::Launch("access denied".to_owned()),
                release: None
            })
        );
        assert!(!paths.result.exists());

        std::fs::write(&paths.new, b"stale").unwrap();
        assert_eq!(settle(&paths, Version(1, 0, 0)), None);
        assert!(!paths.new.exists(), "a download never installed is tidied");
    }
}
