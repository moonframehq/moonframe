//! English report messages from the same revision as sheprd-usage. Service
//! text is kept verbatim; message IDs and arguments never leak into the UI.

use {
    fluent::{FluentArgs, FluentBundle, FluentResource, FluentValue},
    sheprd_usage::{Arg, Text},
};

thread_local! {
    static ENGLISH: FluentBundle<FluentResource> = catalog();
}

fn catalog() -> FluentBundle<FluentResource> {
    let language = "en-US"
        .parse()
        .unwrap_or_else(|error| panic!("English locale: {error}"));
    let mut bundle = FluentBundle::new(vec![language]);
    bundle.set_use_isolating(false);
    let resource = FluentResource::try_new(include_str!("../messages/en-US.ftl").to_owned())
        .unwrap_or_else(|(_, errors)| panic!("usage messages: {errors:?}"));
    bundle
        .add_resource(resource)
        .unwrap_or_else(|errors| panic!("usage messages: {errors:?}"));
    bundle
}

pub(crate) fn words(text: &Text) -> String {
    match text {
        Text::Service(text) => text.clone(),
        Text::Message { id, args } => {
            let mut values = FluentArgs::new();
            for (name, arg) in args {
                let value = match arg {
                    Arg::Number(number) => FluentValue::from(*number),
                    Arg::Text(text) => FluentValue::from(words(text)),
                };
                values.set(*name, value);
            }
            ENGLISH.with(|bundle| {
                let Some(pattern) = bundle.get_message(id).and_then(|message| message.value())
                else {
                    tracing::error!(id, "usage message is missing");
                    return "Usage unavailable".to_owned();
                };
                let mut errors = Vec::new();
                let text = bundle.format_pattern(pattern, Some(&values), &mut errors);
                if !errors.is_empty() {
                    tracing::error!(id, ?errors, "usage message could not be formatted");
                    return "Usage unavailable".to_owned();
                }
                text.into_owned()
            })
        },
    }
}
