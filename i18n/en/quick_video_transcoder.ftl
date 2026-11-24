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

# Heading of the settings panel when several videos are selected. $count is the number of videos (always 2 or more).
settings-heading-selected =
    { $count ->
        [one] { NUM($count) } video selected
       *[other] { NUM($count) } videos selected
    }
# Grey note under the heading when several videos are selected.
settings-change-applies-to-all = A change applies to each of them
# Heading of the settings panel when the queue has no selected video.
settings-no-video = No video selected

# Section headings of the settings panel.
settings-section-profile = Profile
settings-section-kept-part = Kept part
settings-section-estimate = Estimate
settings-section-output = Output

# Profile picker text when the video's settings were edited after choosing the profile. $name is the profile name.
settings-profile-changed = { $name } (changed)
# Button next to the profile picker: puts back the settings of the chosen profile.
settings-profile-reset = Reset
settings-profile-reset-tooltip = Back to the profile's settings
# Placeholder of the text field where the user names a profile to save.
settings-profile-name-hint = Profile name
# Button that saves the current settings as a profile; the "update" form shows when the typed name already exists.
settings-profile-save = Save as profile
settings-profile-update = Update profile
settings-profile-save-tooltip = Keep these settings as a profile for later videos

# Row labels of the settings grid.
settings-size-limit = Size limit
settings-size-limit-tooltip = Megabytes of 1,000,000 bytes
# The size limit in its number box ("25.0 MB"); megabytes of 1,000,000 bytes.
settings-size-value = { NUM($mb, decimals: 1) } MB
settings-resolution = Resolution
# Picker text for the resolution ceiling. $short is the short side of the picture in pixels, e.g. 720 (shown as 720p).
settings-resolution-up-to = up to { $short }p
settings-frame-rate = Frame rate
# Picker text for the frame rate ceiling. $fps is a number of frames per second.
settings-frame-rate-up-to = up to { NUM($fps) } fps
# A frame rate in a list of choices. $fps is a number of frames per second.
settings-fps-value = { NUM($fps) } fps
settings-codec = Codec
# Picker text for a codec list made of one codec. $codec is the codec name, e.g. "H.264".
settings-codec-only = { $codec } only
# Shown after the arrow in the codec picker when no installed encoder can encode the codec list.
settings-codec-no-encoder = no encoder
settings-codec-tooltip = The codecs the destination plays, preferred first: the first one your GPU encodes is used, else the first one the CPU encodes
settings-hdr = HDR
# Checkbox: converts HDR videos to SDR when encoding.
settings-hdr-convert = Convert to SDR
settings-hdr-convert-tooltip = Off: an HDR video stays HDR (10-bit); phones without HDR show it washed out
# Shown when the checkbox is disabled because the chosen codecs cannot carry HDR.
settings-hdr-convert-disabled-tooltip = H.264 cannot carry HDR
settings-tone-mapping = Tone mapping
settings-tone-mapping-disabled-tooltip = This video is SDR
settings-sound = Sound
settings-sound-auto-tooltip = The game's track: the first one whose name is not a microphone or a voice chat
# Sound choices. "Every track mixed" mixes all the audio tracks of the capture into one.
settings-sound-mix-all = Every track mixed
settings-sound-none = No sound
# The automatic choice when it picks a track that has a name. $title is the track's name, as written in the file.
settings-sound-auto-titled = Automatic: { $title }
# The automatic choice when the picked track has no name. $number is the track's position, starting at 1.
settings-sound-auto-numbered = Automatic: track { $number }
# The automatic choice when no track can be identified yet (no video analysed, or no audio).
settings-sound-auto-default = Automatic (the game's track)
# A track without a name in the sound picker. $number is its position, starting at 1.
settings-sound-track = Track { $number }
settings-sound-bitrate = Sound bitrate
# A bitrate of the sound or the video. $kbps is a number of kilobits per second.
settings-kbps-value = { NUM($kbps) } kbit/s

# Collapsible group of the settings grid.
settings-quality-bounds = Quality bounds
settings-best-worth-it = Best worth it
settings-best-worth-it-tooltip = Past this quality the extra bytes are not visible: an easy clip ends up smaller than the limit
settings-worst-accepted = Worst accepted
settings-worst-accepted-tooltip = Below this quality the frame rate, then the resolution, are lowered
# Value of a quality slider. QP is the quantization parameter (lower is better). $qp is its number, $word a short
# description of that quality, such as "good" (one of the estimate-quality-* messages, already translated).
settings-quality-value = QP { NUM($qp) } ({ $word })
settings-lowest-resolution = Lowest resolution
settings-lowest-frame-rate = Lowest frame rate
# Text added to the name of each output file.
settings-file-name-suffix = File name suffix

# Kept part (trim) section.
settings-kept-part-multi-note = The kept part is set video by video, on the timeline
settings-trim-start = Start
settings-trim-end = End
settings-trim-duration = Duration
settings-keep-everything = Keep everything
# Message in the log when the user typed something that is not a time in the start or end field. $text is what was typed.
settings-not-a-time = not a time: { $text }

# Estimate section: a sentence above the details, depending on how the encode will fit.
settings-verdict-best-quality = Fits at the best quality worth it: the file will be smaller than the limit
settings-verdict-fitted = Fills the size limit
settings-verdict-below-floor = Below the worst accepted quality even at the lowest settings: shorten the kept part
settings-estimate-output = Output
# Planned output. $size is the picture size like "1280x720", $fps a frame rate number, $target a short description of the planned settings.
settings-estimate-output-value = { $size } at { $fps } fps ({ $target })
settings-estimate-video-bitrate = Video bitrate
settings-estimate-quality = Quality
# $word is a short description of the quality, $qp the quantization parameter (lower is better).
settings-estimate-quality-value = { $word } (QP { NUM($qp, decimals: 1) })
settings-estimate-size = Size
# $predicted is the expected size of the file and $limit the size limit, both in megabytes.
settings-estimate-size-value = ≈ { NUM($predicted, decimals: 1) } MB of { NUM($limit, decimals: 1) } MB
# Label of the number of short test encodes the estimate is based on.
settings-estimate-measured = Measured
settings-estimate-measured-value =
    { $count ->
        [one] { NUM($count) } sample encode
       *[other] { NUM($count) } sample encodes
    }
settings-estimating = Estimating…
# Shown while estimating: the video bitrate available, in kilobits per second.
settings-estimate-budget = Budget: { NUM($kbps) } kbit/s of video
# Small grey line. $engine is the name of the filter engine (data, not translated).
settings-estimate-filters = Filters: { $engine }

# Output section: where the files go, and the encoder settings.
# Shown in place of the output folder when there is none: each output goes in its source's folder.
settings-output-next-to-video = Next to each video
# Button that goes back to writing each output next to its source (enabled when a folder is chosen).
settings-output-use-next-to-video = Next to each video
settings-output-choose-folder = Choose folder…
settings-output-folder-dialog-title = Output folder
settings-encoder = Encoder
settings-encoder-automatic = Automatic
settings-parallel-encodes = Encodes at once
# $count is the number of encodes the GPU runs at once before they slow each other down.
settings-parallel-measured-tooltip = Measured on this GPU: { $count } at once before encodes slow each other down (an RTX 4090 has two NVENC engines, an RTX 3080 one)
settings-parallel-not-measured-tooltip = Not measured yet
# Small grey status under the "Encodes at once" slider, in lower case on purpose.
settings-parallel-measuring = measuring…
# $count is the measured number of simultaneous encodes.
settings-parallel-measured = measured: { $count }
settings-parallel-not-measured = not measured
# Small button: sets the slider back to the measured number.
settings-parallel-use-measured = Use it
settings-parallel-measure-again = Measure again
settings-parallel-measure-again-tooltip = A few seconds; new estimates and encodes wait meanwhile

# Profiles window (also in src/app/settings.rs).
profiles-title = Profiles
profiles-intro = The first profile is given to new videos. A profile is changed by saving a video's settings under its name.
# One line summarizing a profile in the list. $mb is the size limit in megabytes, $short the short side in pixels
# (720 gives 720p), $fps the highest frame rate (written right after the "p"), $codecs the codec list (data).
profiles-summary = { NUM($mb) } MB · { $short }p{ NUM($fps) } · { $codecs }
# Same as profiles-summary for a profile that converts HDR videos to SDR.
profiles-summary-sdr = { NUM($mb) } MB · { $short }p{ NUM($fps) } · { $codecs } · SDR
profiles-move-up-tooltip = Move up
profiles-delete-tooltip = Delete
# Menu button listing the profiles that came with the application and were deleted from the list.
profiles-add-built-in = Add a built-in profile

## Preview (src/app/preview.rs)

# Overlay on the preview. $size is the preview picture size like "1280x720", $width and $height the source size in
# pixels, $codec the source codec name (data).
preview-info = Preview { $size } · source { $width }x{ $height } { $codec }
# Same for an HDR source encoded to SDR. $dynamic is the HDR format name (data).
preview-info-hdr-encoded = Preview { $size } · source { $width }x{ $height } { $codec } · { $dynamic } tone mapped as encoded
# Same for an HDR source kept as HDR in the output: the preview cannot show HDR, so it is tone mapped.
preview-info-hdr-kept = Preview { $size } · source { $width }x{ $height } { $codec } · { $dynamic } kept HDR, shown tone mapped
preview-seeking = Seeking…
# $detail is the technical error of the video decoder (English).
preview-decoder-error = Decoder: { $detail }
# Centered texts of the preview when there is no picture.
preview-empty-queue = Drop game captures here, or add them with Ctrl+O
preview-decoding = Decoding…
preview-select-video = Select a video

## Queue panel (src/app/queue_panel.rs)

# Heading of the left panel.
queue-panel-heading = Queue
# Button at the top of the queue; a "➕ " symbol is put before this text.
queue-panel-add = Add…
queue-panel-add-tooltip = Add videos (Ctrl+O), or drop them here
# Shown in the empty queue, in two lines (this one is the larger).
queue-panel-empty-title = Drop videos here
queue-panel-empty-hint = or use Explorer's Send to menu
# Details line of a queue row: video summary (resolution, frame rate…), kept duration, profile name.
queue-panel-row-details = { $summary } · { $duration } · { $profile }
# Kept duration of a trimmed video: both are time codes (e.g. "0:12" of "1:05").
queue-panel-duration-trimmed = { $kept } of { $total }
queue-panel-reading = Reading…
queue-panel-estimating = Estimating the size…
queue-panel-waiting = Waiting for an encoder…
# Small button in a row that waits or encodes: stops that video.
queue-panel-cancel = Cancel
# Progress text in a row while encoding. $percent is 0 to 100; $speed is the encoding speed relative to
# the video's duration (2.5 means 2.5 times faster than playback), 0 when unknown; $retry is the number of
# retries so far, 0 for the first attempt.
queue-panel-progress =
    { NUM($percent) } %{ $speed ->
        [0] { "" }
       *[other] { " · " }{ NUM($speed, decimals: 1) }x
    }{ $retry ->
        [0] { "" }
       *[other] { " · " }retry { NUM($retry) }
    }
# Size of an encoded video in a row; a "✔ " symbol is put before this text.
queue-panel-done-size = { NUM($mb, decimals: 2) } MB
# Small button in a row of an encoded video: copies the file to the clipboard.
queue-panel-copy = Copy
queue-panel-copy-tooltip = Copy the file, to paste it into the conversation
# Small button in a row of an encoded video: opens its folder in Explorer.
queue-panel-show = Show
queue-panel-show-tooltip = Show in Explorer
# Estimate of a video's output. $target is the size and frame rate it will be encoded at, $mb the
# predicted file size, $quality a word for the quality, $queued is "yes" when the video is already queued
# for encoding, else "no".
queue-panel-estimate =
    { $target } · ≈ { NUM($mb, decimals: 1) } MB · { $quality }{ $queued ->
        [yes] { " · " }queued
       *[other] { "" }
    }
# Shown in a row while the estimate is being recomputed after a change of settings.
queue-panel-settings-changed = Settings changed…
# Button below the queue; a "▶ " symbol is put before this text. $count: how many selected videos can be encoded.
queue-panel-encode-selected = Encode selected ({ NUM($count) })
queue-panel-encode-selected-tooltip = Encode the selected videos with their estimate (Ctrl+E)
# Button below the queue. $count: how many videos can be encoded.
queue-panel-encode-all = Encode all ({ NUM($count) })
# Button below the queue; a "⏹ " symbol is put before this text. Stops every queued and running encode.
queue-panel-stop-all = Stop all
# Button below the queue. $count: how many videos are encoded.
queue-panel-copy-encoded = Copy encoded ({ NUM($count) })
queue-panel-copy-encoded-tooltip = Copy every encoded file, to paste them into a conversation

## Main window: menus, status bar, log, shortcuts, dialogs (src/app/mod.rs)

# Menu bar. Shortcuts written in a label (Ctrl+O, Del) use the key names of the language's keyboards.
menu-file = File
menu-file-add-videos = Add videos…  (Ctrl+O)
menu-file-remove-selected = Remove selected  (Del)
menu-file-remove-encoded = Remove encoded videos
menu-file-quit = Quit
menu-profiles = Profiles
menu-profiles-edit = Edit profiles…
# Opens the folder that holds the profiles file, named profiles.toml, in Explorer.
menu-profiles-open-folder = Open profiles.toml folder
menu-view = View
# Checkbox that shows or hides the log panel.
menu-view-log = Log
menu-help = Help
menu-help-shortcuts = Keyboard shortcuts
menu-help-documentation = Documentation
menu-help-about = About Quick Video Transcoder

# Bottom status bar. $total is the number of videos in the queue; the other counts are how many of them
# are being encoded (or wait for an encoder), being estimated, and encoded.
status-summary =
    { $total ->
        [one] { NUM($total) } video
       *[other] { NUM($total) } videos
    } · { NUM($encoding) } encoding · { NUM($estimating) } estimating · { NUM($done) } done
# Next to a spinner, while FFmpeg's encoders are being detected at start.
status-detecting = Detecting the encoders…

# Window with the list of keyboard shortcuts: a title, then one line per shortcut made of the keys (left)
# and what they do (right). Key names follow the language's keyboards (Space, Home, Page Up / Down…);
# letters (I, O, M) do not change.
shortcuts-title = Keyboard shortcuts
shortcuts-play-keys = Space
shortcuts-play-action = Play / pause the kept part
shortcuts-step-keys = Left / Right
shortcuts-step-action = Previous / next frame
shortcuts-second-keys = Shift + Left / Right
shortcuts-second-action = -1 s / +1 s
shortcuts-ten-seconds-keys = Page Up / Down
shortcuts-ten-seconds-action = -10 s / +10 s
shortcuts-trim-keys = I / O
shortcuts-trim-action = Start / end of the kept part at the current frame
shortcuts-goto-keys = Home / End
shortcuts-goto-action = Go to the start / end of the kept part
shortcuts-mute-keys = M
shortcuts-mute-action = Mute / unmute
shortcuts-add-keys = Ctrl + O
shortcuts-add-action = Add videos
shortcuts-select-all-keys = Ctrl + A
shortcuts-select-all-action = Select every video
shortcuts-encode-keys = Ctrl + E
shortcuts-encode-action = Encode the selected videos
shortcuts-delete-keys = Delete
shortcuts-delete-action = Remove the selected videos
shortcuts-handle-keys = Drag a handle
shortcuts-handle-action = Move the start or end of the kept part
shortcuts-shift-drag-keys = Shift + drag
shortcuts-shift-drag-action = Move the kept part as a whole

# Title of the window. $total is the number of videos in the queue.
title-idle =
    Quick Video Transcoder — { $total ->
        [one] { NUM($total) } video
       *[other] { NUM($total) } videos
    }
# Title of the window while encoding. $count: videos being encoded; $percent: their average progress.
title-encoding = Quick Video Transcoder — encoding { NUM($count) } ({ NUM($percent) } %)

# File dialog opened by "Add videos": its title and the names of its two file filters.
dialog-add-title = Add videos
dialog-add-filter-videos = Videos
dialog-add-filter-all = All files
# Question asked when the window is closed while videos are being encoded (Yes / No buttons).
dialog-close-title = Encodes running
dialog-close-description = Closing stops the running encodes and deletes their partial files. Close anyway?

# Lines of the log panel (and of the status bar). $detail is a technical error text, kept in English.
# $path: a file or folder path; $program: the name of a program (ffmpeg, ffprobe); $version: FFmpeg's version.
app-profiles-load-failed = { $detail }; using the built-in profile
app-folder-not-video = { $path } is a folder, not a video
app-files-copied =
    { $count ->
        [one] { NUM($count) } file
       *[other] { NUM($count) } files
    } copied: paste them into the conversation
app-profiles-no-folder = no data folder: the profiles cannot be saved
app-profiles-saved = profiles saved to { $path }
app-ffmpeg-missing = { $program } is not on the PATH
# $required: the oldest FFmpeg major version that works.
app-ffmpeg-too-old = { $version }: FFmpeg { NUM($required) } or later is needed
# $summary: the detected encoders, as a technical list.
app-hardware = hardware: { $summary }
app-no-encoder = FFmpeg runs, but none of its video encoders works on this computer
# $encoder: an encoder's name, for example "H.265 (NVENC)".
app-measuring = measuring how many encodes { $encoder } runs at once…
app-measure-failed = the encoder failed while measuring its parallel encodes
# $key: the graphics card, its driver and the encoder; $count: the number of encodes at once.
app-measured =
    { $key }: { $count ->
        [one] { NUM($count) } encode
       *[other] { NUM($count) } encodes
    } at once

## Help windows: About, FFmpeg, Send to (src/app/help.rs)

# Log line when a later launch of the application (Explorer's Send to) hands over files.
app-files-received =
    { $count ->
        [one] { NUM($count) } file
       *[other] { NUM($count) } files
    } received from Send to

# File menu items that add the application to Explorer's "Send to" menu (the shortcut lives in the
# user's SendTo folder).
sendto-menu-add = Add to Explorer's Send to menu
sendto-menu-update = Update the Send to shortcut
sendto-menu-tooltip = Select videos in Explorer, right-click, Send to, Quick Video Transcoder
sendto-menu-remove = Remove from Explorer's Send to menu
# $path: where the shortcut was written.
sendto-menu-written = Send to shortcut written: { $path }
sendto-menu-removed = Send to shortcut removed

# About window.
about-title = About Quick Video Transcoder
# $version: the version number.
about-heading = Quick Video Transcoder { $version }
about-summary = Trims game captures and re-encodes them to fit a size limit.
# Label of a row; its value is the short code of the build's source commit.
about-commit = Commit
about-documentation = Documentation
# Link text, after the "Documentation" label.
about-documentation-link = for this version
# Label of a row; its value is a link to the change history file (CHANGELOG.md).
about-changes = Changes
about-licence = License
about-built-with = Built with
about-third-party-link = third-party libraries and their licenses
about-notices-no-folder = no data folder to open the third-party notices from
# $path: the file that could not be written; $detail: the technical reason.
about-notices-failed = cannot write { $path }: { $detail }

# Window shown when FFmpeg (the free program that reads and encodes videos) is missing or too old.
ffmpeg-title = FFmpeg is needed
# $program: the name of the program that was not found (ffmpeg or ffprobe).
ffmpeg-missing = Quick Video Transcoder reads and encodes videos with FFmpeg, a free program it does not include, and { $program } cannot be found on this computer.
# $version: the version found; $required: the oldest major version that works.
ffmpeg-too-old = The FFmpeg found is too old ({ $version }). Version { NUM($required) } or later is needed.
ffmpeg-installing = Installing FFmpeg with winget: a minute or two…
# winget is Windows' package manager. $package: the winget package identifier.
ffmpeg-winget-offer = It can be installed for you with winget, Windows' package manager: the { $package } package (gyan.dev's full build, about 100 MB, for your account only).
ffmpeg-install-button = Install FFmpeg for me
ffmpeg-licence-note = Installing accepts the package's license (GPL) and the winget source's terms.
# Introduces the manual steps below it.
ffmpeg-or-yourself = Or install it yourself:
ffmpeg-looking-for-winget = Looking for winget…
# Before a command to type in Windows Terminal; the command follows, then a "Copy" button.
ffmpeg-in-terminal = In Terminal:
ffmpeg-copy = Copy
# Followed by a link to the download page, on the same line. "release full" is the name of a build on
# that page.
ffmpeg-download = Or download the "release full" build from
ffmpeg-unzip = unzip it, and add its bin folder to the Path environment variable.
ffmpeg-check-again = Check again
# Log line. $package: the winget package identifier.
ffmpeg-install-log = installing FFmpeg with winget ({ $package })…

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
estimate-too-small = { NUM($mb, decimals: 0) } MB cannot even hold the sound of { $duration }: shorten the kept part
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
        [one] { NUM($count) } time
       *[other] { NUM($count) } times
    }: lower the quality floor or shorten the kept part
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

## Updates window (src/app/update_window.rs)

# Help menu item: opens the updates window and checks GitHub at once (the only time the app connects).
menu-help-updates = Check for updates…
# Title of the updates window.
update-title = Updates
# First line of the window. $version: the running version, like 1.0.0 (data).
update-running = This is Quick Video Transcoder { $version }.
update-check = Check for updates
update-check-again = Check again
update-checking = Checking for updates…
# $version: the running version, which is the latest.
update-up-to-date = { $version } is the latest version.
# $version: the newer version found. Above the release notes (shown in English, from the changelog).
update-available = Version { $version } is available
# Button. $mb: the executable's size in megabytes.
update-download = Download and install ({ NUM($mb, decimals: 1) } MB)
# Link to the release on GitHub, in the browser.
update-release-page = See it on GitHub
update-downloading = Downloading { $version }…
# Shown once the file is downloaded and its SHA-256 checked against the release's.
update-ready = { $version } is downloaded and verified. Restarting installs it.
update-restart = Restart now
# Under the disabled restart button while videos are encoding (a restart would stop them).
update-wait-for-encodes = Available once the encodes are finished or stopped.
update-restarting = Installing { $version }: the application closes and comes back…
# Shown at the first start after an update. $version: the new version.
update-updated = Updated to { $version }.
update-see-log = Details are in the log (View → Log).
update-retry = Try again
update-fault-connection = Could not reach GitHub: no connection, or it did not answer.
# $status: the HTTP status GitHub answered with, like 503 (data).
update-fault-status = GitHub answered with an error ({ $status }). Try again later.
update-fault-unexpected = GitHub's answer was not what this version expects. See the release page.
update-fault-not-verified = The downloaded file does not match the published one: it was deleted. Try again.
# The executable's folder is read-only (for instance under Program Files): the user can download it by hand.
update-fault-cannot-write = This copy's folder cannot be written. Download the new version from the release page instead.
# $detail: why the install failed (technical, English: a Windows or PowerShell message).
update-fault-install = The update could not be installed: { $detail }
# GitHub refused because its hourly limit (60 checks per internet connection, shared by everyone on it) is
# reached. $minutes: minutes until it resets.
update-fault-rate-limited =
    GitHub limits checks to 60 an hour per internet connection, and that limit is reached. Try again in { $minutes ->
        [one] { NUM($minutes) } minute
       *[other] { NUM($minutes) } minutes
    }.
update-fault-rate-limited-later = GitHub limits checks to 60 an hour per internet connection, and that limit is reached. Try again later.
# Under Restart now when videos are in the queue: the queue is not kept across the restart.
update-queue-cleared =
    Restarting clears the queue ({ $count ->
        [one] { NUM($count) } video
       *[other] { NUM($count) } videos
    }): their trims and settings are not kept.
