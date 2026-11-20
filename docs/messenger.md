# Messenger's limits for videos

**TL;DR** — The built-in *Messenger* profile keeps the settings used for years: H.265, 720p, 30 fps,
under 25 MB. Meta documents files up to 100 MB in a chat since April 2024, and says nothing official about
the resolution, frame rate or codec a sent video keeps. Whether 1080p60 survives, and whether an AV1 file
is re-encoded, can only be settled by measuring what the other side receives:
[How to measure it](#how-to-measure-it).

Researched on 2026-11-20.

## What Meta says

- **Files up to 100 MB** in a Messenger chat, announced on 9 April 2024 with HD photos
  ([Meta newsroom](https://about.fb.com/news/2024/04/hd-photos-shared-albums-and-more-on-messenger/)).
  The post speaks of files in general and names Word, PDF and Excel; it does not say whether a video sent
  as a video (played in the conversation) follows the same limit as a file attachment.
- **HD video is 720p**, from the 2018 announcement of HD video in Messenger
  ([Meta newsroom](https://about.fb.com/news/2018/04/with-360-degree-photos-and-hd-quality-video-messenger-gets-even-more-visual)).
  The 2024 HD toggle is for photos only.
- Nothing about the frame rate, the codecs accepted, or whether a sent video is re-encoded. The Messenger
  Platform's Send API documentation gives no video limit either.

## What others claim, unverified

- Several compression tools' pages say 25 MB was the video limit until it rose to 100 MB, in April 2024
  ([VidKit](https://vidkit.eu/messenger-video-size-limit/)) or April 2025
  ([CompressYourVideo](https://compressyourvideo.com/blog/compress-video-for-facebook-messenger)); they
  disagree on the date and cite no Meta source.
- The same pages say sent videos are recompressed for delivery, and that Messenger handles H.264 better than
  H.265. No source, and nothing about AV1.

## Why the profile stays at 720p30 / 25 MB

It is what has worked, sent from the desktop, for a long time; 1080p60 and a 100 MB limit are plausible
but unproven for a video message. A profile is a few clicks away: raise *Size limit*, *Resolution* and
*Frame rate* on a video, then *Save as profile*.

The size fitting already uses what a higher limit allows: a one-minute Xbox capture (1080p60 H.264 SDR)
fits 25 MB at 720p30 with a QP of 22.8, "very good", which suggests 1080p30 at 25 MB would still look good.

## How to measure it

1. Pick one busy capture, about a minute long.
2. Encode it three times with profiles that differ by one setting: 720p30 H.265 under 25 MB (the
   reference), 1080p60 H.265 under 25 MB, and 1080p60 AV1 under 25 MB. Then one at 1080p60 H.265 under
   90 MB for the size limit.
3. Send each to a test conversation from the desktop or web app, and note which ones are refused.
4. On the receiving side, save each video (*Save* in the conversation's media) and run
   `ffprobe -v error -show_entries stream=codec_name,width,height,avg_frame_rate,bit_rate -of compact <file>`.
5. Received codec, size, frame rate and bitrate equal to what was sent: kept as is. Different: re-encoded,
   and the received bitrate says how hard. Compare the received AV1 and H.265 files frame by frame at the
   same time code to see which arrives better.

Record the results here, dated, with the app version used.
