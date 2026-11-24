//! Drive the example through the shell's real view and message paths.
use super::*;

fn shows(workspace: &Workspace, label: &str) -> bool {
    fn contains(node: &Node, label: &str) -> bool {
        match node {
            Node::Text(text) => text.text == label,
            Node::Stack(stack) => stack.children.iter().any(|child| contains(child, label)),
            Node::Popover(popover) => {
                popover.anchor.iter().any(|child| contains(child, label))
                    || (popover.open && contains(&popover.content, label))
            },
            _ => false,
        }
    }
    let slot = workspace.slot(SHELL).unwrap();
    assert!(slot.error().is_none(), "{:?}", slot.error());
    assert!(slot.broken().is_empty(), "{:?}", slot.broken());
    contains(slot.view().unwrap(), label)
}

fn overview() -> (Workspace, tempfile::TempDir) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = tempfile::tempdir().unwrap();
    let roots = crate::Roots::profile(
        root.join("app"),
        [],
        Some(crate::AppLayer {
            path: root.join("examples/t3-overview"),
            keep: Vec::new(),
        }),
        None,
        profile.path().to_owned(),
    );
    (Workspace::open_with(roots, false, None), profile)
}

#[test]
fn t3_overview_filters_folds_and_expands_live_sessions() {
    let (mut workspace, _profile) = overview();
    assert!(shows(&workspace, "Connecting to T3 Code…"));
    let sample = mlua::Lua::new();
    let snapshot = sample
        .load(
            r##"return { available = true, total = 2, threads = {
                {id="a", title="Review theme", group="waiting", status="running", project="Desktop",
                 provider="codex", updatedAt="today", worktree="/work/a", issues={}, pullRequests={
                    {label="#42", title="desktop", url="https://example.com/pull/42"}}},
                {id="b", title="Fix scroll", group="running", status="starting", project="Mobile",
                 updatedAt="today", issues={}, pullRequests={}},
            }}"##,
        )
        .eval()
        .unwrap();
    workspace.set_host("t3", Data::read(&snapshot).unwrap());
    assert!(shows(&workspace, "Fix scroll"));
    for (key, value, label, visible) in [
        ("tab", "Pull requests", "Fix scroll", false),
        ("thread", "a", "codex · today · /work/a", true),
        ("fold", "waiting", "Review theme", false),
        ("fold", "waiting", "Review theme", true),
        ("tab", "Issues", "No matching sessions", true),
        ("tab", "Worktrees", "Review theme", true),
    ] {
        workspace.send(SHELL, SHELL, Data::map([(key, Data::from(value))]));
        assert_eq!(shows(&workspace, label), visible, "{key} = {value}");
    }
}

fn click(workspace: &mut Workspace, label: &str) {
    fn find(node: &Node, label: &str) -> Option<Action> {
        match node {
            Node::Button(button) if button.label == label && !button.disabled => {
                button.action.clone()
            },
            Node::Stack(stack) => stack.children.iter().find_map(|child| find(child, label)),
            _ => None,
        }
    }
    let action = find(workspace.slot(SHELL).unwrap().view().unwrap(), label)
        .unwrap_or_else(|| panic!("missing button {label}"));
    workspace.dispatch(SHELL, &action);
}

#[test]
fn t3_overview_guides_first_run_and_connects_from_the_shell() {
    let (mut workspace, _profile) = overview();
    for (step, title) in [
        ("install", "Install T3 Code"),
        ("start", "Start T3 Code"),
        ("pair", "Connect to T3 Code"),
    ] {
        workspace.set_host(
            "t3",
            Data::map([
                ("available", Data::from(false)),
                ("setup", Data::from(step)),
            ]),
        );
        assert!(shows(&workspace, title));
    }
    click(&mut workspace, "Connect");
    let outbox = workspace.take_outbox();
    let [(provider, sender, msg)] = outbox.as_slice() else {
        panic!("expected a pairing request")
    };
    assert_eq!((provider.as_str(), sender.as_str()), ("t3", SHELL));
    assert_eq!(msg, &Data::from("connect"));
}

#[test]
fn t3_composer_submits_through_the_control_and_handles_receipts() {
    let (mut workspace, _profile) = overview();
    let lua = mlua::Lua::new();
    let sample = lua
        .load(
            r#"return {available=true, environmentId="env", writer="shell", ticket="first",
        total=1, threads={{id="t", title="Review", project="App", status="running", group="running", issues={}, pullRequests={}, updatedAt="today"}},
        projects={{id="p", label="App"}, {id="q", label="Docs"}}, results={}}"#,
        )
        .eval()
        .unwrap();
    let Data::Map(mut data) = Data::read(&sample).unwrap() else {
        panic!()
    };
    workspace.set_host("t3", Data::Map(data.clone()));
    click(&mut workspace, "New chat");
    assert!(shows(&workspace, "Choose project"));
    workspace.send(SHELL, SHELL, Data::map([("project", Data::from("p"))]));
    workspace.send(
        SHELL,
        SHELL,
        Data::map([
            ("msg", Data::from("draft")),
            ("value", Data::from("Review")),
        ]),
    );
    click(&mut workspace, "Start chat");
    let outbox = workspace.take_outbox();
    let [(provider, sender, Data::Map(msg))] = outbox.as_slice() else {
        panic!()
    };
    assert_eq!(provider.as_str(), "t3");
    assert_eq!(sender.as_str(), SHELL);
    assert_eq!(msg["action"], Data::from("start"));
    assert_eq!(msg["text"], Data::from("Review"));
    assert_eq!(msg["target"], Data::from("p"));
    let Data::String(id) = &msg["id"] else {
        panic!()
    };
    data.insert(
        "results".into(),
        Data::map([(
            id.as_str(),
            Data::map([
                ("status", Data::from("error")),
                ("error", Data::from("Try again")),
            ]),
        )]),
    );
    workspace.set_host("t3", Data::Map(data.clone()));
    assert!(shows(&workspace, "Try again"));
    click(&mut workspace, "Start chat");
    let outbox = workspace.take_outbox();
    let [(_, _, Data::Map(retry))] = outbox.as_slice() else {
        panic!()
    };
    assert_ne!(retry["id"], msg["id"]);
    assert_eq!(retry["text"], msg["text"]);
    let Data::String(id) = &retry["id"] else {
        panic!()
    };
    data.insert(
        "results".into(),
        Data::map([(
            id.as_str(),
            Data::map([
                ("status", Data::from("started")),
                ("threadId", Data::from("t")),
            ]),
        )]),
    );
    workspace.set_host("t3", Data::Map(data));
    assert!(shows(&workspace, "Chat started"));
    assert!(!shows(&workspace, "Start chat"));
    click(&mut workspace, "Reply");
    assert!(shows(&workspace, "Reply to Review"));
    workspace.send(
        SHELL,
        SHELL,
        Data::map([
            ("msg", Data::from("draft")),
            ("value", Data::from("Follow up")),
        ]),
    );
    click(&mut workspace, "Send message");
    let outbox = workspace.take_outbox();
    let [(_, _, Data::Map(reply))] = outbox.as_slice() else {
        panic!("expected a follow-up")
    };
    assert_eq!(reply["action"], Data::from("send"));
    assert_eq!(reply["target"], Data::from("t"));
}
