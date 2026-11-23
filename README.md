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
  sound. A profile lists the codecs the destination plays, preferred first, and the application takes the
  first one your GPU encodes (the CPU only when no GPU encoder handles any). Built-in profiles:
  *Messenger* (720p, 30 fps, H.265 then H.264, under 25 MB) and *Discord* (AV1, H.265, H.264) for each
  upload limit (20 MB, 50 MB, 100 MB, 1 GB). Edit any setting on one or
  several selected videos, and save it as a new profile.
- **HDR to SDR done right.** An HDR capture is tone mapped on the GPU by libplacebo, with peak detection
  and gamut mapping, and the preview shows it exactly as it will be encoded.
- **Ready to paste.** *Copy* puts the encoded file on the clipboard: paste it into the conversation.

Nothing is ever overwritten: an output name that exists gets ` (2)`, and the source is only read.

## Requirements

- Windows 10 or 11 (64-bit), with a graphics driver supporting DirectX 12 or Vulkan. Linux and macOS
  build, but are not tested.
- [FFmpeg](https://ffmpeg.org/download.html) 7 or later. The simplest way is one command in Terminal:
  `winget install --id Gyan.FFmpeg -e`; or the *release full* build from
  [gyan.dev](https://www.gyan.dev/ffmpeg/builds/), with its `bin` folder added to the `Path`. GPU tone
  mapping needs an FFmpeg built with libplacebo and Vulkan (the gyan.dev full build is); without it the
  CPU filters are used. If FFmpeg is missing, the application says so and how to install it, and finds it
  once installed without signing out.
- A GPU encoder makes it fast: NVIDIA NVENC, Intel Quick Sync or AMD AMF. The CPU encoders work too,
  slowly. **Tested on NVIDIA GPUs only**: Quick Sync and AMF are detected and used, but have not been
  tried on real hardware yet.

## Install

1. Download the zip from the [latest release](https://github.com/Geeooff/quick-video-transcoder/releases/latest)
   and extract it where you keep your tools. It holds the program, this readme, its licence and the
   third-party notices.
2. Start `quick-video-transcoder.exe`. The program is not signed, so the first time Windows may show
   *Windows protected your PC*: click *More info*, then *Run anyway*. To check the download first, compare
   it with `SHA256SUMS.txt` on the release page, or verify where it was built with
   `gh attestation verify <zip> --repo Geeooff/quick-video-transcoder`.
3. *File → Add to Explorer's Send to menu*. Move the folder later? Do it again: the shortcut follows.
   *File → Remove from Explorer's Send to menu* takes it away.

Only one window opens: videos sent while it is open join its queue.

## Use

1. Select captures in Explorer, right-click, *Send to*, *Quick Video Transcoder* (or drop them on the
   window).
2. Click a video to preview it. Trim it with the handles, `I` / `O`, or the start and end fields.
3. Read the estimate under each video: the output it will get, its size, a word for its quality.
4. *Encode selected* or *Encode all*. As many videos encode at a time as your GPU handles: the
   application measures it once, in a few seconds, the first time it runs on a GPU.
5. *Copy* on an encoded video, then paste it into the conversation.

Keyboard shortcuts are under *Help*, with *About* (the version, and this documentation as it was for
that version).

## When something goes wrong

The application keeps a log of its last two runs in its data folder (*Profiles → Open profiles.toml
folder*): `quick-video-transcoder.log` and `quick-video-transcoder.previous.log`. *View → Log* shows the
current one in the window. If the window cannot open or the application stops, a message says why and
names the log; attach it to an [issue](https://github.com/Geeooff/quick-video-transcoder/issues).

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
codecs = ["h265", "h264"]
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
`{ kind = "track", track = 1 }` (tracks count from 0). `codecs` lists `av1`, `h265` and `h264` in order of
preference: the first one a GPU encoder handles is used, else the first one the CPU handles (files written
with a single `codec = "h265"` still load).

Does Messenger keep 1080p60, or re-encode AV1? Meta does not say: see [Messenger's limits](docs/messenger.md)
for what is documented and how to measure it.

## Documentation

| Page | For |
| --- | --- |
| [Messenger's limits](docs/messenger.md) | What Meta documents, and how to measure the rest. |
| [Discord's limits](docs/discord.md) | Upload limits with and without Nitro, formats, the Discord profiles. |
| [Architecture](docs/architecture.md) | How the size fitting, the filters and the preview work, with measurements. |
| [Changelog](CHANGELOG.md) | What each release changed. |
| [Contributing](CONTRIBUTING.md) | Building, testing, rules. |
| [AGENTS.md](AGENTS.md) | How to work in this repository. |

## License

MIT. See [LICENSE](LICENSE). The libraries built in are listed with their licences in
`THIRD-PARTY-NOTICES.html`, in the zip. FFmpeg is not included; it has its own licenses.
