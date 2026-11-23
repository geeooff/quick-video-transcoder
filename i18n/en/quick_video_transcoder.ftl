# Quick Video Transcoder — English messages: the reference every other language falls back to.
#
# Conventions (see docs/localization.md):
# - Ids are `area-element[-detail]`, lower case with hyphens; `-tooltip` for hover texts.
# - Numbers always go through NUM: { NUM($mb, decimals: 1) } MB. Never format a number in Rust.
# - Variables are named for what they hold ($name, $count, $mb…); plurals use selectors on $count.
# - Keyboard keys are written as the user's keyboard names them (Home, Space…) and translated with the text.
# - Each section below belongs to one source area; keep new messages in their area.

## Language menu (src/app/mod.rs)

menu-language = Language
language-automatic = Automatic (system language)

## Timeline and transport (src/app/timeline.rs)

timeline-first-tooltip = Start of the kept part (Home)
timeline-previous-tooltip = Previous frame (Left)
timeline-play-tooltip = Play / pause the kept part (Space)
timeline-next-tooltip = Next frame (Right)
timeline-last-tooltip = End of the kept part (End)
timeline-start-here = [ Start here
timeline-start-here-tooltip = Start the kept part at the current frame (I)
timeline-end-here = End here ]
timeline-end-here-tooltip = End the kept part after the current frame (O)
timeline-volume-tooltip = Volume { NUM($percent) }%
timeline-mute-tooltip = Mute / unmute (M)
timeline-no-sound = No sound

## Settings panel (src/app/settings.rs)

## Preview (src/app/preview.rs)

## Queue panel (src/app/queue_panel.rs)

## Main window: menus, status bar, log, shortcuts, dialogs (src/app/mod.rs)

## Help windows: About, FFmpeg, Send to (src/app/help.rs)

## Queue, estimate and encode messages (src/queue.rs, src/estimate.rs, src/encode.rs)

# Log line when a video has been read. $name: file name. $summary: one line such as "3840x2160 60 fps AV1
# HDR10". $duration: a time code. $tracks: number of audio tracks. $color: the file's color tags as FFmpeg
# names them (data). $engine: how the filters run (a media-engine-* text).
queue-probed =
    { $name }: { $summary }, { $duration }, { $tracks ->
        [one] { NUM($tracks) } audio track
       *[other] { NUM($tracks) } audio tracks
    }, color { $color }; { $engine }
# Log line when a video cannot be used. $name: file name. $detail: why (already a sentence or a technical text).
queue-job-error = { $name }: { $detail }
# Log line when a size estimate failed. $detail: the technical cause.
queue-estimate-failed = { $name }: estimate failed: { $detail }
# Log line when the filters of a video fall back to another engine. $engine: a media-engine-* text.
queue-engine-changed = { $name }: now using { $engine }
# Log line when an encode finished. $path: the output file. $mb: its size in megabytes. $seconds: how long it took.
queue-encoded = { $name }: encoded to { $path } ({ NUM($mb, decimals: 2) } MB in { NUM($seconds, decimals: 1) } s)
# Log line when an encode failed. $detail: the technical cause.
queue-encode-failed = { $name }: encode failed: { $detail }
# Shown in the queue row when the user asked to encode a video whose estimate failed. $detail: the cause.
queue-no-estimate = cannot encode without an estimate: { $detail }

# Words describing the predicted quality of a video, shown in the queue row and the settings, next to the
# size: "720p30 · ≈ 24.5 MB · good". Lowercase, one word or two.
estimate-quality-excellent = excellent
estimate-quality-very-good = very good
estimate-quality-good = good
estimate-quality-fair = fair
estimate-quality-poor = poor
# Estimate errors, shown in the queue row.
estimate-no-size = no output size to try
estimate-empty-range = the trimmed range is empty
# $mb: the size limit in megabytes. $duration: the kept part's length as a time code.
estimate-too-small = { NUM($mb, decimals: 0) } MB cannot even hold the sound of { $duration }: shorten the clip
# $status: FFmpeg's exit status (data). $detail: FFmpeg's own message (technical, English).
estimate-sample-failed = sample encode failed ({ $status }): { $detail }
estimate-sample-failed-silent = sample encode failed ({ $status }): no output

# Encode errors, shown in the queue row. $path: a file path. $status: FFmpeg's exit status (data).
# $detail: FFmpeg's own message (technical, English).
encode-output-exists = { $path } already exists
encode-cannot-read = cannot read { $path }
encode-cannot-rename = cannot rename the encode to { $path }
# $mb: the size limit in megabytes. $count: how many times the encode was tried.
encode-overshoot =
    the encoder overshot { NUM($mb, decimals: 1) } MB { $count ->
        [one] 1 time
       *[other] { NUM($count) } times
    }: lower the quality floor or shorten the clip
encode-ffmpeg-failed = ffmpeg failed ({ $status }): { $detail }
encode-ffmpeg-failed-silent = ffmpeg failed ({ $status }): no message
# Safety checks: the encode only writes its own partial file and never the source.
encode-refuse-partial = refusing to write { $path }: not a partial file
encode-refuse-source = refusing to write over the source { $path }

## Profiles and settings values (src/profile.rs, src/plan.rs)

# Tone mapping curves (HDR to SDR conversion), in the Tone mapping menu. Curve names are technical terms.
profile-tone-auto = Automatic (libplacebo)
profile-tone-bt2390 = BT.2390
profile-tone-spline = Spline
profile-tone-hable = Hable (filmic)
profile-tone-mobius = Möbius
# Errors reading or writing the profiles file. $path: the file's path.
profile-invalid-file = invalid profiles file
profile-cannot-read = cannot read { $path }
profile-cannot-create = cannot create { $path }
profile-cannot-write = cannot write { $path }
profile-cannot-replace = cannot replace { $path }

# A frame rate as a number, without the unit: a whole rate (60) or a fraction rounded to two decimals (29.97).
plan-rate-whole = { NUM($fps, decimals: 0) }
plan-rate-fraction = { NUM($fps, decimals: 2) }

## Media details (src/ffmpeg/)

# Errors about the FFmpeg tools, shown in the queue row or the log.
media-ffmpeg-start-failed = cannot start ffmpeg (is FFmpeg on the PATH?)
media-ffprobe-start-failed = cannot run ffprobe (is FFmpeg on the PATH?)
# $detail: ffprobe's own message (technical, English).
media-ffprobe-failed = ffprobe failed: { $detail }
media-probe-invalid = invalid ffprobe output
media-no-video = no video stream
media-no-dimensions = video stream without dimensions
# $status: FFmpeg's exit status (data). $detail: FFmpeg's own message (technical, English).
media-no-frame = no frame ({ $status }): { $detail }

# How the filters run, shown as "Filters: ..." in the settings and in the log. Vulkan and libplacebo are
# technical names.
media-engine-vulkan = Vulkan decode + libplacebo
media-engine-placebo = libplacebo
media-engine-software = software filters

# Dynamic range of a video, in the preview line and the queue row. Standard names; usually kept as is.
media-dynamic-sdr = SDR
media-dynamic-pq = HDR10
media-dynamic-hlg = HLG

# Channel layout of an audio track. $channels: the channel count.
media-audio-layout =
    { $channels ->
        [1] mono
        [2] stereo
        [6] 5.1
        [8] 7.1
       *[other] { NUM($channels) } channels
    }
# An audio track in a menu, without and with the title the file gives it ("Microphone"). $number: its
# position (1, 2…). $codec: codec name (data). $layout: a media-audio-layout text. $title: the file's text (data).
media-audio-track = { NUM($number) } · { $codec }, { $layout }
media-audio-track-titled = { NUM($number) } · { $title } ({ $codec }, { $layout })

# One line describing a source video in the log and the queue. $width, $height: picture size in pixels
# (text, no digit grouping). $fps: frame rate text such as "60" or "29.97". $codec: codec name (data).
# $dynamic: a media-dynamic-* text.
media-summary = { $width }x{ $height } { $fps } fps { $codec } { $dynamic }

# Log line listing what this computer can encode with. $encoders: such as "NVENC H.265/H.264, CPU H.265",
# or media-hardware-none. $placebo: "yes" or "no", whether GPU tone mapping works.
media-hardware-summary =
    encoders: { $encoders }; GPU tone mapping (libplacebo): { $placebo ->
        [yes] yes
       *[no] no
    }
media-hardware-none = none

# Errors installing FFmpeg with winget, shown in the FFmpeg window. $status: exit status (data). $detail: winget's last lines.
media-winget-start-failed = cannot start winget
media-winget-failed = winget failed ({ $status }): { $detail }

## Startup, Send to, clipboard (src/main.rs, src/sendto.rs, src/clipboard.rs)

# Message box when the window cannot open: title, then body. $detail: the technical cause (English).
startup-window-failed-title = Quick Video Transcoder cannot open its window
startup-window-failed =
    { $detail }

    The window needs a graphics driver with DirectX 12 or Vulkan. Updating the graphics driver usually fixes this.
# Message box after a crash: title, then body. $detail: the panic message (English).
startup-stopped-title = Quick Video Transcoder has stopped
startup-stopped =
    Something went wrong: { $detail }

    Encodes that were running did not finish.
# Last paragraph of those message boxes. $path: the log file.
startup-log-hint = Details are in { $path }

# Errors of the Explorer "Send to" shortcut. $path: the shortcut's path. $detail: PowerShell's message (technical).
sendto-no-folder = no Send to folder on this system
sendto-no-executable = cannot find this executable
sendto-no-exe-folder = the executable has no folder
sendto-start-failed = cannot start PowerShell
sendto-create-failed = cannot create { $path }: { $detail }
sendto-delete-failed = cannot delete { $path }
# The description stored in the shortcut (Explorer shows it in the shortcut's properties).
sendto-shortcut-description = Trim and re-encode videos to fit a size limit

# Errors copying files to the clipboard. $detail: the system's message (technical).
clipboard-copy-failed = cannot copy to the clipboard: { $detail }
clipboard-unsupported = copying files to the clipboard is only supported on Windows
