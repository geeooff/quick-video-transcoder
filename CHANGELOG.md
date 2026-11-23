# Changelog

What changed for the person using the application, one section per release, newest first. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). A release commit dates its section; the release workflow
refuses a tag without one.

## [Unreleased]

### Added

- The application speaks French: *View → Language* offers Automatic (the system's language), English
  (United States, United Kingdom) and French (France, Canada, Belgium, Switzerland). It switches at once,
  without restarting.
- Numbers follow the chosen language and region: `25,0 Mo` in French, the Canadian and Swiss groupings,
  `25.0 MB` in English. The size limit box accepts a comma or a point.

### Fixed

- *Encodes at once* is measured from 1 to 4 encodes and takes the fewest that reach the best speed: the
  NVIDIA driver can run the first two encodes on one engine and start the second engine only with a
  third, which the measurement read as a single engine (an RTX 4090 got 1 instead of 3).

## [1.0.0] - 2026-11-23

The first release.

### Added

- A queue of videos, filled from the command line, Explorer's *Send to* menu or drag and drop, with a
  thumbnail, the source details, the estimate and the progress of each.
- A preview with sound, showing each video at its output size and, for HDR, tone mapped as it will be
  encoded.
- A trim timeline with two handles that show the frame under them, a thumbnail strip, frame stepping,
  and `I` / `O` to set the start and end at the current frame.
- Size fitting: sample encodes find the best resolution, frame rate and bitrate under the size limit,
  lowering the frame rate then the resolution only when the quality would fall too low.
- Output profiles in `profiles.toml`, with built-in profiles for Messenger (720p, 30 fps, H.265, under
  25 MB) and for each Discord upload limit: 20 MB, 50 MB (Nitro Basic, server level 2), 100 MB (server
  level 3) and 1 GB (Nitro). Settings can be changed on several selected videos at once and saved as a
  profile; *Profiles → Edit profiles* adds back a built-in profile.
- Each profile lists the codecs its destination plays, preferred first, and the encode takes the first one
  a GPU encoder handles, falling back to the CPU only when none does: the Discord profiles encode AV1 on
  GPUs that have an AV1 encoder (RTX 40 series) and H.265 on the others, Messenger H.265. The *Codec*
  menu shows which codec and encoder a video gets.
- HDR to SDR tone mapping on the GPU with libplacebo, and the CPU filters when it is not available.
- The sound is the game's track by default, not the microphone: the first track whose name is not a
  microphone or a voice chat. The *Sound* menu lists each track with its name, as players show it
  ("System sounds", "Microphone"), and offers every track mixed or no sound.
- Encoders are detected by trying them with the options the encode uses, so a GPU that cannot run them
  falls back to another encoder instead of failing.
- As many encodes at a time as the GPU runs without slowing down, measured once on each GPU and driver
  in a few seconds (two on an RTX 4090; one at a time on the CPU, which already uses every core), with a
  size check that re-encodes slightly lower if a file comes out over the limit.
- Variable frame rate captures keep their real rate: an Xbox HDR capture that FFmpeg reports at 56 fps
  is read as 59.94 fps and encoded at 29.97 fps, not 28.
- *Copy* puts encoded files on the clipboard, ready to paste into a conversation; *Show* opens their
  folder.
- *File → Add to Explorer's Send to menu* adds the application to *Send to*, no script needed; *Remove*
  takes it away.
- One window at a time: videos sent while the window is open join its queue.
- An icon, for the program, its window, the taskbar and *Send to*, and a version in the file's
  properties.
- *Help → About* shows the version and the commit it was built from, and *Help → Documentation* opens the
  documentation as it was for that build.
- If FFmpeg is missing or too old, a window says how to install it, offers to do it with winget, and
  checks again; FFmpeg installed while Windows was running is found without signing out.
- A log of the last two runs in the data folder, and a message naming it if the window cannot open or
  the application stops.
- One file, nothing to install: the release is `quick-video-transcoder.exe` alone, the C runtime and the
  third-party licences inside (*Help → About*). Replacing the file updates it.

[Unreleased]: https://github.com/Geeooff/quick-video-transcoder/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/Geeooff/quick-video-transcoder/releases/tag/v1.0.0
