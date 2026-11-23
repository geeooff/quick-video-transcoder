# Contributing

Thanks for your interest. [docs/architecture.md](docs/architecture.md) describes the design;
[AGENTS.md](AGENTS.md) the rules of this repository.

## Build and test

You need [rustup](https://rustup.rs) (`rust-toolchain.toml` selects stable with rustfmt and clippy) and
FFmpeg on the `PATH` for the end-to-end test. Before committing, run the same checklist as CI:

```powershell
.\scripts\build.ps1            # fmt, clippy, rustdoc, tests, doc links, no video or personal path tracked
.\scripts\build.ps1 build      # the same, then a release build
.\scripts\build.ps1 release    # from a clean tree: the same, then the zip in dist\
```

Warnings are errors, in clippy and in rustdoc. The release zip's third-party notices need
[cargo-about](https://github.com/EmbarkStudios/cargo-about) (`cargo install cargo-about --locked --features cli`);
`about.toml` lists the licences accepted. The icon and the version block are compiled in by `build.rs`
with `rc.exe` from the Windows SDK (installed with the MSVC build tools); without it the build still works,
without an icon. After editing `assets/icon/quick-video-transcoder.svg`, run `.\scripts\render-icon.ps1`
(needs Microsoft Edge) and commit the three files.

Measuring the size fitting on a real video, without the window (the video is only read; the output folder
must exist and differ from the video's):

```powershell
cargo run --release --example transcode -- <video> [output folder or -] [size limit in MB or -] [built-in profile name]
```

## Rules

- **Never commit a video**, and never a real path or name: tests generate synthetic clips with FFmpeg's
  `lavfi` sources at run time. `.gitignore` blocks video extensions and `build.ps1` checks the tracked
  files, but check your diff anyway.
- **Never write next to a real capture while testing.** Outputs of tests and measurements go to a
  temporary folder.
- No `unsafe` code: the crate forbids it.

## Commits and pull requests

- One logical change per commit, with a message that says what and why.
- A change you can see goes in `CHANGELOG.md` under `[Unreleased]`, in the same pull request.
- Pull requests must pass CI. Add tests for new behavior: pure logic gets a unit test.
