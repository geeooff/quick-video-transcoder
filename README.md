# Quick Video Transcoder

Share game captures in a chat without fighting its size limit. Drop a few clips, trim each one on a
timeline, and the application finds, for each, the best resolution, frame rate and bitrate that fit the
destination — Messenger's 25 MB by default — then encodes them all on your GPU.

Made for NVIDIA app captures (4K60 AV1 HDR) and Xbox captures synced through OneDrive (1080p60 H.264),
and any video FFmpeg reads.

## What it does

- **A queue.** Every file given on the command line, dropped on the window or sent from Explorer's
  *Send to* menu joins the queue, with a thumbnail and its source details.
- **Visual trimming.** A preview with sound, a timeline with two handles and a strip of thumbnails: drag a
  handle and the preview shows the frame under it. `I` and `O` set the start and end at the current frame.
- **Size fitting.** For each video the application encodes short samples to learn how hard its picture
  is, then picks the best target that fits the limit: it lowers the frame rate, then the resolution, only
  when the quality would otherwise fall too low, and stops spending bytes once more would not show.
- **Profiles.** A profile is a destination's limits: size, resolution, frame rate, codec, SDR only or not,
  sound. The built-in *Messenger* profile is 720p, 30 fps, H.265, under 25 MB. Edit any setting on one or
  several selected videos, and save it as a new profile.
- **HDR to SDR done right.** An HDR capture is tone mapped on the GPU by libplacebo, with peak detection
  and gamut mapping, and the preview shows it exactly as it will be encoded.
- **Ready to paste.** *Copy* puts the encoded file on the clipboard: paste it into the conversation.

Nothing is ever overwritten: an output name that exists gets ` (2)`, and the source is only read.

## Requirements

- Windows 10 or 11 (64-bit). Linux and macOS build, but are not tested.
- [FFmpeg](https://ffmpeg.org/download.html) 7 or later on the `PATH`, for instance the *full* build from
  [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) or `winget install Gyan.FFmpeg`. GPU tone mapping needs
  an FFmpeg built with libplacebo and Vulkan (the gyan.dev full build is); without it the CPU filters are
  used.
- A GPU encoder makes it fast: NVIDIA NVENC, Intel Quick Sync or AMD AMF. The CPU encoders work too.

## Install

1. Download the zip from the [latest release](https://github.com/Geeooff/quick-video-transcoder/releases/latest)
   and extract it where you keep your tools.
2. To add it to Explorer's *Send to* menu, run the script that comes with it:

   ```powershell
   .\install-sendto.ps1
   ```

   `.\install-sendto.ps1 -Remove` takes it away.

## Use

1. Select captures in Explorer, right-click, *Send to*, *Quick Video Transcoder* (or drop them on the
   window).
2. Click a video to preview it. Trim it with the handles, `I` / `O`, or the start and end fields.
3. Read the estimate under each video: the output it will get, its size, a word for its quality.
4. *Encode selected* or *Encode all*. Two videos encode at a time.
5. *Copy* on an encoded video, then paste it into the conversation.

Keyboard shortcuts are under *Help*.

## Profiles

Profiles are kept in `profiles.toml` in the application's data folder (*Profiles → Open profiles.toml
folder*), editable by hand:

```toml
[[profile]]
name = "Messenger"
suffix = "_messenger"
max_size_mb = 25.0
max_short_side = 720
min_short_side = 360
max_fps = 30.0
min_fps = 24.0
codec = "h265"
sdr_only = true
tone_mapping = "auto"
audio_bitrate_kbps = 128
quality_ceiling = 20
quality_floor = 32

[profile.audio]
kind = "auto"
```

Sizes are megabytes of 1,000,000 bytes. Qualities are H.265 QPs, lower is better: `quality_ceiling` is the
best quality worth its bytes, `quality_floor` the worst accepted before lowering the frame rate or the
resolution. `audio` is `auto` (the game's track, not the microphone), `mix-all`, `none`, or
`{ kind = "track", track = 1 }` (tracks count from 0).

Does Messenger keep 1080p60, or re-encode AV1? Meta does not say: see [Messenger's limits](docs/messenger.md)
for what is documented and how to measure it.

## Documentation

| Page | For |
| --- | --- |
| [Messenger's limits](docs/messenger.md) | What Meta documents, and how to measure the rest. |
| [Architecture](docs/architecture.md) | How the size fitting, the filters and the preview work, with measurements. |
| [Changelog](CHANGELOG.md) | What each release changed. |
| [Contributing](CONTRIBUTING.md) | Building, testing, rules. |
| [AGENTS.md](AGENTS.md) | How to work in this repository. |

## License

MIT. See [LICENSE](LICENSE). FFmpeg is not included; it has its own licenses.
