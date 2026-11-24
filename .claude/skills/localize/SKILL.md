---
name: localize
description: Translate Quick Video Transcoder — new or changed English messages into French and its regional variants, or a whole new language — with persona-briefed subagents (translator, cold reviewer, regional localizers), screenshots and the project's Fluent rules. Use when messages were added or changed, when the maintainer asks for a new language or variant, or to review a translation.
---

# Localizing

Rules live in `docs/localization.md` (conventions, typography, glossary, regional decisions) and the doc
comment of `src/i18n.rs`. Read both first. The maintainer has no native reviewers: quality comes from the
briefs, context and a cold review — never from literal translation.

## When code adds or changes texts

- Every user-facing text is `crate::tr!("area-element", var = value)` with the English message in its source
  area of `i18n/en/quick_video_transcoder.ftl`, a `#` comment when the meaning is not obvious (where it
  shows, what each variable holds). Numbers through `{ NUM($x, decimals: N) }`, plurals as multi-line
  selectors, data (names, codecs, time codes) as string variables, logs stay English.
- `french_is_complete` fails until French has the new ids: translate them in the same change. When a French
  message changes, update the same message in regional files (`fr-CA`…).
- Check the layout in the longest languages: debug build, then
  `QVT_LANGUAGE=fr-FR` or `QVT_PSEUDO=1` with `QVT_SCREENSHOT=<file.ppm>` on a synthetic clip in a scratch
  folder (one app instance at a time: a second launch hands its files to the first).

## Translating with subagents

A few messages: do it inline with the same rules. Many, or a new language — orchestrate:

1. **Brief** (one file in scratch, read by every agent): the app and its users (gamers sharing clips in
   size-limited chats), `README.md`, `docs/localization.md`, the English file with comments, a screenshot of
   the window in English, the Fluent rules (keep ids/variables/selectors, literal U+00A0/U+202F, CLDR plural
   categories), the checks (`cargo test --locked --lib i18n`, a screenshot in their language), "edit only your
   file, never commit". Only one agent at a time takes screenshots.
2. **Translator** with a strong native persona (a seasoned localizer of games and consumer software from a
   precise city, who uses the platform's own terms and hates translated-sounding text). Writes the complete
   base file.
3. **Cold reviewer**, another persona (UX writer) who did not write it: contresens against the code,
   calques, register, consistency, typography, length on the screenshot. Edits the file, reports
   before → after.
4. **Regional localizers**, one persona per variant, in parallel: keep only real differences (vocabulary,
   typography conventions applied to every message concerned, keyboard names). An empty result is valid —
   then no file; record the decision in `docs/localization.md`.
5. **Orchestrator** (you): read every report, fix what they found in the English source or the code, check
   the screenshots yourself, run `.\scripts\build.ps1`, commit per step.

Use the strongest model for the base translator and the reviewer; a lighter one is fine for variants with
few expected differences. Agents hit by a usage limit are resumed with SendMessage, not restarted.

## A new language

`LANGUAGES` in `src/i18n.rs` (variants named in their own language), a complete base file, regional files,
a completeness test if the project commits to it, egui font coverage (Latin, Greek, Cyrillic built in; CJK
via the system font fallback; no right-to-left layout in egui), and a row in the regional decisions table.
