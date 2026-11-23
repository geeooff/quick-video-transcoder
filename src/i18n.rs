//! Localization: Project Fluent messages and ICU4X number formats.
//!
//! Rules this module is shaped by:
//! - **Every text a user reads goes through [`crate::tr!`]** with a message id from
//!   `i18n/en/quick_video_transcoder.ftl`; `fl!` checks the id and its arguments against that file at
//!   compile time. Logs and technical details (FFmpeg's own messages, error chains) stay in English, for
//!   support.
//! - **English is the reference and the fallback**: complete, every other file may be partial and falls
//!   back message by message (`fr-CA` → `fr` → `en`).
//! - **A language is a regional variant** ([`LANGUAGES`]): it chooses the texts (the closest files) and the
//!   number formats (CLDR data of that exact variant: `fr-CA` groups with a no-break space, `fr` with a
//!   narrow one). *Automatic* takes the OS's preferred display languages.
//! - **Numbers are formatted by Fluent's `NUM` function**, never in Rust: `{ NUM($mb, decimals: 1) } Mo`
//!   gives `25,0 Mo` in French. The CLDR data is compiled in (ICU4X `compiled_data`), so the result is the
//!   same on every OS, and OS regional customizations are not followed (as Firefox does by default).
//! - **Languages are always negotiated** ([`i18n_embed::select`]): loading a language without files
//!   panics in i18n-embed, so a stored preference is never loaded directly.

use std::sync::{LazyLock, Mutex, PoisonError};

use fixed_decimal::{Decimal, FloatPrecision, SignedRoundingMode, UnsignedRoundingMode};
use fluent_bundle::FluentValue;
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::{DesktopLanguageRequester, LanguageLoader};
use icu_decimal::DecimalFormatter;
use icu_locale_core::Locale;
use rust_embed::RustEmbed;
use unic_langid::LanguageIdentifier;

/// The `.ftl` files, embedded in the executable.
#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

/// The languages offered in the *Language* menu: a regional variant and its name in that language (a
/// language is always named in itself, so anyone finds theirs).
pub const LANGUAGES: [(&str, &str); 6] = [
    ("en-US", "English (United States)"),
    ("en-GB", "English (United Kingdom)"),
    ("fr-FR", "Français (France)"),
    ("fr-CA", "Français (Canada)"),
    ("fr-BE", "Français (Belgique)"),
    ("fr-CH", "Français (Suisse)"),
];

/// The loader every [`crate::tr!`] reads.
pub static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    // The English file always loads: messages are never missing, even before `apply`.
    if let Err(error) = i18n_embed::select(
        &loader,
        &Localizations,
        &[loader.fallback_language().clone()],
    ) {
        log::error!("cannot load the English messages: {error}");
    }
    register_functions(&loader, None);
    loader
});

/// The language currently applied (`None`: automatic), for the menu.
static CHOICE: Mutex<Option<String>> = Mutex::new(None);

/// A localized message: `tr!("id")` or `tr!("id", name = value, …)`.
#[macro_export]
macro_rules! tr {
    ($id:literal) => {
        i18n_embed_fl::fl!(&*$crate::i18n::LOADER, $id)
    };
    ($id:literal, $($args:tt)*) => {
        i18n_embed_fl::fl!(&*$crate::i18n::LOADER, $id, $($args)*)
    };
}

/// Applies a language: `None` follows the OS, `Some("fr-CA")` a variant of [`LANGUAGES`]. An unknown tag
/// (an old preference) counts as automatic. Can be called again at any time; the next frame shows it.
pub fn apply(choice: Option<&str>) {
    let choice = choice.filter(|tag| LANGUAGES.iter().any(|(known, _)| known == tag));
    let requested: Vec<LanguageIdentifier> = match choice {
        Some(tag) => tag.parse().into_iter().collect(),
        None => DesktopLanguageRequester::requested_languages(),
    };
    let loader = &*LOADER;
    match i18n_embed::select(loader, &Localizations, &requested) {
        Ok(loaded) => log::info!(
            "language: {} (asked: {})",
            loaded
                .first()
                .map_or_else(|| "en".to_owned(), ToString::to_string),
            requested
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Err(error) => log::error!("cannot load the messages: {error}"),
    }
    // Numbers follow the asked variant when its language is the one shown (fr-CA formats for French
    // texts), else the language of each file.
    register_functions(loader, requested.first());
    *CHOICE.lock().unwrap_or_else(PoisonError::into_inner) = choice.map(str::to_owned);
}

/// The language chosen in the menu (`None`: automatic).
pub fn choice() -> Option<String> {
    CHOICE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Sets up every loaded bundle: the `NUM` function, no bidi isolation marks (Fluent wraps each variable in
/// U+2068/U+2069 for right-to-left scripts, which egui does not lay out and would draw as boxes), and the
/// pseudo-localization in debug builds. i18n-embed rebuilds the bundles on each load, so this follows
/// every [`i18n_embed::select`].
fn register_functions(loader: &FluentLanguageLoader, asked: Option<&LanguageIdentifier>) {
    let asked = asked.cloned();
    #[cfg(debug_assertions)]
    let pseudo = std::env::var_os("QVT_PSEUDO").is_some();
    loader.with_bundles_mut(move |bundle| {
        let Some(language) = bundle.locales.first().cloned() else {
            return;
        };
        bundle.set_use_isolating(false);
        #[cfg(debug_assertions)]
        if pseudo {
            bundle.set_transform(Some(|text| {
                fluent_pseudo::transform_dom(text, false, true, true)
            }));
        }
        let format_locale = asked
            .as_ref()
            .filter(|asked| asked.language == language.language)
            .unwrap_or(&language);
        let formatter = number_formatter(&format_locale.to_string());
        let added = bundle.add_function("NUM", move |positional, named| {
            let Some(FluentValue::Number(number)) = positional.first() else {
                return FluentValue::Error;
            };
            let decimals = match named.get("decimals") {
                Some(FluentValue::Number(d)) => d.value.clamp(0.0, 6.0) as i16,
                _ => 0,
            };
            format_number(&formatter, number.value, decimals)
                .map_or(FluentValue::Error, |text| FluentValue::String(text.into()))
        });
        if let Err(error) = added {
            log::error!("cannot register NUM: {error}");
        }
    });
}

/// A CLDR number formatter for a locale tag; the root (English-like) one if the tag does not parse.
fn number_formatter(tag: &str) -> DecimalFormatter {
    let locale: Locale = tag.parse().unwrap_or(Locale::UNKNOWN);
    DecimalFormatter::try_new(locale.into(), Default::default())
        .expect("the compiled CLDR data has every locale, falling back to root")
}

/// `value` with exactly `decimals` digits after the separator (25 with 1 → "25.0" / "25,0").
fn format_number(formatter: &DecimalFormatter, value: f64, decimals: i16) -> Option<String> {
    let mut decimal = Decimal::try_from_f64(value, FloatPrecision::RoundTrip).ok()?;
    decimal.round_with_mode(
        -decimals,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    decimal.pad_end(-decimals);
    Some(formatter.format(&decimal).to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use fluent_syntax::ast;

    use super::*;

    /// The message files, by language folder.
    fn files() -> BTreeMap<String, String> {
        Localizations::iter()
            .filter(|path| path.ends_with(".ftl"))
            .map(|path| {
                let language = path.split('/').next().unwrap().to_owned();
                let file = Localizations::get(&path).unwrap();
                (language, String::from_utf8(file.data.to_vec()).unwrap())
            })
            .collect()
    }

    /// Each message id with the variables it uses (in its value and attributes, selectors included).
    fn messages(text: &str) -> BTreeMap<String, BTreeSet<String>> {
        let resource = fluent_syntax::parser::parse(text).unwrap_or_else(|(_, errors)| {
            panic!("invalid Fluent syntax: {errors:?}");
        });
        let mut messages = BTreeMap::new();
        for entry in resource.body {
            if let ast::Entry::Message(message) = entry {
                let mut variables = BTreeSet::new();
                let patterns = message
                    .value
                    .iter()
                    .chain(message.attributes.iter().map(|a| &a.value));
                for pattern in patterns {
                    collect_pattern(pattern, &mut variables);
                }
                messages.insert(message.id.name.to_owned(), variables);
            }
        }
        messages
    }

    fn collect_pattern(pattern: &ast::Pattern<&str>, variables: &mut BTreeSet<String>) {
        for element in &pattern.elements {
            if let ast::PatternElement::Placeable { expression } = element {
                collect_expression(expression, variables);
            }
        }
    }

    fn collect_expression(expression: &ast::Expression<&str>, variables: &mut BTreeSet<String>) {
        match expression {
            ast::Expression::Select { selector, variants } => {
                collect_inline(selector, variables);
                for variant in variants {
                    collect_pattern(&variant.value, variables);
                }
            }
            ast::Expression::Inline(inline) => collect_inline(inline, variables),
        }
    }

    fn collect_inline(inline: &ast::InlineExpression<&str>, variables: &mut BTreeSet<String>) {
        match inline {
            ast::InlineExpression::VariableReference { id } => {
                variables.insert(id.name.to_owned());
            }
            ast::InlineExpression::FunctionReference { arguments, .. } => {
                for argument in &arguments.positional {
                    collect_inline(argument, variables);
                }
                for argument in &arguments.named {
                    collect_inline(&argument.value, variables);
                }
            }
            ast::InlineExpression::Placeable { expression } => {
                collect_expression(expression, variables)
            }
            _ => {}
        }
    }

    /// Every file parses, and a translation has no message English lacks nor a variable English does not
    /// pass (it would show as an error in the window).
    #[test]
    fn translations_match_the_english_messages() {
        let files = files();
        let english = messages(files.get("en").expect("the English file"));
        assert!(!english.is_empty());
        for (language, text) in &files {
            for (id, variables) in messages(text) {
                let Some(reference) = english.get(&id) else {
                    panic!("{language}: `{id}` is not an English message");
                };
                assert!(
                    variables.is_subset(reference),
                    "{language}: `{id}` uses {variables:?}, English passes {reference:?}"
                );
            }
        }
    }

    /// French is complete: a message added in English must be translated in the same change (regional
    /// files stay partial on purpose).
    #[test]
    fn french_is_complete() {
        let files = files();
        let english: BTreeSet<String> = messages(&files["en"]).into_keys().collect();
        let french: BTreeSet<String> = messages(&files["fr"]).into_keys().collect();
        let missing: Vec<&String> = english.difference(&french).collect();
        assert!(missing.is_empty(), "not translated in French: {missing:?}");
    }

    /// Every English message is used by the code, so the file holds no dead text to translate.
    #[test]
    fn every_message_is_used() {
        let mut source = String::new();
        let mut folders = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(folder).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    folders.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    source.push_str(&std::fs::read_to_string(path).unwrap());
                }
            }
        }
        let used: BTreeSet<&str> = source
            .split("tr!(")
            .skip(1)
            .filter_map(|after| after.trim_start().strip_prefix('"')?.split('"').next())
            .collect();
        let english = messages(files().get("en").unwrap());
        let unused: Vec<&String> = english
            .keys()
            .filter(|id| !used.contains(id.as_str()))
            .collect();
        assert!(
            unused.is_empty(),
            "English messages no code uses: {unused:?}"
        );
    }

    /// `tr!` with a number goes through NUM (English is loaded by default).
    #[test]
    fn numbers_reach_the_messages() {
        assert_eq!(
            crate::tr!("timeline-volume-tooltip", percent = 80.0),
            "Volume 80%"
        );
    }

    #[test]
    fn every_offered_language_has_a_name_and_parses() {
        for (tag, name) in LANGUAGES {
            assert!(tag.parse::<LanguageIdentifier>().is_ok(), "{tag}");
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn formats_numbers_per_variant() {
        let fr = number_formatter("fr-FR");
        assert_eq!(format_number(&fr, 25.0, 1).unwrap(), "25,0");
        assert_eq!(format_number(&fr, 1234.0, 0).unwrap(), "1\u{202f}234");
        assert_eq!(format_number(&fr, 29.97, 2).unwrap(), "29,97");
        let ca = number_formatter("fr-CA");
        assert_eq!(format_number(&ca, 1234.0, 0).unwrap(), "1\u{a0}234");
        let en = number_formatter("en-GB");
        assert_eq!(format_number(&en, 1234.56, 1).unwrap(), "1,234.6");
    }
}
