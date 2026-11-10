//! herdr-usage's words in English. A provider names most of what a report
//! shows (section titles, fact and balance labels, model windows) by Fluent
//! message ID and leaves the writing to the app; `locales/en-US` holds those
//! messages, copied from herdr-gpui's `crates/sheprd-gpui/locales/en-US/
//! usage-*.ftl` at the rev the workspace pins. Run
//! `scripts/update-usage-messages.py <checkout>` when that rev moves;
//! a message missing here shows as its ID.

use {
    herdr_i18n::{Catalog, FluentArgs, FluentValue},
    herdr_usage::{
        Arg, Text,
        model::{Kind, Window},
    },
    std::{sync::LazyLock, time::SystemTime},
};

static ENGLISH: LazyLock<Option<Catalog>> = LazyLock::new(|| {
    Catalog::english(include!(concat!(env!("OUT_DIR"), "/files.rs")))
        .inspect_err(|error| tracing::warn!("usage messages did not load: {error}"))
        .ok()
});

/// A report's words: the service's own as they are, a message in English.
pub(crate) fn english(text: &Text) -> String {
    match text {
        Text::Service(text) => text.clone(),
        Text::Message { id, args } => {
            let Some(catalog) = ENGLISH.as_ref().filter(|catalog| catalog.has(id)) else {
                return (*id).to_owned();
            };
            if args.is_empty() {
                return catalog.text(id);
            }
            let mut fluent = FluentArgs::with_capacity(args.len());
            for (name, arg) in args {
                let value = match arg {
                    Arg::Number(number) => FluentValue::from(*number),
                    Arg::Text(text) => FluentValue::from(english(text)),
                };
                fluent.set(*name, value);
            }
            catalog.text_with(id, &fluent)
        },
    }
}

/// `Weekly`, or a named window's words.
pub(crate) fn title(kind: &Kind) -> String {
    match kind {
        Kind::Named(name) => english(name),
        kind => kind.title(),
    }
}

/// `3% used 2h 53m`, or `85% used Opus` for a named window.
pub(crate) fn label(limit: &Window, now: SystemTime) -> String {
    match &limit.kind {
        Kind::Named(name) => format!("{}% used {}", limit.percent(), english(name)),
        _ => limit.label(now),
    }
}
