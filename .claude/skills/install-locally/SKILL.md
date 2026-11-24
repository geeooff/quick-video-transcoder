---
name: install-locally
description: Update the maintainer's installed copy of Quick Video Transcoder (the folder their Send to shortcut points at) from a published release with its GitHub attestation verified, or from a build of main. Use when the maintainer asks to update, reinstall or refresh the installed or "release" copy, or to test a published release locally.
---

# Updating the installed copy

The install folder and the shortcut are machine-specific: read them from `CLAUDE.local.md` (never write them
in a tracked file). The folder holds **only** `quick-video-transcoder.exe`; its *Send to* shortcut points at
that exe, so replacing the file in place keeps it working.

## Rules

- Never overwrite while the application runs: `Get-Process quick-video-transcoder`. If one runs, ask the
  maintainer to close it — never kill their window. Check the *path* of a running process: a test build of
  yours may be the one running.
- A windowed exe: PowerShell does not wait for it when capturing output. Ask it for its version with
  `Start-Process <exe> -ArgumentList '--version' -RedirectStandardOutput <file> -NoNewWindow -Wait`.
- Any script with a backslash goes through a file written with the Write tool. Assert every path before
  writing or deleting, and compare hashes after copying.
- Deleting anything else in the install folder needs the maintainer's yes.

## From a published release (preferred)

1. `gh release download vX.Y.Z --repo geeooff/quick-video-transcoder --dir <new scratch folder>`.
2. The SHA-256 of the exe equals the line in `SHA256SUMS.txt`.
3. `gh attestation verify <exe> --repo geeooff/quick-video-transcoder --signer-workflow geeooff/quick-video-transcoder/.github/workflows/release.yml --source-ref refs/tags/vX.Y.Z`
   — restricted to that workflow and tag; `--format json` shows the commit. Any failure: do not install.
4. `--version` prints `X.Y.Z` and the tag's commit.
5. Copy over the installed exe, re-check the hash, show the folder and the file's version block.

## From main (between releases, to try what is merged)

`git worktree add --detach <scratch>/main-tree origin/main`, `.\scripts\build.ps1 release` there (the
published profile), install `dist\quick-video-transcoder.exe` as above (hash, `--version` naming that
commit), then remove the worktree (`Remove-Item -LiteralPath ('\\?\' + <path>)` for long paths, then
`git worktree prune`). Say plainly that it is not a release (About shows the last version number).

## Windows details

- *Open with* / *Send to* names come from the exe's `FileDescription`, cached per path in
  `HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache`
  (`<path>.FriendlyAppName`, `.ApplicationCompany`). A stale name (an exe once run without a version block)
  is fixed by deleting those two values for that path — with the maintainer's yes.
- A shell started by the Claude desktop app may see `%APPDATA%` through the package's private copy: check
  the shortcut where Explorer reads it before concluding it is missing.
