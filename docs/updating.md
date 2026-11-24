# Updating

**TL;DR** — *Help → Check for updates* opens a window that reads the latest GitHub release, shows its notes,
downloads `quick-video-transcoder.exe` beside the running one, checks its SHA-256 against two witnesses,
and on *Restart now* swaps the executable and restarts. The application connects only when the user clicks.
The swap is GameModeExecutor's (lot 13), measured there and here.

Code: `src/update/` (the rules, the feed, the swap), `src/app/update_window.rs` (the window). Its module doc
comments carry the rules.

## The window

| Phase | The window shows |
| --- | --- |
| Checking | a spinner |
| Up to date | *1.1.0 is the latest version*, *Check again* |
| Available | *Version 1.2.0 is available*, the release notes (the changelog section, English), *Download and install (18.9 MB)*, *See it on GitHub* |
| Downloading | a progress bar |
| Ready | *Restart now* — disabled while videos are encoding (a restart would stop them) |
| Updated | *Updated to 1.2.0*, at the first start of the new version |
| Failed | the reason in a sentence, *Details are in the log*, *Try again* or *Check again*, the release page |

Closing the window hides it: a download goes on and is found again on reopening. All rules live in
`update::Machine` (pure, tested whole); the window draws its phase and sends events.

## The release, and what is trusted

One `GET api.github.com/repos/{owner}/{repo}/releases/latest` (60 an hour per IP without a token): the tag
(exactly `vX.Y.Z`, else "an answer this version does not understand"), the notes, and per asset its URL,
size and `digest: sha256:…`. Drafts and pre-releases are refused; a release not newer than the running
version is never offered (no downgrade). The download uses that release's own URLs.

Two witnesses to the file, both over TLS: the workflow's `SHA256SUMS.txt` and, when GitHub gives one (it
computes it for every asset uploaded since 2025, this repository's included), its digest computed on
upload. They must agree with each other and with the downloaded bytes, else the file is deleted. **What this
does not defend against:** a compromised account or workflow — the hash proves the file is the one
published, not that it is honest. Verifying the build attestation (Sigstore) inside the application was
weighed on 2026-11-23 and left out: the Rust Sigstore crates are heavy and not yet stable; `gh attestation
verify` remains the way to check a download by hand (README). A file fetched by a library carries no Mark of
the Web, so SmartScreen does not see it: the hash is what vouches for it.

TLS: rustls (through `ureq`), certificates checked against the OS's trust store (`rustls-platform-verifier`:
a corporate proxy's own root is accepted like in a browser), Windows' proxy settings read
(`win-system-proxy`). A generic `User-Agent` with the version, nothing about the user. The trade, said
plainly: a TLS-inspecting proxy whose root the machine trusts can replace both the checksums and the file,
and the hash then proves nothing against it — as for any download on that machine.

GitHub allows 60 unauthenticated API requests an hour per IP address, shared by everyone behind it (a
household, an office, a carrier's NAT). A click-only check rarely comes near it; when it does, the window
says so and when the limit resets (`x-ratelimit-reset`), rather than "GitHub answered 403".

## The swap

The running executable is never written. Windows refuses to overwrite or delete an executable that runs, but
a separate process can rename it once it has exited:

1. The download goes to `<exe>.new`, beside the executable: the swap is a rename on the same folder, not a
   copy. Creating it is also the test that the folder can be written; a read-only folder (Program Files)
   fails here and the window offers the release page.
2. *Restart now* writes `updates\pending.txt` (the version being installed) in the data folder, starts a
   hidden Windows PowerShell by its absolute path (`%SystemRoot%\System32\WindowsPowerShell\v1.0`, never
   by name: Windows searches the application's own folder first, and a `powershell.exe` dropped in a
   Downloads folder would run; winget likewise) with `-NoProfile -NonInteractive -WindowStyle Hidden` and
   `CREATE_NO_WINDOW`, and closes the window the normal way. The window warns first when videos are in the
   queue: the queue lives in memory and does not survive the restart.
3. The script waits for this process (`Wait-Process -Id <pid> -Timeout 30`, only while that id is still
   this program by name: an id can be reused), removes an older `.old`, renames `<exe>` to `<exe>.old`
   (retried 10 times, 0.5 s apart) and `<exe>.new` to `<exe>` (retried for 30 s: OneDrive may still be
   uploading the new file — think of an executable on a Desktop that Windows 11 backs up), then starts
   `<exe>`. If the application did not close in 30 s, nothing is touched and `result.txt` says so.
4. On any failure, each recovery step runs in its own `try`, restore first: if the old executable was
   moved, the new one is set aside (back to `.new`) and the old one put back (retried); then the executable
   in place is started; `result.txt` is written last, so failing to write it never skips the restore. A new
   executable that is in place but refuses to start (an antivirus) is set aside the same way. The script a
   version carries is the one that installs the next: these paths were reviewed by GameModeExecutor's
   session on 2026-11-24, before 1.1.0 froze them.
5. The next start compares `pending.txt` with its own version: equal, *Updated to X* and the `.old` goes;
   different, the failure and `result.txt`'s reason, and the unused `.new` goes. A `.new` left without a
   pending update (a download never installed) is deleted at start.

Paths reach the script only as PowerShell single-quoted literals (`'` doubled) through `-LiteralPath`: spaces,
apostrophes and brackets cannot change the command (GameModeExecutor lost a delete to double quotes on
2026-09-17). Why PowerShell: `Wait-Process` waits for one process id, where a batch file could only guess
with a delay. Why not the `self-replace` crate (renaming the running image from inside): never measured
here, and it has no way back when the new file cannot be put in place.

The protocol proves itself from the second release that carries it: 1.1.0 is the first; the first real
update through it is 1.1.0 → 1.2.0.

## Measured, 2026-11-23, on the maintainer's machine

A debug build of the branch, copied to a scratch folder named with a space, an apostrophe and brackets
(`update test [it's]`), run with `QVT_UPDATE_AS=0.9.0` (debug builds only) so the published 1.0.0 counts as
newer:

| Step | Result |
| --- | --- |
| *Check for updates* | `releases/latest` answered, *Version 1.0.0 is available* with its notes, under a second |
| *Download and install* | 18,912,256 bytes in about a second, both witnesses agreed, *downloaded and verified* |
| *Restart now* | the window closed; the swap left `quick-video-transcoder.exe` with the published 1.0.0's SHA-256 (`a55a5f19…05ee02`) and the debug build as `.old`; 1.0.0 started and came to the foreground |
| Rollback | the same, with `.old` held open by another process: the removal failed ten times, the catch wrote *The process cannot access the file…* to `result.txt`, the executable stayed in place and restarted, and its next start showed the failure and the reason |

After the review's changes, 2026-11-24, the same way (`review test [it's]`):

| Case | Result |
| --- | --- |
| Normal update | 1.0.0 in place (`a55a5f19…`), the previous build as `.old`, 1.0.0 running |
| The new file refuses to start (the verified download replaced by a non-program just before *Restart now*) | `Start-Process` failed (*not a valid application for this OS platform*); the script set the new file aside, put the old executable back, started it; the next start showed the reason and deleted the rejected file — no `.old`, no `.new` left |

Not measured: behind a proxy; the *Updated to* message from a real release pair (1.0.0 does not read
`pending.txt` — it waits for 1.1.0 → 1.2.0); OneDrive's locks (the retries are there for them); the
focus after a slow close (near the 30 s wait, the window may only flash in the taskbar).

Known rare cases, left as they are: a second window (when the single instance could not be claimed) still
running the old image keeps `.old` held, and the next update fails safely until it is closed; that second
window's start may also delete a download the first one is making (`settle` tidies a `.new` with no pending
update), and the swap then fails safely.

## Files written

Beside the executable: `<exe>.new` (the download) and `<exe>.old` (the previous version, until the next
start). In the data folder: `updates\pending.txt` and `updates\result.txt`. Nothing else, and only after a
click.
