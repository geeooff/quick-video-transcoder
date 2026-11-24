---
name: upgrade-dependencies
description: Upgrade Quick Video Transcoder's dependencies across breaking versions — crate 0.x minors and majors (eframe/egui, wgpu, cpal, rfd, ICU4X, i18n-embed…), GitHub Actions majors, a new stable toolchain — following each upgrade guide, without workarounds, and explain a Dependabot PR that did not merge by itself. Use when the maintainer asks to update dependencies or why a Dependabot PR is waiting or failing.
---

# Upgrading the dependencies

Dependabot merges by itself what Cargo calls compatible (patches, minors of crates ≥ 1.0) once the `build`
check passes. A 0.x minor (`rfd` 0.17 → 0.18), a major, or an action major waits — by design of
`dependabot-automerge.yml`. With CI green, such a PR may be merged (standing approval); otherwise the work is
done here.

## Why a Dependabot PR is waiting or red

- Waiting with CI green: a 0.x minor or a major — the workflow skipped its merge step on purpose. Say so,
  then merge it if the maintainer's standing approval applies.
- Red: read `gh pr checks <n>` and `gh run view <id> --log-failed`; usually an API break (follow §2) or a new
  `rust-version` requirement.

## What is behind

`cargo info <crate>` for each direct dependency (`latest` shows the newest), each `uses:` against
`gh api repos/<owner>/<repo>/releases/latest --jq .tag_name`, `gh pr list --author app/dependabot`. Show the
list (locked, latest, kind of change) and agree on what to take.

## One dependency (or family) at a time

On `deps/<name>-<version>` from `main`:

1. **Read the upgrade path**: changelog / migration guide of every version between, from the publisher. Note
   each break this code meets.
2. Raise `Cargo.toml`, `cargo update -p <crate>`. A newer required Rust raises `rust-version` (comment why).
3. Fix each break **the way the guide says**. Never: a silenced lint, a `[patch]` or fork, a held-back
   version, a feature turned off to dodge a change.
4. Families that move together, in one commit and one Dependabot group (`.github/dependabot.yml`):
   - `eframe` + `egui`: egui is a direct dependency only for `monochrome_emoji_fonts`; a lone eframe bump
     builds two eguis and silently drops it. `app::tests::symbols_are_in_the_embedded_fonts` catches it.
   - The ICU4X crates (`icu_decimal`, `icu_locale_core`, `icu_provider`, `fixed_decimal`) on one 2.x line;
     `i18n-embed` + `i18n-embed-fl`. Check `i18n::tests` and a French screenshot (`25,0 Mo`).
5. egui upgrades: check the window — `QVT_SCREENSHOT` on a synthetic clip, and the pseudo-locale
   (`QVT_PSEUDO=1`) for layout regressions. Glyphs can change between versions.
6. `.\scripts\build.ps1` (and `build` for anything that changes the exe), read the whole output.
7. Changelog only for what a user notices. Push, PR, auto-merge (push needs the maintainer's go-ahead).

An upgrade that cannot be done properly (upstream bug, no replacement) is not forced: stop, write why, ask.
