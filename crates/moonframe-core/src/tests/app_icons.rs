//! Icons a host app ships beside Moonframe's: named `app-…`, checked only
//! for their shape, and looked up in the host's assets.
use {super::*, crate::IconName};

#[test]
fn an_app_icon_is_named_with_its_prefix() {
    let icon = IconName::new("app-sessions").unwrap();
    assert_eq!(icon.name(), "app-sessions");
    assert_eq!(icon.path(), "icons/app-sessions.svg");
    // Not Moonframe's: no bundled name, so a backend without the host's
    // assets draws the placeholder rather than another icon.
    assert_eq!(icon.bundled(), None);
    // The placeholder is one Moonframe bundles.
    assert!(
        IconName::new(crate::MISSING_APP_ICON)
            .unwrap()
            .bundled()
            .is_some()
    );
    assert_eq!(
        IconName::new("app-pull-request-2").unwrap().name(),
        "app-pull-request-2"
    );
    assert_eq!(IconName::new("plus").unwrap().bundled(), Some("plus"));
}

#[test]
fn an_app_icon_takes_lowercase_words_joined_by_hyphens() {
    for name in [
        "app-",
        "app--sessions",
        "app-sessions-",
        "app-Sessions",
        "app-ses sions",
        "app-sessions.svg",
        "app-../secret",
        "app-é",
    ] {
        assert!(IconName::new(name).is_none(), "{name} was taken");
    }
    let long = format!("app-{}", "a".repeat(65));
    assert!(IconName::new(&long).is_none());
    assert!(IconName::new(&format!("app-{}", "a".repeat(64))).is_some());
}

#[test]
fn a_view_draws_an_app_icon_and_refuses_a_malformed_one() {
    let source = r#"return mod { view = function()
        return ui.row { ui.icon "app-sessions", ui.button { icon = "app-pulse", msg = "go" } }
    end }"#;
    let Node::Stack(row) = view_of(source).unwrap() else {
        panic!("expected a row");
    };
    let (Node::Icon(icon), Node::Button(button)) = (&row.children[0], &row.children[1]) else {
        panic!("unexpected children {:?}", row.children);
    };
    assert_eq!(icon.name.name(), "app-sessions");
    assert_eq!(button.icon.as_ref().map(IconName::name), Some("app-pulse"));
    let source = r#"return mod { view = function()
        return ui.row { ui.icon "app-Sessions" }
    end }"#;
    let dir = app(&[("bad", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = broken(&workspace, "bad");
    assert!(error.contains("no icon named `app-Sessions`"), "{error}");
}
