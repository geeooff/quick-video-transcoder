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
- As many encodes at a time as the GPU runs without slowing down, measured once on each GPU and driver
  in a few seconds (two on an RTX 4090), with a size check that re-encodes slightly lower if a file comes
  out over the limit.
- Variable frame rate captures keep their real rate: an Xbox HDR capture that FFmpeg reports at 56 fps
  is read as 59.94 fps and encoded at 29.97 fps, not 28.
- *Copy* puts encoded files on the clipboard, ready to paste into a conversation; *Show* opens their
  folder.
- `install-sendto.ps1` adds the application to Explorer's *Send to* menu.
