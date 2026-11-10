use {
    super::*,
    crate::entries::{Entries, EntryState, RootPolicy},
    std::sync::{Arc, Mutex},
};

const MOD: &str = r#"return mod {
    init = function(previous) return previous or { count = 0 } end,
    subscribe = { "provider/user:counter" },
    update = function(state, msg)
        if msg == "add" then state.count += 1
        elseif msg == "send" then return state, {{ to = "provider:user:counter", msg = state.count }}
        elseif type(msg) == "table" and msg.topic then state.count = msg.value end
    end,
    view = function(state) return ui.text(tostring(state.count)) end,
}"#;

fn text(workspace: &Workspace, name: &str) -> String {
    let Some(Node::Text(text)) = workspace.slot(name).and_then(|slot| slot.view()) else {
        panic!("no text for {name}");
    };
    text.text.clone()
}

#[test]
fn selected_entries_mount_route_and_persist_with_the_namespace() {
    let dir = app(&[]);
    let external = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function() return ui.column {
        ui.mount "user:counter", ui.mount { "other:counter", id = "second" }
    } end }"#,
    );
    write(external.path(), "counter/main.luau", MOD);
    let entries = Entries::default();
    for namespace in ["user", "other"] {
        entries
            .add_root(namespace, external.path(), RootPolicy::default())
            .unwrap();
        entries
            .add_entry(namespace, "counter", external.path().join("counter"))
            .unwrap();
    }
    let saved = Memory::default();
    let roots = || entries.apply(dir.path().into());
    let mut workspace = Workspace::open_with(roots(), false, Some(Box::new(saved.clone())));
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    workspace.send("shell", "user:counter", Data::from("add"));
    workspace.send("shell", "other:counter#second", Data::from("add"));
    assert_eq!(text(&workspace, "user:counter"), "1");
    assert_eq!(text(&workspace, "other:counter"), "0");
    workspace.send("shell", "user:counter", Data::from("send"));
    assert!(
        workspace
            .take_outbox()
            .iter()
            .any(|(provider, _, value)| provider == "user:counter" && value == &Data::Integer(1))
    );
    workspace.set_host("user:counter", Data::Integer(7));
    assert_eq!(text(&workspace, "user:counter"), "7");
    workspace.flush();
    assert!(saved.0.borrow().0.contains_key("mod:user:counter"));
    assert!(saved.0.borrow().0.contains_key("mod:other:counter#second"));
    drop(workspace);
    let mut workspace = Workspace::open_with(roots(), false, Some(Box::new(saved)));
    assert_eq!(text(&workspace, "user:counter"), "7");
    entries.remove_entry("user", "counter");
    workspace.set_roots(roots());
    assert!(workspace.slot("user:counter").is_none());
    entries
        .add_entry("user", "counter", external.path().join("counter"))
        .unwrap();
    workspace.set_roots(roots());
    assert_eq!(text(&workspace, "user:counter"), "7");
    entries.remove_root("other");
    workspace.set_roots(roots());
    assert!(workspace.slot("other:counter").is_none());
}

#[test]
fn failed_mod_parse_init_and_first_view_keep_the_old_code_running() {
    let dir = app(&[]);
    let external = tempfile::tempdir().unwrap();
    write(external.path(), "main.luau", MOD);
    let entries = Entries::default();
    entries
        .add_root("user", external.path(), RootPolicy::default())
        .unwrap();
    entries
        .add_entry("user", "counter", external.path())
        .unwrap();
    let mut workspace = Workspace::open(entries.apply(dir.path().into()), false);
    let statuses = Arc::new(Mutex::new(Vec::new()));
    let sink = statuses.clone();
    workspace.on_entry_status(Arc::new(move |status| sink.lock().unwrap().push(status)));
    for bad in [
        "return mod {",
        "return mod { init = function() error('bad init') end, view = function() return ui.text 'bad' end }",
        "return mod { view = function() error('bad view') end }",
    ] {
        write(external.path(), "main.luau", bad);
        workspace.reload();
        let last = statuses.lock().unwrap().last().unwrap().clone();
        assert_eq!(last.state, EntryState::Stale);
        assert!(last.error.is_some());
        workspace.send("shell", "user:counter", Data::from("add"));
    }
    assert_eq!(text(&workspace, "user:counter"), "3");
    write(external.path(), "main.luau", MOD);
    workspace.reload();
    assert_eq!(
        statuses.lock().unwrap().last().unwrap().state,
        EntryState::Ok
    );
}

#[test]
fn registry_changes_notify_outside_the_lock_and_reserved_names_fail() {
    let entries = Entries::default();
    let reader = entries.clone();
    entries.on_change(move || {
        let _ = reader.snapshot();
    });
    let dir = tempfile::tempdir().unwrap();
    assert!(
        entries
            .add_root("provider", dir.path(), RootPolicy::default())
            .is_err()
    );
    entries
        .add_root("user", dir.path(), RootPolicy::default())
        .unwrap();
    assert!(entries.add_entry("user", "../escape", dir.path()).is_err());
    assert!(entries.add_entry("missing", "id", dir.path()).is_err());
    entries.add_entry("user", "id", dir.path()).unwrap();
    entries.remove_entry("user", "id");
    assert!(entries.snapshot().is_empty());
}
