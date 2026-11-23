# Working in this repository

**quick-video-transcoder**: a Rust (eframe/egui 0.37, wgpu) Windows application that queues game captures,
trims them on a timeline and re-encodes each to fit a size-limited destination (Messenger by default)
through the FFmpeg command line tools. Public repository: no personal data.

Design and measurements: [docs/architecture.md](docs/architecture.md). Messenger's documented limits:
[docs/messenger.md](docs/messenger.md). Machine-specific facts belong in `CLAUDE.local.md` (gitignored).

## Hard rules

- **Nothing is ever written near a real capture except by the user's own encode.** Tests and measurements
  use synthetic clips (`lavfi` `testsrc2`, `sine`) and write into a temporary or scratch folder. Real
  captures are only read (probe, measurement with `examples/transcode.rs` into a scratch folder); no part
  of their path ever becomes a path written to in a test.
- The encode writes only `<output>.qvt-partial`, after `naming::check_partial`, then renames it. An
  existing file is never overwritten (`naming::unique_output`). Keep both rules on every new write path.
- Repository content in **English**; talk to the maintainer in **French**.
- **Explicit approval each time** for: pushing, creating or changing the GitHub repository and its
  settings, tagging or publishing a release, rewriting `main`. Standing exception: Dependabot's compatible
  crate updates merge themselves once CI passes.

## Principles

- **The preview is the encode.** Preview, samples and encode build their filters with
  `ffmpeg::filters::graph`. A change of color handling, scaling or tone mapping goes there, and the
  preview follows.
- **Metadata decides, never the origin.** HDR means a PQ or HLG transfer tag; an untagged source is BT.709
  limited range; the range is read from the file. NVIDIA app and Xbox captures differ completely
  (docs/architecture.md, *The filters*).
- **Measure, then decide.** A speed or size claim is measured on this machine and written in
  docs/architecture.md, dated. Messenger's behaviour is a measurement to make (docs/messenger.md), not an
  assumption.
- **The UI renders state and holds no rule.** The queue owns the workers, the profile owns what a setting
  means. The UI never waits on a worker and touches the disk only when the user acts.

## Layout

```
src/app/        egui UI: mod (state, shortcuts, menus), queue_panel, preview, timeline, settings
src/queue.rs    jobs, workers, Slots (encoder sessions)       src/profile.rs  profiles, profiles.toml
src/plan.rs     frame rates, sizes, ladder, budget            src/estimate.rs size fitting
src/encode.rs   encode, size check, retry                     src/naming.rs   output and partial names
src/ffmpeg/     probe, encoders, filters, hardware, decoder (preview), audio
src/player.rs   preview playback      src/audio.rs  cpal output and clock      src/thumbnails.rs  strip
src/main.rs     arguments, log file, crash box, window     src/instance.rs one window, later launches' files
src/sendto.rs   the Send to shortcut                       src/build_info.rs + build.rs  version, commit, icon
assets/icon/    the icon: SVG source, .ico and .png from scripts/render-icon.ps1
tests/pipeline.rs   end to end on synthetic clips          examples/transcode.rs  measurement on a video
```

## Conventions

- Spawn FFmpeg with `ffmpeg::tool()` or `background_tool()` (no console window); collect stderr with
  `StderrTail::finish()` after exit; stop with `kill_tree` (Chocolatey and Scoop shims).
- Never hard-code an encoder: go through `Capabilities::encoder`. H.265 stays the default codec.
- Every text a user reads is a Fluent message: `crate::tr!("id")`, numbers through `NUM` in the message
  (docs/localization.md). Logs and technical details stay in English.
- Qualities are H.265 QPs everywhere; `EncoderSpec::qp` converts for AV1 scales.
- Module doc comments carry the rules a module is shaped by: read them first, update them with the rule.
- The cpal callback is real time: short locks, no I/O, never wait on a decoder.
- egui: `lost_focus()` lasts two frames, apply a text field only if the user typed in it (`time_edit`);
  sliders use `SliderClamping::Edits`. Some symbols are missing from egui's fonts (arrows, most emoji): check
  a screenshot after adding one.
- `QVT_PSEUDO=1` and `QVT_LANGUAGE=fr-CA` (debug builds) pseudo-localize or force a language for
  screenshots (docs/localization.md).
- `QVT_PRETEND_NO_FFMPEG=1` (debug builds) shows the *FFmpeg is needed* window without uninstalling
  FFmpeg. Never click *Install FFmpeg for me* while testing: it really runs winget.
- Checking the UI without driving it: `QVT_SCREENSHOT=<file.ppm>` saves the window once the queue is
  estimated and the preview shows a frame, then quits. Run it on synthetic clips in a scratch folder.

## Workflow

Project skills in `.claude/skills/`: **release**, **install-locally**, **upgrade-dependencies**, **localize** —
the procedures this repository repeats, with their pitfalls. Use them; update them when a procedure changes.


```powershell
.\scripts\build.ps1            # fmt, clippy -D warnings, rustdoc -D warnings, tests, doc links, no video / personal path
.\scripts\build.ps1 build      # + optimized build, release-fast profile (thin LTO, ~20 s)
.\scripts\build.ps1 release    # + clean tree, notices, the .exe, SHA256SUMS.txt and notes.md in dist\ (needs cargo-about)
```

- **Two optimized profiles.** `release` (fat LTO, one codegen unit: the smallest, best-optimized exe,
  but one thread, ~1 min 45 s) is what users get: tags, main's CI. `release-fast` (thin LTO, 16 units,
  ~20 s, exe ~12 % larger) is for checks: `build.ps1 build` and pull request CI (`release -Fast`). Never
  publish a `-Fast` build. Measured 2026-11-23 on a Ryzen 9 9950X3D: fat LTO cannot use more threads.
  CI caches are scoped: a pull request reads main's cache and never saves its own on an exact key match,
  so main's CI also builds `release-fast`, only to put its dependencies in the cache.

- **One file.** A release is `quick-video-transcoder.exe` alone: static C runtime, icon, third-party
  notices (`--licenses`, *About*) inside. FFmpeg stays outside, found on the `PATH` or installed with
  winget on the user's request. Keep it that way: nothing a user must unpack or install beside it.
- **Documentation ships at its commit.** `build.rs` stamps the commit into the executable; *About* and
  `--version` link the documentation *of that commit*. A change updates the README, `docs/` and module doc
  comments in the same commit, or the build that carries it links stale pages forever.

- Run `test` before every commit and read its whole output; never pipe it into `Select-Object -First`.
- Commit only when asked. Messages say what and why, ending with the `Co-Authored-By` line.
- **The changelog is the agent's to write**: each line is something the user sees or does, under *Added*,
  *Changed*, *Fixed* or *Removed*, in `[Unreleased]` as work lands. None for tests or refactors.
- A change the user sees updates the README in the same commit.
- Releases: bump the version and date the changelog section in the release commit, then tag `vX.Y.Z` (with
  the maintainer's approval); `release.yml` rebuilds, attests and publishes.
- GitHub: `main` protected by a ruleset requiring the `build` check (the CI job id). Auto-merge enabled
  for Dependabot.

## Pitfalls that have already cost time

- Shell heredocs and inline strings eat backslashes: any script with a backslash goes through a file
  written by an editing tool, never a heredoc. A Python heredoc editing a workflow turned `\b` into a
  backspace and `\n` into a line break in `release.yml` (2026-11-23).
- A PowerShell regex built in a double-quoted string runs `$(...)` as a subexpression: the release notes
  pattern did, and would have failed the first tag. Build patterns from single-quoted pieces; `build.ps1`
  tests the extraction on a sample changelog every run.
- A shell started by the Claude desktop app may read `%APPDATA%` through the package's private copy: the
  profiles file seen from such a shell may not be the one the application reads.
- `-hwaccel vulkan` silently falls back to software decoding for a codec Vulkan cannot decode, and
  libplacebo then receives software frames; `hardware::vulkan_decodes` tests the real file once.
- libplacebo's frame-mixing option is `frame_mixer` (default `none`), not `frame_mixing`.
- NVENC throughput on an RTX 4090 saturates at two concurrent encodes.
- libplacebo's `fps` option outputs one blank, untagged frame first when the seek lands between two
  frames (any trim start rounded to milliseconds): a black flash and an output without color tags. The
  frame rate goes through FFmpeg's `fps` filter before libplacebo; `tests/pipeline.rs` trims at 1.017 s to
  keep that case covered.
- egui labels are selectable by default and take the click from a row under them:
  `selectable_labels` is off for the whole window.
- Driving the window with computer-use: the Claude desktop app's own window may cover part of the screen.
  It is hidden in screenshots but still takes the clicks; check with `WindowFromPoint` before suspecting
  the UI, and move the window into the free area. `QVT_DEBUG_HITS=1` (debug builds) draws the widget under the pointer
  and logs each press with the layer it lands on. Drags need a press, several moves and a release, each a
  separate action: one quick drag reads as a click.
- ffprobe's `r_frame_rate` is a guess and can be far off on variable-rate captures (56 for a 59.94 fps
  Xbox HDR capture, which made 28 fps outputs): `probe::nominal_rate` trusts it only when it is a standard
  rate.
