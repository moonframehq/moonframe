use super::*;

#[test]
fn report_messages_publish_words_instead_of_ids() {
    let name = Text::message("usage-bifrost-sourced-limit")
        .arg("source", Text::message("usage-report-usage"))
        .arg("limit", "Opus");
    let report = Report::new(
        claude(),
        Account {
            email: Some(Text::service("me@example.com")),
            plan: Some(Text::message("usage-venice-subscription-credits")),
        },
        vec![Window::new(Kind::Named(name), 25., None, None)],
    )
    .with_balances([Balance::new(
        Text::message("usage-venice-credits"),
        3.,
        Unit::Count(herdr_usage::model::Count::Credits),
    )])
    .with_sections([
        Section::Facts {
            title: Text::message("usage-report-usage"),
            facts: vec![(
                Text::message("usage-venice-credits"),
                Text::message("usage-venice-credit-count")
                    .arg("credits", 3)
                    .arg("count", "3"),
            )],
        },
        Section::Shares {
            title: Text::message("usage-report-usage"),
            shares: vec![(Text::service("<Opus> {literal}"), 25.)],
        },
    ]);
    let usage = Usage::showing(Host::Local, vec![Reading {
        provider: claude(),
        report: Some(report),
        error: Some(Error::RateLimited.into()),
        access: None,
    }]);
    let out = publish::usage(&usage, &UsageConfig::default(), Some(&Host::Local));
    let provider = &out["providers"][0];
    assert_eq!(provider["email"], "me@example.com");
    assert_eq!(provider["plan"], "Subscription credits");
    assert_eq!(provider["error"], Error::RateLimited.to_string());
    assert_eq!(provider["windows"][0]["title"], "Usage Opus");
    assert_eq!(provider["windows"][0]["label"], "25% used Usage Opus");
    assert_eq!(provider["balances"][0]["label"], "Credits");
    assert_eq!(provider["balances"][0]["unit"], "credits");
    assert_eq!(provider["sections"][0]["title"], "Usage");
    assert_eq!(provider["sections"][0]["facts"][0]["label"], "Credits");
    assert_eq!(provider["sections"][0]["facts"][0]["value"], "3 credits");
    assert_eq!(
        provider["sections"][1]["shares"][0]["label"],
        "<Opus> {literal}"
    );
}

#[test]
fn plural_messages_use_numeric_arguments() {
    for (count, expected) in [(1, "1 credit"), (2, "2 credits")] {
        let text = Text::message("usage-venice-credit-count")
            .arg("credits", count)
            .arg("count", count.to_string());
        assert_eq!(text::english(&text), expected);
    }
}

#[test]
fn messages_track_the_pinned_usage_dependency() {
    let manifest: toml::Table = include_str!("../../../../Cargo.toml").parse().unwrap();
    let rev = manifest["workspace"]["dependencies"]["herdr-usage"]["rev"]
        .as_str()
        .unwrap();
    assert_eq!(include_str!("../../locales/REVISION").trim(), rev);
}
