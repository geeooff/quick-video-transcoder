# Localization

**TL;DR** — Texts are [Project Fluent](https://projectfluent.org/) messages in `i18n/<language>/quick_video_transcoder.ftl`,
embedded in the executable and read with `tr!("id")`. English is the reference and the fallback; regional
files (`fr-CA`, `en-GB`…) hold only what differs from their language. Numbers are formatted by the `NUM`
function (ICU4X, CLDR data compiled in). *View → Language* picks *Automatic* or a regional variant.

Design and rules: the doc comment of [`src/i18n.rs`](../src/i18n.rs). Spikes behind the choices (2026-11-23):
ICU4X adds about 93 KB with every locale's number data; OS regional customizations are not followed (as
Firefox does by default), the chosen variant's CLDR formats are.

## Writing code

- Every text a user reads: `crate::tr!("area-element", name = value)`. Never a literal, never `format!` to
  build a sentence, never two messages glued together: word order differs between languages.
- Numbers are passed as numbers (`mb = 25.0`), formatted in the message: `{ NUM($mb, decimals: 1) } MB`.
  Counts too, so plurals work: `{ $count -> [one] 1 video *[other] { NUM($count) } videos }`.
- Names (files, codecs, encoder names, profile names) are passed as variables and never translated: they are
  data. The built-in profile names stay as they are in every language (they are stored in `profiles.toml`).
- Symbols-only labels (`⏮`, `▶`) stay in code; their tooltip is a message.
- Logs (`log::info!`…) and technical details (FFmpeg's stderr, `anyhow` chains) stay in English: they are
  for support. A message shown to the user wraps them: `encode-failed = Encoding failed: { $detail }`.
- Time codes (`00:01:02.500`) and technical units that are identifiers (`kbit/s`, `fps` in a codec label)
  are formatted by `timecode` and passed as variables when they are data; a unit in a sentence belongs to the
  message, so a translator can write `i/s`.
- One message per sentence or label, with an id `area-element[-detail]` (`-tooltip` for hover texts), in
  the section of its source file in the English file. A comment above a message explains it when the
  English alone is ambiguous (where it shows, what a variable holds).
- `cargo test` checks that every file parses, that translations use only English ids and variables, and
  that every English message is used.
- Debug builds: `QVT_PSEUDO=1` elongates and brackets every text (`[Ŝţàŕţ ĥéŕé~~~]`): a text left in code
  shows unbracketed, a layout that cannot take a longer language shows clipped.

## Translating

The application is for people who share game captures in chats (Messenger, Discord): gamers, not video
engineers. It should read like a well-made native app of that language, never like a translation:

- **Translate the meaning in this context, not the words.** Read the code around a message when the id and
  comment are not enough. Prefer the term the platform and well-known apps of that language use (Windows,
  Explorer, Discord, OBS, HandBrake, VLC in that language).
- **Short.** Buttons and menu items are commands; tooltips are one line. French runs about 20 % longer than
  English: when a label would not fit, rephrase rather than abbreviate.
- **Consistent.** One concept, one word, everywhere (glossary below).
- **Keep variables and markup** exactly: `{ $name }`, `{ NUM(...) }`, selectors; reorder them freely.
- **Keyboard keys** are named as that language's keyboards and Windows name them (`Début`, `Fin`, `Espace`,
  `Gauche`…); letter shortcuts (`I`, `O`, `M`) do not change.

### French (`fr`, and what differs in `fr-CA`, `fr-BE`, `fr-CH`)

- Typography: a no-break space (U+00A0) before `:`, `;`, `!`, `?`, `%` and inside `« »`; a narrow no-break
  space (U+202F) is also correct before `;` `!` `?` (fr-FR). Ellipsis `…`. Apostrophe `’`. Capital only on
  the first word of a label (`Ajouter des vidéos`, not `Ajouter Des Vidéos`).
- Units: `Mo`, `Go`, `kbit/s`, `i/s` (images par seconde), `Hz`. A no-break space between a number and its unit.
- Commands in the infinitive (`Encoder`, `Ajouter des vidéos…`), not the imperative.
- `fr-CA`: Québec usage, Office québécois de la langue française terminology where it is what people
  actually say and what Windows in Canadian French shows (`courriel`, `clavarder` are not needed here).
  Only messages that really differ go in `i18n/fr-CA/`.
- `fr-BE`, `fr-CH`: same texts as `fr` unless a term really differs; their number formats differ anyway (CLDR).

### English (`en`, `en-GB`)

- `en` is US English. `en-GB` holds only British spellings and terms that differ (*licence* as a noun,
  *colour*, *optimise*…), nothing else.

## Glossary

| English | French (`fr`) | Notes |
| --- | --- | --- |
| video (a queued file) | vidéo | |
| queue | file d’attente | « la file » once the context is clear |
| kept part (trimmed range) | partie conservée | the range between the two handles |
| trim / trim handle | découper / poignée | |
| start / end (of the kept part) | début / fin | |
| frame (a picture) | image | « trame » is interlaced fields: never |
| frame rate | fréquence d’images | `30 i/s` in values |
| resolution | résolution | |
| size limit | taille maximale | |
| bitrate | débit | |
| estimate (noun) | estimation | |
| encode (verb) / encoding | encoder / encodage | |
| encoder (NVENC, x265…) | encodeur | |
| profile | profil | |
| preview | aperçu | |
| tone mapping | mappage tonal | HDR → SDR |
| sound / audio track | son / piste audio | |
| microphone | micro | |
| quality bounds | limites de qualité | |
| Send to (Explorer menu) | Envoyer vers | Windows’ own French name |
| log | journal | |
| output folder | dossier de sortie | |
