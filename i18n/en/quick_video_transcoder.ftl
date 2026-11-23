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
about-licence = Licence
about-built-with = Built with
about-third-party-link = third-party libraries and their licences
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
ffmpeg-licence-note = Installing accepts the package's licence (GPL) and the winget source's terms.
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

## Profiles and settings values (src/profile.rs, src/plan.rs)

## Media details (src/ffmpeg/)

## Startup, Send to, clipboard (src/main.rs, src/sendto.rs, src/clipboard.rs)
