//! Element problems reported from live views: once, with a suggestion,
//! cleared once fixed.

use {
    super::*,
    crate::{
        ViewDiagnostic, ViewLevel,
        entries::{Entries, EntryStatus, RootPolicy},
    },
    std::sync::{Arc, Mutex},
};

/// Draws a padded row only once its provider has published.
const DATA_ONLY: &str = r#"return mod {
    view = function(_, app)
        local data = app["user:fuubar"]
        if not data then return ui.text "waiting" end
        return ui.column { ui.row { padding = 8, ui.text(data.title) } }
    end,
}"#;

const FIXED: &str = r#"return mod {
    view = function(_, app)
        local data = app["user:fuubar"]
        if not data then return ui.text "waiting" end
        return ui.column { ui.row { padding = "small", ui.text(data.title) } }
    end,
}"#;

fn data(title: &str) -> Data {
    Data::map([("title", Data::from(title))])
}

#[test]
fn a_data_only_mistake_is_reported_once_and_cleared_after_a_fix() {
    let dir = app(&[]);
    let external = tempfile::tempdir().unwrap();
    write(external.path(), "main.luau", DATA_ONLY);
    let entries = Entries::default();
    entries
        .add_root("user", external.path(), RootPolicy::default())
        .unwrap();
    entries
        .add_entry("user", "fuubar", external.path())
        .unwrap();
    let mut workspace = Workspace::open(entries.apply(dir.path().into()), false);
    let statuses: Arc<Mutex<Vec<EntryStatus>>> = Arc::default();
    let sink = statuses.clone();
    workspace.on_entry_status(Arc::new(move |status| sink.lock().unwrap().push(status)));
    let seen: Arc<Mutex<Vec<ViewDiagnostic>>> = Arc::default();
    let heard = seen.clone();
    workspace.on_view_diagnostic(Arc::new(move |found| heard.lock().unwrap().push(found)));
    assert!(
        statuses
            .lock()
            .unwrap()
            .iter()
            .all(|status| status.view_errors.is_empty())
    );
    let before = statuses.lock().unwrap().len();

    workspace.set_host("user:fuubar", data("hi"));
    let found = seen.lock().unwrap().clone();
    assert_eq!(found.len(), 1, "{found:?}");
    let found = &found[0];
    assert_eq!(found.module, "user:fuubar");
    assert_eq!(found.path, "column[1].row");
    assert_eq!(found.element.as_deref(), Some("row"));
    assert_eq!(found.key.as_deref(), Some("padding"));
    assert_eq!(found.value.as_deref(), Some("8"));
    assert_eq!(found.line, Some(5));
    assert_eq!(found.level, ViewLevel::Broken);
    let suggestion = found.suggestion.as_deref().unwrap();
    assert!(
        suggestion.contains("none|small|normal|large"),
        "{suggestion}"
    );
    assert!(suggestion.contains("styles"), "{suggestion}");
    {
        let statuses = statuses.lock().unwrap();
        assert_eq!(statuses.len(), before + 1, "one status for the new problem");
        assert_eq!(statuses.last().unwrap().view_errors, vec![found.clone()]);
    }
    // The warning sign's tooltip says the same.
    let Some(Node::Stack(column)) = workspace.slot("user:fuubar").unwrap().view() else {
        panic!("no column");
    };
    let Node::Problem(tooltip) = &column.children[0] else {
        panic!("no problem");
    };
    assert!(tooltip.contains(suggestion), "{tooltip}");

    // Drawn again and again: told once.
    for title in ["a", "b", "c"] {
        workspace.set_host("user:fuubar", data(title));
    }
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(statuses.lock().unwrap().len(), before + 1);

    write(external.path(), "main.luau", FIXED);
    workspace.reload();
    let last = statuses.lock().unwrap().last().unwrap().clone();
    assert!(last.view_errors.is_empty(), "{last:?}");
    assert_eq!(seen.lock().unwrap().len(), 1);

    // Broken again: told again.
    write(external.path(), "main.luau", DATA_ONLY);
    workspace.reload();
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(
        statuses.lock().unwrap().last().unwrap().view_errors.len(),
        1
    );
}

fn first(source: &str) -> ViewDiagnostic {
    let dir = app(&[("mistake", source)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let seen: Arc<Mutex<Vec<ViewDiagnostic>>> = Arc::default();
    let heard = seen.clone();
    workspace.on_view_diagnostic(Arc::new(move |found| heard.lock().unwrap().push(found)));
    let found = seen.lock().unwrap();
    found
        .iter()
        .find(|found| found.module == "mistake")
        .cloned()
        .unwrap_or_else(|| panic!("nothing reported: {found:?}"))
}

#[test]
fn app_mods_are_reported_with_suggestions() {
    let selected = first(
        r#"return mod { view = function() return ui.row { ui.button { label = "a", selected = true } } end }"#,
    );
    assert_eq!(selected.key.as_deref(), Some("selected"));
    assert_eq!(selected.element.as_deref(), Some("button"));
    assert!(selected.suggestion.unwrap().contains("variant"));

    let icon =
        first(r#"return mod { view = function() return ui.row { ui.icon { "chek" } } end }"#);
    assert_eq!(icon.value.as_deref(), Some("\"chek\""));
    assert!(icon.suggestion.unwrap().contains("check"));

    let tone = first(
        r#"return mod { view = function() return ui.row { ui.text { "a", tone = "mutd" } } end }"#,
    );
    assert!(tone.suggestion.unwrap().contains("`muted`"));

    let typo = first(
        r#"return mod { view = function() return ui.row { ui.button { lable = "a" } } end }"#,
    );
    assert!(typo.suggestion.unwrap().contains("`label`"));
}

#[test]
fn the_shell_is_reported_too() {
    let dir = app(&[]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function() return ui.column { ui.row { gap = 4 } } end }"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    let seen: Arc<Mutex<Vec<ViewDiagnostic>>> = Arc::default();
    let heard = seen.clone();
    workspace.on_view_diagnostic(Arc::new(move |found| heard.lock().unwrap().push(found)));
    let found = seen.lock().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].module, SHELL);
    assert_eq!(found[0].key.as_deref(), Some("gap"));
}

#[test]
fn snapshot_view_returns_the_drawn_tree_from_any_thread() {
    let dir = app(&[]);
    let external = tempfile::tempdir().unwrap();
    write(external.path(), "main.luau", DATA_ONLY);
    let entries = Entries::default();
    entries
        .add_root("user", external.path(), RootPolicy::default())
        .unwrap();
    entries
        .add_entry("user", "fuubar", external.path())
        .unwrap();
    let mut workspace = Workspace::open(entries.apply(dir.path().into()), false);
    assert_eq!(entries.snapshot_view("user:fuubar"), None);
    workspace.share_views(entries.clone());
    let reader = entries.clone();
    let first = std::thread::spawn(move || reader.snapshot_view("user:fuubar"))
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(first.tree.get("kind"), Some(&Data::from("text")));
    assert_eq!(first.tree.get("text"), Some(&Data::from("waiting")));
    assert_eq!(first.reads, Some(vec!["user:fuubar".to_owned()]));
    assert!(first.diagnostics.is_empty());

    workspace.set_host("user:fuubar", data("hi"));
    let drawn = entries.snapshot_view("user:fuubar").unwrap();
    assert!(drawn.draws > first.draws);
    let Some(Data::List(children)) = drawn.tree.get("children") else {
        panic!("no children: {:?}", drawn.tree);
    };
    assert_eq!(
        children[0].get("path"),
        Some(&Data::from("column[1].problem"))
    );
    assert_eq!(drawn.diagnostics.len(), 1);
    assert_eq!(drawn.diagnostics[0].path, "column[1].row");
    assert!(!drawn.truncated);

    entries.remove_entry("user", "fuubar");
    assert_eq!(entries.snapshot_view("user:fuubar"), None);
}
