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
# Unit shown after the size limit value (a number box: "25 MB"), megabytes of 1,000,000 bytes.
settings-unit-mb = MB
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
# description of that quality, such as "good" (English words from the estimate, not yet translated).
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
settings-output-next-to-video = Next to each video
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

## Main window: menus, status bar, log, shortcuts, dialogs (src/app/mod.rs)

## Help windows: About, FFmpeg, Send to (src/app/help.rs)

## Queue, estimate and encode messages (src/queue.rs, src/estimate.rs, src/encode.rs)

## Profiles and settings values (src/profile.rs, src/plan.rs)

## Media details (src/ffmpeg/)

## Startup, Send to, clipboard (src/main.rs, src/sendto.rs, src/clipboard.rs)
