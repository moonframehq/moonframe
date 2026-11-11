//! Checking an entry with data, states and messages, not only empty.

use {
    super::*,
    crate::entries::{CheckOptions, RootPolicy, Severity, check, check_in},
};

const DATA_ONLY: &str = r#"return mod {
    init = function(previous) return previous or { open = false } end,
    update = function(state, msg)
        if msg == "open" then state.open = true end
    end,
    view = function(state, app)
        local data = app["user:fuubar"]
        if state.open then return ui.row { ui.icon { "chek" } } end
        if not data then return ui.text "waiting" end
        return ui.column { ui.row { padding = 8, ui.text(data.title) } }
    end,
}"#;

fn entry(source: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "main.luau", source);
    dir
}

fn named() -> CheckOptions {
    CheckOptions {
        name: Some("user:fuubar".to_owned()),
        ..Default::default()
    }
}

#[test]
fn an_empty_check_misses_what_only_data_draws() {
    let dir = entry(DATA_ONLY);
    assert_eq!(
        check_in(dir.path(), &RootPolicy::default(), &named()),
        Vec::new()
    );
}

#[test]
fn data_states_and_messages_reach_every_branch() {
    let dir = entry(DATA_ONLY);
    let options = CheckOptions {
        app: vec![Data::map([(
            "user:fuubar",
            Data::map([("title", Data::from("hi"))]),
        )])],
        ..named()
    };
    let found = check_in(dir.path(), &RootPolicy::default(), &options);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(found[0].line, Some(10));
    assert!(
        found[0].message.contains("column[1].row.padding"),
        "{}",
        found[0]
    );
    assert!(
        found[0].message.contains("pixels belong in styles"),
        "{}",
        found[0]
    );
    assert!(found[0].message.contains("[app case 1]"), "{}", found[0]);

    // From a state, and after a message, the icon branch draws.
    for options in [
        CheckOptions {
            state: vec![Data::map([("open", Data::Bool(true))])],
            ..named()
        },
        CheckOptions {
            messages: vec![Data::from("open")],
            ..named()
        },
    ] {
        let found = check_in(dir.path(), &RootPolicy::default(), &options);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].message.contains("chek"), "{}", found[0]);
        assert!(
            found[0].message.contains("check"),
            "close names: {}",
            found[0]
        );
        assert_eq!(found[0].line, Some(8));
    }
}

#[test]
fn luau_errors_carry_their_line_and_budget_overruns_are_reported() {
    let dir = entry("return mod {\n  view = function()\n    error('boom')\n  end,\n}");
    let found = check(dir.path(), &RootPolicy::default());
    assert_eq!(found[0].line, Some(3), "{found:?}");
    assert!(found[0].message.contains("boom"));

    let dir = entry("return mod { view = function() while true do end end }");
    let found = check(dir.path(), &RootPolicy::default());
    assert!(
        found
            .iter()
            .any(|found| found.message.contains("ran out of instructions")),
        "{found:?}"
    );
}

#[test]
fn strict_refuses_classes_no_rule_styles() {
    let dir = entry(
        r#"return mod {
    styles = { [".mine"] = { padding = 4 } },
    view = function() return ui.row { class = "mine", ui.text { "a", class = "nowhere" } } end,
}"#,
    );
    assert_eq!(check(dir.path(), &RootPolicy::default()), Vec::new());
    let strict = CheckOptions {
        strict: true,
        ..Default::default()
    };
    let found = check_in(dir.path(), &RootPolicy::default(), &strict);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].message.contains("nowhere"), "{}", found[0]);
    assert_eq!(found[0].severity, Severity::Error);
}
