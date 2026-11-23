# Architecture

**TL;DR** — An egui window over the FFmpeg command line tools. Each queued video is probed, estimated by
encoding samples at constant QP, and encoded at the bitrate the estimate chose. The preview, the samples
and the encode share one filter builder, so the preview shows what the encode writes. HDR is tone mapped
on the GPU by libplacebo.

## Layout

| Module | Role |
| --- | --- |
| `src/app/` | The egui UI: queue panel, preview, trim timeline, settings and profiles. Renders state, holds no rule. |
| `src/queue.rs` | The videos and their background work (probe, estimate, encode), on threads reporting through a channel. |
| `src/profile.rs` | Output profiles and `profiles.toml`. |
| `src/plan.rs` | Pure arithmetic: frame rates, output sizes, the ladder of targets, the byte budget. |
| `src/estimate.rs` | The size fitting: sample encodes and the solver. |
| `src/encode.rs` | The encode, its size check and retry. |
| `src/naming.rs` | Output names and the partial-file rules. |
| `src/ffmpeg/` | Probe, encoders, filters, hardware detection, preview decoder, audio decoder. |
| `src/player.rs`, `src/audio.rs` | Preview playback with the sound as master clock (`cpal`). |
| `src/thumbnails.rs` | The thumbnail strip, by parallel seeks. |
| `src/instance.rs` | One window: a second launch hands its files to the open one. |
| `src/sendto.rs` | The *Send to* shortcut. |
| `src/build_info.rs`, `build.rs` | The version, the commit, links to the documentation of that commit; the icon and version block. |
| `src/main.rs` | Arguments, the log file, the crash message, the window. |

## The filters

`ffmpeg::filters::graph` builds the decoder options and the `-vf` chain from the source, the target size
and frame rate, the tone-mapping curve and the sink (an encoder, or the RGBA preview).

- **libplacebo on Vulkan** when it runs (`Capabilities::placebo`): scaling (orthogonal Lanczos), frame
  rate (no frame mixing: every output frame is a source frame) and tone mapping with dynamic peak
  detection, since NVIDIA app captures carry no mastering metadata. The video is decoded by Vulkan when
  `vulkan_decodes` says it can (a one-frame test per video), else by `-hwaccel auto` and uploaded.
- **Software**: `fps`, `scale` and, for HDR, `zscale` to linear light at 100 nits, `tonemap`, `zscale`
  back to BT.709.

Color is always explicit. The source range comes from the file (NVIDIA HDR captures are full range); a
source with no color tag at all (Xbox captures synced through OneDrive) is declared BT.709 limited range by
`setparams`; the output is tagged BT.709 limited range, or BT.2020 PQ/HLG when HDR is kept.

A GPU path that fails falls back: the estimate retries without Vulkan decoding, then with the software
filters, and keeps the engine that worked; the preview switches to the software filters.

### Measurements (RTX 4090, FFmpeg 9.0.2, 2026-11-20)

| Pipeline, 4K60 AV1 HDR10 source → 720p30 SDR H.265 | Real time |
| --- | --- |
| Vulkan decode + libplacebo on the GPU | 4.4x (10 s in 2.26 s) |
| NVDEC to system memory + libplacebo | 2.3x |
| NVDEC to system memory + `zscale`/`tonemap` | 2.6x |
| Software AV1 decode alone (dav1d) | 3.7x |

| Encodes at once (same pipeline, 23.6 s clip) | Wall time | Throughput |
| --- | --- | --- |
| 1 | 4.5 s | 5.2x |
| 2 | 6.8 s | 7.0x |
| 4 | 13.0 s | 7.3x |
| 6 | 19.3 s | 7.3x |

Two encodes saturate the card (two NVENC engines). The engine count varies within a generation (one
NVENC on an RTX 3080, two on an RTX 4090, per NVIDIA's support matrix) and no tool reports it, so the
application measures it (`hardware::measure_parallel_encodes`): 1, 2, 3… encodes of a synthetic 1080p60
picture at once, until one more adds less than half of what one encode does alone. About 7 s, once per
GPU, driver and encoder; new estimates and encodes wait meanwhile, and the result sets *Encodes at once*
unless the user set it by hand. Measured 2026-11-23 with `hevc_nvenc` p7 on the RTX 4090: 146, 289 and
302 frames/s for 1, 2 and 3 encodes, so 2.

## Frame rates

The nominal rate comes from ffprobe's `r_frame_rate` only when it is a standard rate; game captures are
variable rate, and an Xbox HDR capture reported `r_frame_rate=56/1` for an average of 59.3 fps. Otherwise
the standard rate within 3 % of `avg_frame_rate` is taken (59.94 there), then the raw values
(`probe::nominal_rate`).

## The size fitting

1. **Budget.** The video bitrate that fills the limit once the audio (AAC, with 2 % framing) and the
   container (64 kB plus 1 %) are paid for, 4 % under the limit for the rate control's error
   (`plan::video_budget`).
2. **Samples.** Two to six samples spread over the kept part, each one keyframe interval long (4 s), so
   they carry the same share of keyframes as the encode; a kept part barely longer than the samples is
   encoded whole. Each sample is encoded at constant QP to the elementary stream on standard output, and
   its bytes counted: no temporary file.
3. **Model.** `bitrate(qp) = r0 · 2^(-(qp - q0) / k)`. A first measurement at the middle of the quality
   bounds, a second near the predicted answer, fit `k` for the video.
4. **Ladder.** The targets from the profile's maximums down to its minimums, by decreasing pixel rate
   (`plan::ladder`). When the top does not fit within the quality floor, the solver predicts the others
   from the bitrate's growth with the pixel rate (exponent 0.75, then fitted on the video's own
   measurements) and searches between the last failure and the best fit, so it settles on the best target
   that fits.
5. **Verdict.** Better than the ceiling: encode at the ceiling's bitrate, the file ends up smaller. Between
   the bounds: fill the budget. Below the floor even at the bottom of the ladder: fill the budget and say so.

The encode is NVENC VBR with full-resolution two-pass, peak at twice the target over a four-target buffer.
A file over the limit is re-encoded at the bitrate scaled by its overshoot, aiming 3 % under, at most twice.

Measured on real captures (2026-11-20, Messenger profile): a 23.6 s 4K60 AV1 HDR capture fitted at 720p30
at the ceiling, 17.6 MB predicted 19.4; a 59.8 s 1080p60 H.264 Xbox capture fitted at 720p29.97 QP 22.8,
24.72 MB predicted 23.99 (98.9 % of the limit, first attempt).

## The encoder

A profile lists the codecs its destination plays, preferred first (`Profile::codecs`).
`Capabilities::choose` takes, in order: the first codec the user's chosen backend encodes, when one is
chosen; else the first codec with any hardware encoder, on its best hardware backend (NVENC, Quick Sync,
AMF); else the first codec the CPU encodes. A GPU encode of a codec the destination plays always beats a
CPU encode of a preferred one: a laptop with AV1 on NVENC and H.265 on Quick Sync gets AV1 on NVENC for
`AV1 > H.265`, an RTX 3080 (no AV1 encoder) gets H.265 on NVENC. The chosen encoder's codec decides
whether HDR can be kept and the samples' elementary stream format.

The first AV1 encode measured (2026-11-23, `av1_nvenc`, a synthetic HDR clip to 1080p60 under 20 MB) came
out 0.03 % over the limit and was re-encoded once, to 97.4 % of it.

## Files written

Next to the videos, only the output and its partial file. FFmpeg writes `<output>.qvt-partial`, checked by
`naming::check_partial` (same folder, marker extension, not the source) before every run; it is renamed
once the size is checked, deleted otherwise. An output name that exists gets ` (2)`, ` (3)`…: nothing is
ever overwritten. Source metadata (titles, encoder tags, chapters) is not copied.

In the data folder (`eframe::storage_dir`): `profiles.toml`, eframe's saved preferences, the logs of the
last two runs (`quick-video-transcoder.log`, `.previous.log`) and `instance.txt`, the loopback port and
random token through which a second launch hands its files to the open window (`instance`). In the user's
*Send to* folder, `Quick Video Transcoder.lnk`, only when the user adds or removes it from the *File*
menu (`sendto`).

## Startup

FFmpeg is checked first (`ffmpeg::Install::check`): `ffmpeg` and `ffprobe` must run and be version 7 or
later. When they are not on the inherited `PATH`, the `PATH` is read again from the user's and the
system's settings, so FFmpeg installed after Explorer started is found without signing out. Then every
encoder FFmpeg was built with is tried in parallel with the options the estimate and the encode use
(constant QP, then the bitrate rate control with B-frames, lookahead and multipass) on a small picture:
a bare encode test passes on GPUs that then refuse those options. The parallel encodes are measured for
GPU encoders only; with a CPU encoder, one at a time.

## Releases

`build.rs` stamps the commit into the executable, with the icon and the version block (`rc.exe`).
`scripts/build.ps1 release` asks the built executable for its commit (`--version`), refuses one that
differs from the checkout, and packages the executable, the readme with its relative links turned into
links at that commit, the licence and the third-party notices (`cargo about`). `release.yml` runs it on a
`v*` tag, requires the changelog section of the version (`notes.md`), attests `SHA256SUMS.txt` and
publishes.
