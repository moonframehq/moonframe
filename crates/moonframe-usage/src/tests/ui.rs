//! Exercise the shipped Luau mod with published data, without reading credentials.

use {
    super::*,
    moonframe_core::{Action, Node, Popover, Scroll, Workspace},
    std::fs,
};

fn app(data: &Value) -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in [
        (
            "main.luau",
            "return mod { view = function() return ui.region 'status' end }",
        ),
        (
            "mods/usage/main.luau",
            include_str!("../../../../app/mods/usage/main.luau"),
        ),
        (
            "components/usage-bar.luau",
            include_str!("../../../../app/components/usage-bar.luau"),
        ),
    ] {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let mut workspace = Workspace::open(dir.path(), false);
    workspace.set_host("usage", moonframe_providers::from_json(data).unwrap());
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    (dir, workspace)
}

fn sheet(workspace: &Workspace) -> &Popover {
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    match workspace.slot("usage").unwrap().view().unwrap() {
        Node::Popover(popover) => popover,
        other => panic!("usage is inaccessible: {other:?}"),
    }
}

fn nodes(node: &Node) -> Vec<&Node> {
    let mut found = vec![node];
    if let Node::Stack(stack) = node {
        found.extend(stack.children.iter().flat_map(nodes));
    }
    found
}

fn has_text(node: &Node, expected: &str) -> bool {
    nodes(node)
        .iter()
        .any(|node| matches!(node, Node::Text(text) if text.text == expected))
}

fn open(workspace: &mut Workspace) {
    let action = sheet(workspace)
        .anchor
        .iter()
        .flat_map(nodes)
        .find_map(|node| match node {
            Node::Stack(stack) => stack.action.clone(),
            _ => None,
        })
        .expect("no clickable status widget");
    let Action::Send { to, msg } = action else {
        panic!("expected a message");
    };
    workspace.send("usage", to.as_deref().unwrap_or("usage"), msg);
    assert!(sheet(workspace).open);
}

fn refresh_is_reachable(popover: &Popover) {
    assert!(nodes(&popover.content).iter().any(|node| {
        matches!(node, Node::Item(item) if item.action == Some(Action::Send {
            to: Some("provider:usage".into()),
            msg: Data::from("refresh"),
        }))
    }));
}

fn published(reading: Reading, config: &UsageConfig) -> Value {
    publish::usage(
        &Usage::showing(Host::Local, vec![reading]),
        config,
        Some(&Host::Local),
    )
}

#[test]
fn configured_provider_without_numbers_can_open_its_error_and_refresh() {
    let provider = registry::find("openrouter").unwrap();
    let config = UsageConfig {
        show_providers: vec![provider.id().into()],
        ..UsageConfig::default()
    };
    let data = published(
        Reading {
            provider,
            report: None,
            error: Some("Not signed in".into()),
            access: None,
        },
        &config,
    );
    assert_eq!(data["headline"], json!([]));
    let (_dir, mut workspace) = app(&data);
    open(&mut workspace);
    let popover = sheet(&workspace);
    assert!(has_text(&popover.content, "Not signed in"));
    refresh_is_reachable(popover);
}

#[test]
fn a_host_failure_without_providers_can_open_and_retry() {
    let data = json!({
        "host": "ssh:box", "busy": false, "providers": [], "headline": [],
        "error": "SSH connection failed",
    });
    let (_dir, mut workspace) = app(&data);
    open(&mut workspace);
    let popover = sheet(&workspace);
    assert!(has_text(&popover.content, "SSH connection failed"));
    refresh_is_reachable(popover);
}

#[test]
fn balance_only_accounts_show_the_balance_in_the_status_bar() {
    let provider = registry::find("openrouter").unwrap();
    let report = Report::new(provider, Account::default(), vec![]).with_balances([Balance::new(
        "Credits",
        60.,
        Unit::Currency("USD".into()),
    )
    .out_of(100.)]);
    let data = published(
        Reading {
            provider,
            report: Some(report),
            error: None,
            access: None,
        },
        &UsageConfig::default(),
    );
    let (_dir, mut workspace) = app(&data);
    let popover = sheet(&workspace);
    assert!(
        popover
            .anchor
            .iter()
            .any(|node| has_text(node, "Credits: $60.00 of $100.00"))
    );
    assert!(
        !popover
            .anchor
            .iter()
            .any(|node| has_text(node, "Allow access"))
    );
    open(&mut workspace);
    assert!(has_text(&sheet(&workspace).content, "$60.00 of $100.00"));
}

#[test]
fn access_only_accounts_still_offer_allow() {
    let provider = registry::find("zed").unwrap();
    let config = UsageConfig {
        show_providers: vec![provider.id().into()],
        ..UsageConfig::default()
    };
    let data = published(
        Reading {
            provider,
            report: None,
            error: Some("Keychain access needed".into()),
            access: Some(herdr_usage::Access::Needed),
        },
        &config,
    );
    let (_dir, mut workspace) = app(&data);
    assert!(
        sheet(&workspace)
            .anchor
            .iter()
            .any(|node| has_text(node, "Allow access"))
    );
    open(&mut workspace);
    assert!(nodes(&sheet(&workspace).content).iter().any(|node| {
        matches!(node, Node::Button(button) if button.action == Some(Action::Send {
            to: Some("provider:usage".into()),
            msg: moonframe_providers::from_json(&json!({"allow": "zed"})).unwrap(),
        }))
    }));
}

#[test]
fn long_reports_keep_all_details_inside_a_bounded_scroll_area() {
    let report = Report::new(claude(), Account::default(), vec![Window::new(
        Kind::Weekly,
        40.,
        None,
        None,
    )])
    .with_sections([Section::Facts {
        title: "Details".into(),
        facts: (0..30)
            .map(|i| (format!("Detail {i}"), "value".into()))
            .collect(),
    }]);
    let data = published(
        Reading {
            provider: claude(),
            report: Some(report),
            error: None,
            access: None,
        },
        &UsageConfig::default(),
    );
    let (_dir, mut workspace) = app(&data);
    open(&mut workspace);
    let popover = sheet(&workspace);
    let Node::Stack(viewport) = &*popover.content else {
        panic!("no scroll container");
    };
    assert_eq!(viewport.frame.scroll, Scroll::Y);
    assert!(
        viewport
            .frame
            .max_height
            .is_some_and(|height| height > 0. && height <= 300.)
    );
    let Node::Stack(content) = &viewport.children[0] else {
        panic!("no report content");
    };
    assert!(!content.frame.shrink);
    for i in 0..30 {
        assert!(has_text(&popover.content, &format!("Detail {i}")));
    }
    refresh_is_reachable(popover);
}
