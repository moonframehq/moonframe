#![allow(clippy::unwrap_used, clippy::expect_used)]

mod ui;

use {
    super::*,
    herdr_usage::{
        Reading,
        model::{Account, Balance, Kind, Report, Section, Unit, WEEK, Window},
    },
    serde_json::json,
    std::time::{Duration, SystemTime},
};

fn claude() -> herdr_usage::Provider {
    registry::find("claude").unwrap()
}

#[test]
fn hosts_read_as_they_are_named() {
    for name in ["local", "ssh:me@box", "wsl:Ubuntu"] {
        assert_eq!(host_name(&parse_host(name).unwrap()), name);
    }
    for name in [
        "",
        "ssh:",
        "wsl:",
        "box",
        "tcp:box",
        "ssh:-oProxyCommand=touch /tmp/x",
        "ssh:box -v",
        "ssh:box\n",
        "wsl:--exec",
    ] {
        assert!(parse_host(name).is_err(), "{name}");
    }
}

#[test]
fn settings_are_herdrs_usage_table_and_typos_are_refused() {
    let settings = json!({
        "show_providers": ["openrouter"],
        "browser_cookies": false,
        "host": "local",
        "grants": "/tmp/x.json",
    });
    let read = config(&settings).unwrap();
    assert_eq!(read.show_providers, ["openrouter"]);
    assert!(!read.browser_cookies);
    assert!(config(&json!({ "browser_cookies": "yes" })).is_err());
}

#[test]
fn unknown_provider_ids_are_dropped_not_fatal() {
    let read = config(&json!({ "hide_providers": ["claude", "nope"] })).unwrap();
    assert_eq!(read.hide_providers, ["claude"]);
}

#[test]
fn a_window_reads_as_fractions_and_countdowns() {
    let now = SystemTime::now();
    // Three and a half days into a week, with 40% used: 10% in reserve.
    let limit = Window::new(Kind::Weekly, 40., Some(now + WEEK / 2), Some(WEEK));
    let out = publish::window(&limit, now);
    assert_eq!(out["title"], "Weekly");
    assert_eq!(out["kind"], "weekly");
    assert_eq!(out["used"], 0.4);
    assert_eq!(out["left"], 0.6);
    assert_eq!(out["severity"], "normal");
    assert_eq!(out["resets_in"], "3d 12h");
    assert_eq!(out["pace"], 0.5);
    assert_eq!(out["pace_note"], "10% in reserve · Lasts until reset");
}

#[test]
fn a_window_without_a_reset_has_no_pace() {
    let out = publish::window(
        &Window::new(Kind::Named("Opus".into()), 85., None, None),
        SystemTime::now(),
    );
    assert_eq!(out["severity"], "critical");
    assert_eq!(out["label"], "85% used Opus");
    assert!(out["pace"].is_null());
    assert!(out["resets_in"].is_null());
}

#[test]
fn bundled_logos_are_named_and_others_left_out() {
    assert_eq!(
        publish::icon("claude", Some("icons/agent-claude.svg")),
        Some("agent-claude")
    );
    assert_eq!(
        publish::icon("openai", Some("icons/agent-codex.svg")),
        Some("agent-codex")
    );
    assert_eq!(publish::icon("factory", None), Some("agent-droid"));
    assert_eq!(publish::icon("copilot", None), Some("agent-copilot"));
    assert_eq!(publish::icon("openrouter", None), None);
    assert_eq!(
        publish::icon("kimi", Some("icons/providers/kimi.svg")),
        None
    );
}

#[test]
fn a_reading_publishes_its_tightest_window_and_details() {
    let now = SystemTime::now();
    let report = Report::new(
        claude(),
        Account {
            email: Some("me@example.com".into()),
            plan: Some("Max".into()),
        },
        vec![
            Window::new(
                Kind::Weekly,
                70.,
                Some(now + Duration::from_secs(3600)),
                Some(WEEK),
            ),
            Window::new(Kind::Session, 10., None, None),
        ],
    )
    .with_balances([Balance::new("Extra usage", 5., Unit::Currency("USD".into())).out_of(20.)])
    .with_sections([Section::Shares {
        title: "By surface".into(),
        shares: vec![("Claude Code".into(), 75.)],
    }]);
    let usage = Usage::showing(Host::Local, vec![Reading {
        provider: claude(),
        report: Some(report),
        error: Some("rate limited".into()),
        access: None,
    }]);
    let out = publish::usage(&usage, &UsageConfig::default(), Some(&Host::Local));
    assert_eq!(out["host"], "local");
    assert_eq!(out["headline"], json!(["claude"]));
    let claude = &out["providers"][0];
    assert_eq!(claude["name"], "Claude");
    assert_eq!(claude["icon"], "agent-claude");
    assert_eq!(claude["plan"], "Max");
    assert_eq!(claude["stale"], true);
    assert_eq!(claude["used"], 0.7);
    assert_eq!(claude["severity"], "warning");
    // Sorted session first, as herdr shows them.
    assert_eq!(claude["windows"][0]["kind"], "session");
    assert_eq!(claude["balances"][0]["text"], "$5.00 of $20.00");
    assert_eq!(claude["balances"][0]["fraction"], 0.25);
    assert_eq!(claude["sections"][0]["kind"], "shares");
    assert_eq!(claude["sections"][0]["shares"][0]["share"], 0.75);
}

#[test]
fn balance_fractions_keep_spending_and_credit_labels_without_claiming_left() {
    let provider = registry::find("deepinfra").unwrap();
    let usd = Unit::Currency("USD".into());
    let report = Report::new(provider, Account::default(), vec![]).with_balances([
        Balance::new("Billing cycle spend", 12., usd.clone()).out_of(50.),
        Balance::new("Credits left", 38., usd.clone()).out_of(50.),
        Balance::new("Uncapped", 12., usd.clone()),
        Balance::new("Zero budget", 12., usd).out_of(0.),
    ]);
    let usage = Usage::showing(Host::Local, vec![Reading {
        provider,
        report: Some(report),
        error: None,
        access: None,
    }]);
    let out = publish::usage(&usage, &UsageConfig::default(), Some(&Host::Local));
    let balances = out["providers"][0]["balances"].as_array().unwrap();
    assert_eq!(balances[0]["label"], "Billing cycle spend");
    assert_eq!(balances[0]["fraction"], 0.24);
    assert_eq!(balances[1]["label"], "Credits left");
    assert_eq!(balances[1]["fraction"], 0.76);
    assert!(balances[2]["fraction"].is_null());
    assert!(balances[3]["fraction"].is_null());
    assert!(balances.iter().all(|balance| balance.get("left").is_none()));
}

#[test]
fn grants_survive_a_relaunch_and_ignore_unknown_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state/keychain.json");
    let mut grants = Grants::load(Some(path.clone()));
    grants.grant(claude());
    assert!(
        Grants::load(Some(path.clone()))
            .granted()
            .contains(&claude())
    );
    grants.revoke(claude());
    assert!(Grants::load(Some(path)).granted().is_empty());
    assert!(grants::parse(br#"{"providers":["claude","nope"]}"#).contains(&claude()));
    assert!(grants::parse(b"not json").is_empty());
}

#[test]
fn only_writers_may_allow_or_switch_hosts() {
    use std::sync::{Arc, Mutex};
    let failures = Arc::new(Mutex::new(Vec::new()));
    let events: moonframe_providers::Events = {
        let failures = Arc::clone(&failures);
        Arc::new(move |event| {
            if let moonframe_providers::Event::Failed { problem, .. } = event {
                failures.lock().unwrap().push(problem.to_string());
            }
        })
    };
    let settings = moonframe_providers::from_json(&json!({ "writers": ["usage"] })).unwrap();
    let handle = Handle::new("usage", std::path::Path::new("."), settings, events);
    let mut provider = UsageProvider {
        writers: Mods::from_settings(handle.settings(), "writers"),
        host: Some(Host::Local),
        ..UsageProvider::default()
    };
    let message = |value| moonframe_providers::from_json(&value).unwrap();
    provider.receive(&handle, "other", message(json!({ "host": "ssh:box" })));
    provider.receive(&handle, "other", message(json!({ "allow": "claude" })));
    assert_eq!(provider.host, Some(Host::Local));
    assert!(provider.grants.granted().is_empty());
    assert_eq!(failures.lock().unwrap().len(), 2);
    // A writer may switch only to a machine the settings list.
    provider.receive(&handle, "usage", message(json!({ "host": "wsl:Ubuntu" })));
    assert_eq!(provider.host, Some(Host::Local));
    assert_eq!(failures.lock().unwrap().len(), 3);
    provider.hosts.push(Host::Wsl("Ubuntu".into()));
    provider.receive(&handle, "usage", message(json!({ "host": "wsl:Ubuntu" })));
    assert_eq!(provider.host, Some(Host::Wsl("Ubuntu".into())));
    provider.receive(&handle, "usage", message(json!({ "host": "local" })));
    assert_eq!(provider.host, Some(Host::Local));
}
