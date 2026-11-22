# Discord's limits for videos

**TL;DR** — Discord limits each file to 20 MB on a free account, 50 MB with Nitro Basic, 1 GB with
Nitro, and 50 MB or 100 MB for everyone in a server at boost level 2 or 3. It accepts MP4 in H.264, HEVC
or AV1. The built-in *Discord* profiles follow these limits: H.265 up to 1080p 60 fps (2160p with Nitro),
SDR.

Researched on 2026-11-23, from Discord's own help pages.

## What Discord says

- **Upload limits** ([File Attachments FAQ](https://support.discord.com/hc/en-us/articles/25444343291031-File-Attachments-FAQ)):
  20 MB without Nitro since August 2026 (it was 10 MB), 50 MB with Nitro Basic, "up to 1GB" with Nitro.
  Discord says it is "experimenting with various file upload size limits", so some accounts may see
  larger ones.
- **Nitro went from 500 MB to 1 GB in 2026**
  ([Nitro's 2026 recap](https://discord.com/blog/nitros-biggest-year-yet-a-look-back-at-2026); the
  [Nitro page](https://discord.com/nitro) lists 50 MB for Nitro Basic and 1 GB for Nitro).
- **Server boosts** ([Server Boosting FAQ](https://support.discord.com/hc/en-us/articles/360028038352-Server-Boosting-FAQ)):
  level 2 (7 boosts) gives every member a 50 MB limit in that server, level 3 (14 boosts) 100 MB. Only
  in that server: a direct message keeps the sender's own limit.
- **Formats**: the same FAQ lists "MP4 (Video encoded with H264, HEVC/H265, or AV1)" and MOV among the
  common file types.
- **Size check before compression**: the size counted is the file's, before anything Discord does to it,
  on desktop and mobile alike. The mobile app can compress videos it uploads (its *Video Uploads* and
  *Data Saving Mode* settings); the desktop app sends the file as is.

Discord documents no resolution, frame rate or length limit, and does not say whether it transcodes the
videos it plays back.

## The built-in profiles

| Profile | Limit | Picture | Sound |
| --- | --- | --- | --- |
| Discord (20 MB) | 20 MB | up to 1080p 60 fps | 128 kbit/s |
| Discord Nitro Basic / server level 2 (50 MB) | 50 MB | up to 1080p 60 fps | 160 kbit/s |
| Discord server level 3 (100 MB) | 100 MB | up to 1080p 60 fps | 160 kbit/s |
| Discord Nitro (1 GB) | 1,000 MB | up to 2160p 60 fps | 192 kbit/s |

All are H.265 and SDR. The size fitting lowers the frame rate, then the resolution, when a long clip would
otherwise look poor; with Nitro, most clips fit at the best quality worth it and end up far below 1 GB.

**Pick the profile of whoever sends**: the limit is the sender's (or the server's, at boost level 2 or 3).

**H.265 and compatibility.** Discord accepts HEVC, but whether a friend's device plays it inline depends on
that device: recent Windows and macOS desktop apps and phones do, some browsers and Linux builds do not.
If a friend sees a download instead of a video, set the codec to H.264 on that video (about 30 % larger
for the same picture) and save it as a profile if it happens often.

HDR is converted to SDR: Discord says nothing of HDR playback, and most viewers would see an HDR video
washed out.
