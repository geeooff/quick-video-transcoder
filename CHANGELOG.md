# Changelog

What changed for the person using the application, one section per release, newest first. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). A release commit dates its section; the release workflow
refuses a tag without one.

## [Unreleased]

### Added

- A queue of videos, filled from the command line, Explorer's *Send to* menu or drag and drop, with a
  thumbnail, the source details, the estimate and the progress of each.
- A preview with sound, showing each video at its output size and, for HDR, tone mapped as it will be
  encoded.
- A trim timeline with two handles that show the frame under them, a thumbnail strip, frame stepping,
  and `I` / `O` to set the start and end at the current frame.
- Size fitting: sample encodes find the best resolution, frame rate and bitrate under the size limit,
  lowering the frame rate then the resolution only when the quality would fall too low.
- Output profiles in `profiles.toml`, with a built-in *Messenger* profile (720p, 30 fps, H.265, under
  25 MB). Settings can be changed on several selected videos at once and saved as a profile.
- HDR to SDR tone mapping on the GPU with libplacebo, and the CPU filters when it is not available.
- Two encodes at a time on the GPU, with a size check that re-encodes slightly lower if a file comes out
  over the limit.
- *Copy* puts encoded files on the clipboard, ready to paste into a conversation; *Show* opens their
  folder.
- `install-sendto.ps1` adds the application to Explorer's *Send to* menu.
