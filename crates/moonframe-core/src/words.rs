//! An app's words in the user's language. Each app layer may hold
//! `locales/<language>/*.ftl`, Fluent messages by ID; a module asks for one
//! with `ui.t("inbox-unread", { count = 3 })`, and writes numbers and dates
//! the language's way with `ui.number` and `ui.date`.
//!
//! The language is the best match between those the app has messages in
//! and those the user prefers ([`requested`]); [`FALLBACK`] is the one the
//! app is written in, under every other, so a message missing from a
//! translation shows in English, and one missing from English shows as its
//! ID. A higher layer's message replaces a lower one's of the same ID, so a
//! package or the user's folder can change any of the app's words.

mod format;
mod luau;

use {
    crate::{Error, Roots},
    fluent_bundle::{FluentArgs, FluentResource, bundle},
    intl_memoizer::IntlLangMemoizer,
    std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
        rc::Rc,
    },
    unic_langid::LanguageIdentifier,
};

/// The language an app's own words are in, and the one shown when the app
/// has none the user prefers.
pub const FALLBACK: &str = "en-US";

/// Overrides the system's languages, for trying an app in one: a tag, or
/// several separated by commas, most preferred first.
pub const LANGUAGE_VAR: &str = "MOONFRAME_LANG";

/// Where an app layer keeps its messages.
const LOCALES: &str = "locales";

/// The most bytes read from one `.ftl` file.
const MAX_FILE: u64 = 1024 * 1024;

type Bundle = bundle::FluentBundle<FluentResource, IntlLangMemoizer>;

/// The messages and formats of one language, shared by every module of a
/// load; cloning shares them.
#[derive(Clone)]
pub(crate) struct Words(Rc<Inner>);

struct Inner {
    locale: LanguageIdentifier,
    /// The language's messages, then the fallback's, when it is another.
    bundles: Vec<Bundle>,
    formats: format::Formats,
}

impl Default for Words {
    /// No messages, in the fallback language: every ID shows as itself.
    fn default() -> Self {
        Self::new(fallback(), Vec::new())
    }
}

impl std::fmt::Debug for Words {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Words")
            .field("locale", &self.0.locale.to_string())
            .finish_non_exhaustive()
    }
}

impl Words {
    fn new(locale: LanguageIdentifier, bundles: Vec<Bundle>) -> Self {
        let formats = format::Formats::new(&locale);
        Self(Rc::new(Inner {
            locale,
            bundles,
            formats,
        }))
    }

    /// Reads every layer's messages in the language of `requested` that
    /// the app has, and in the fallback. What does not read is returned
    /// beside them, and left out: a file Fluent cannot parse loses only its
    /// broken entries.
    pub(crate) fn read(roots: &Roots, requested: &[LanguageIdentifier]) -> (Self, Vec<Error>) {
        let mut problems = Vec::new();
        let languages = languages(roots, &mut problems);
        let available: Vec<LanguageIdentifier> = languages.keys().cloned().collect();
        let locale = negotiate(requested, &available);
        let fallback = fallback();
        let mut chain = vec![locale.clone()];
        if locale != fallback {
            chain.push(fallback.clone());
        }
        let mut ids: BTreeMap<LanguageIdentifier, BTreeSet<String>> = BTreeMap::new();
        let mut bundles = Vec::new();
        for (language, files) in &languages {
            let mut bundle = (chain.contains(language)).then(|| new_bundle(language));
            let known = ids.entry(language.clone()).or_default();
            for path in files {
                let Some(resource) = parse(path, &mut problems) else {
                    continue;
                };
                known.extend(message_ids(&resource));
                if let Some(bundle) = &mut bundle {
                    // Files come lowest layer first: a higher one's
                    // message replaces the same ID below it.
                    bundle.add_resource_overriding(resource);
                }
            }
            if let Some(bundle) = bundle {
                bundles.push((language.clone(), bundle));
            }
        }
        // A translation's ID the app's own words lack is a typo or a
        // message that was removed: the translation is never shown.
        if let Some(source) = ids.get(&fallback) {
            for (language, known) in &ids {
                for id in known.difference(source) {
                    problems.push(Error::new(format!(
                        "{LOCALES}/{language}: `{id}` is not in {LOCALES}/{FALLBACK}, so it never shows"
                    )));
                }
            }
        }
        bundles.sort_by_key(|(language, _)| chain.iter().position(|wanted| wanted == language));
        let bundles = bundles.into_iter().map(|(_, bundle)| bundle).collect();
        (Self::new(locale, bundles), problems)
    }

    /// The language shown, as a BCP 47 tag.
    pub(crate) fn locale(&self) -> String {
        self.0.locale.to_string()
    }

    /// The message `id` (or `id.attribute`) with `args`, from the first
    /// language that has it; `id` itself when none does.
    pub(crate) fn text(&self, id: &str, args: Option<&FluentArgs<'_>>) -> String {
        let (name, attribute) = match id.split_once('.') {
            Some((name, attribute)) => (name, Some(attribute)),
            None => (id, None),
        };
        for bundle in &self.0.bundles {
            let Some(message) = bundle.get_message(name) else {
                continue;
            };
            let pattern = match attribute {
                Some(attribute) => message.get_attribute(attribute).map(|found| found.value()),
                None => message.value(),
            };
            let Some(pattern) = pattern else {
                continue;
            };
            // A missing argument shows as `{$name}`: the rest still reads.
            let mut errors = Vec::new();
            return bundle
                .format_pattern(pattern, args, &mut errors)
                .into_owned();
        }
        id.to_owned()
    }

    pub(crate) fn formats(&self) -> &format::Formats {
        &self.0.formats
    }
}

fn fallback() -> LanguageIdentifier {
    FALLBACK.parse().unwrap_or_default()
}

fn new_bundle(language: &LanguageIdentifier) -> Bundle {
    let mut bundle = Bundle::new(vec![language.clone()]);
    // GPUI draws Fluent's isolation marks as boxes, and lays out right to
    // left text on its own.
    bundle.set_use_isolating(false);
    bundle.set_formatter(Some(format::fluent_number));
    bundle
}

/// The languages under every layer's `locales`, each with its `.ftl`
/// files, lowest layer first and by name within one.
fn languages(
    roots: &Roots,
    problems: &mut Vec<Error>,
) -> BTreeMap<LanguageIdentifier, Vec<PathBuf>> {
    let mut found: BTreeMap<LanguageIdentifier, Vec<PathBuf>> = BTreeMap::new();
    for dir in roots.dirs(LOCALES) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut folders: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        folders.sort();
        for folder in folders {
            let Some(tag) = folder.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Ok(language) = tag.parse::<LanguageIdentifier>() else {
                problems.push(Error::new(format!(
                    "{}: `{tag}` is not a language tag, such as en-US or fr",
                    folder.display()
                )));
                continue;
            };
            let files = found.entry(language).or_default();
            files.extend(ftl_files(&folder));
        }
    }
    found
}

fn ftl_files(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|found| found == "ftl"))
        .collect();
    files.sort();
    files
}

/// `path`'s messages; what does not parse goes to `problems`, by line.
fn parse(path: &Path, problems: &mut Vec<Error>) -> Option<FluentResource> {
    let within = |message: String| Error::new(format!("{}: {message}", path.display()));
    let source = match fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_FILE => {
            problems.push(within(format!("larger than {MAX_FILE} bytes")));
            return None;
        },
        _ => match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                problems.push(within(format!("cannot read: {error}")));
                return None;
            },
        },
    };
    match FluentResource::try_new(source.clone()) {
        Ok(resource) => Some(resource),
        Err((resource, errors)) => {
            for error in errors {
                let line = source[..error.pos.start.min(source.len())]
                    .matches('\n')
                    .count()
                    + 1;
                problems.push(within(format!("line {line}: {}", error.kind)));
            }
            Some(resource)
        },
    }
}

fn message_ids(resource: &FluentResource) -> impl Iterator<Item = String> + '_ {
    resource.entries().filter_map(|entry| match entry {
        fluent_syntax::ast::Entry::Message(message) => Some(message.id.name.to_owned()),
        _ => None,
    })
}

/// The best of `available` for `requested`, most preferred first, else
/// the fallback.
fn negotiate(
    requested: &[LanguageIdentifier],
    available: &[LanguageIdentifier],
) -> LanguageIdentifier {
    let fallback = fallback();
    fluent_langneg::negotiate_languages(
        requested,
        available,
        Some(&fallback),
        fluent_langneg::NegotiationStrategy::Filtering,
    )
    .first()
    .map_or(fallback.clone(), |best| (*best).clone())
}

/// The languages the user prefers, most preferred first: [`LANGUAGE_VAR`]
/// when set, else the system's.
pub fn requested() -> Vec<LanguageIdentifier> {
    let set = std::env::var(LANGUAGE_VAR)
        .ok()
        .filter(|set| !set.trim().is_empty());
    let tags: Vec<String> = match set {
        Some(set) => set.split(',').map(|tag| tag.trim().to_owned()).collect(),
        None => sys_locale::get_locales().collect(),
    };
    tags.iter()
        .filter_map(|tag| posix_to_bcp47(tag).parse().ok())
        .collect()
}

/// `pt_BR.UTF-8@euro`, as some systems name a locale, as `pt-BR`.
fn posix_to_bcp47(tag: &str) -> String {
    let tag = tag.split(['.', '@']).next().unwrap_or(tag);
    tag.replace('_', "-")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
