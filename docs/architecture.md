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

Two encodes saturate the card (two NVENC engines): the default is two.

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

## Files written

Only the output and its partial file. FFmpeg writes `<output>.qvt-partial`, checked by
`naming::check_partial` (same folder, marker extension, not the source) before every run; it is renamed
once the size is checked, deleted otherwise. An output name that exists gets ` (2)`, ` (3)`…: nothing is
ever overwritten. Source metadata (titles, encoder tags, chapters) is not copied.
