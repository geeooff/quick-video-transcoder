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

## Profiles and settings values (src/profile.rs, src/plan.rs)

## Media details (src/ffmpeg/)

## Startup, Send to, clipboard (src/main.rs, src/sendto.rs, src/clipboard.rs)
