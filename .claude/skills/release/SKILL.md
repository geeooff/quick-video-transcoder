---
name: release
description: Prepare and publish a Quick Video Transcoder release (vX.Y.Z), from the release commit to the published, attested executable and the maintainer's installed copy. Use when the maintainer asks to prepare, cut, tag or publish a release.
---

# Releasing Quick Video Transcoder

A release is one file, `quick-video-transcoder.exe`, built by `release.yml` from a tag. Pushing the release
branch and the tag need the maintainer's go-ahead (a pull request merges by itself once CI is green: that
approval stands).

## 1. Ready?

- `main` holds everything meant for the release; open PRs are merged or deliberately left out.
- `CHANGELOG.md` `[Unreleased]` lists what the user sees, under *Added*, *Changed*, *Fixed*, *Removed*. For a
  first release of a feature, fixes made before it shipped are not *Fixed*: fold them into the *Added* line.
- The docs describe this version: the release links them at its commit forever (`AGENTS.md`).
- Version (semver): patch for fixes only, minor for a feature, major for a breaking change of what the user
  relies on (profiles file, behaviour). Propose it; the maintainer decides.

## 2. The release commit

On `release/vX.Y.Z` from an up-to-date `main`:

1. `version = "X.Y.Z"` in `Cargo.toml`, then `cargo update -p quick-video-transcoder --offline` for the lock.
2. `CHANGELOG.md`: a new empty `## [Unreleased]`, then `## [X.Y.Z] - YYYY-MM-DD` (today) with what was under
   it; at the bottom, `[Unreleased]: …/compare/vX.Y.Z...HEAD` and `[X.Y.Z]: …/releases/tag/vX.Y.Z` (and fix
   the previous version's link to compare with its predecessor).
3. Commit `Release X.Y.Z` (body: what it brings, why this bump).
4. `.\scripts\build.ps1 release` from that clean commit: the published fat LTO profile, never `-Fast`.
   Read the whole output: `notes.md from the changelog`, the notices, `release OK`.

## 3. Publish

1. Push the branch, open the PR (`Release X.Y.Z`), `gh pr merge <n> --auto --merge`.
2. Once merged: `git switch main && git pull --ff-only`, check `version` and the dated section, then tag the
   **merge commit** and push the tag alone:
   `git tag -a vX.Y.Z <merge sha> -m "Quick Video Transcoder X.Y.Z"`, `git push origin vX.Y.Z`.
3. Follow the run: `gh run list --workflow release.yml -L 1`, then
   `gh run watch <id> --exit-status` in the background. On failure read `gh run view <id> --log-failed`,
   fix on a branch, and re-tag only with the maintainer (a pushed tag is not moved silently).
4. Check the page: `gh release view vX.Y.Z` — `quick-video-transcoder.exe`, `SHA256SUMS.txt`, notes from the
   changelog, not a draft.

## 4. After

- Install it on this machine with the **install-locally** skill (from the release, attestation verified).
- Report to the maintainer: the release URL, what it contains, what was not verified (other GPUs, Linux).
