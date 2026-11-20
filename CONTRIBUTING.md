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

Warnings are errors, in clippy and in rustdoc.

Measuring the size fitting on a real video, without the window (the video is only read; the output folder
must exist and differ from the video's):

```powershell
cargo run --release --example transcode -- <video> [output folder] [size limit in MB]
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
