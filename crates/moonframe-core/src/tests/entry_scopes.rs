//! A miniapp entry's own components and `style.luau`, and checking one.

use {
    super::*,
    crate::{
        Roots, Slot,
        entries::{Entries, EntryState, RootPolicy, Severity, check, check_in},
    },
    std::sync::{Arc, Mutex},
};

fn in_app(roots: &Roots) -> crate::entries::CheckOptions {
    crate::entries::CheckOptions {
        roots: Some(roots.clone()),
        ..Default::default()
    }
}

fn component(text: &str) -> String {
    format!("return component {{ build = function() return ui.text \"{text}\" end }}")
}

const USES_BADGE: &str =
    r#"local badge = use "badge"; return mod { view = function() return badge {} end }"#;

fn text(workspace: &Workspace, name: &str) -> String {
    let Some(Node::Text(text)) = workspace.slot(name).and_then(|slot| slot.view()) else {
        panic!(
            "no text for {name}: {:?}",
            workspace.slot(name).map(Slot::error)
        );
    };
    text.text.clone()
}

/// An app with the mod `plain` using `badge`, and the entry `user:mini`.
fn setup() -> (tempfile::TempDir, tempfile::TempDir, Entries) {
    let dir = app(&[("plain", USES_BADGE)]);
    write(dir.path(), "components/badge.luau", &component("app"));
    let external = tempfile::tempdir().unwrap();
    write(external.path(), "main.luau", USES_BADGE);
    write(
        external.path(),
        "components/badge.luau",
        &component("entry"),
    );
    let entries = Entries::default();
    entries
        .add_root("user", external.path(), RootPolicy::default())
        .unwrap();
    entries.add_entry("user", "mini", external.path()).unwrap();
    (dir, external, entries)
}

#[test]
fn an_entry_uses_its_own_components_first_and_reloads_them() {
    let (dir, external, entries) = setup();
    let mut workspace = Workspace::open(entries.apply(dir.path().into()), false);
    assert_eq!(text(&workspace, "user:mini"), "entry");
    assert_eq!(text(&workspace, "plain"), "app");
    // The entry's components stay out of the app's catalogue.
    assert_eq!(workspace.components().len(), 1);
    write(
        external.path(),
        "components/badge.luau",
        &component("edited"),
    );
    workspace.reload();
    assert_eq!(text(&workspace, "user:mini"), "edited");
    // Without its own, it falls back to the app's.
    fs::remove_file(external.path().join("components/badge.luau")).unwrap();
    workspace.reload();
    assert_eq!(text(&workspace, "user:mini"), "app");
}

#[test]
fn an_entry_stylesheet_is_scoped_to_its_mod_and_reports_errors() {
    let (dir, external, entries) = setup();
    write(
        external.path(),
        "style.luau",
        r##"return style { rules = { [".hot"] = { color = "#ff0000" } } }"##,
    );
    let mut workspace = Workspace::open(entries.apply(dir.path().into()), false);
    let statuses = Arc::new(Mutex::new(Vec::new()));
    let sink = statuses.clone();
    workspace.on_entry_status(Arc::new(move |status| sink.lock().unwrap().push(status)));
    let selectors =
        |workspace: &Workspace, name: &str| workspace.slot(name).unwrap().rules().selectors();
    assert!(selectors(&workspace, "user:mini").contains(&".hot".to_owned()));
    assert!(!selectors(&workspace, "plain").contains(&".hot".to_owned()));
    let first = statuses.lock().unwrap().last().unwrap().clone();
    assert_eq!(first.state, EntryState::Ok);

    // Colors belong to the app: the error is reported, the last rules stay.
    write(
        external.path(),
        "style.luau",
        r##"return style { colors = { accent = "#00ff00" } }"##,
    );
    workspace.reload();
    let last = statuses.lock().unwrap().last().unwrap().clone();
    assert_eq!(last.state, EntryState::Stale);
    assert!(last.error.unwrap().contains("only `rules`"));
    assert_ne!(last.revision, first.revision);
    assert!(selectors(&workspace, "user:mini").contains(&".hot".to_owned()));

    write(external.path(), "style.luau", "return style { rules = {} }");
    workspace.reload();
    assert_eq!(
        statuses.lock().unwrap().last().unwrap().state,
        EntryState::Ok
    );
    assert!(!selectors(&workspace, "user:mini").contains(&".hot".to_owned()));
}

#[test]
fn embedded_entry_files_wait_for_the_files_folder() {
    let entries = Entries::default();
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    entries
        .add_root("builtin", root.path(), RootPolicy::default())
        .unwrap();
    let files: crate::entries::EntryFiles = Arc::new(|folder: &Path| {
        let written = folder.join("hash");
        fs::create_dir_all(&written)?;
        fs::write(
            written.join("main.luau"),
            "return mod { view = function() return ui.text 'hi' end }",
        )?;
        Ok(written)
    });
    assert!(
        entries
            .add_entry_files("missing", "x", files.clone())
            .is_err()
    );
    entries
        .add_entry_files("builtin", "clock", files.clone())
        .unwrap();
    assert!(
        entries.snapshot().is_empty(),
        "nothing written before the folder is known"
    );
    assert!(entries.set_files_dir(cache.path()).is_empty());
    let entry = entries.snapshot().remove("builtin:clock").unwrap();
    assert_eq!(entry.path, cache.path().join("builtin/clock/hash"));
    // Once known, an entry is written at once.
    entries.add_entry_files("builtin", "timer", files).unwrap();
    assert!(entries.snapshot().contains_key("builtin:timer"));
}

#[test]
fn removing_a_waiting_embedded_entry_drops_it() {
    let entries = Entries::default();
    let root = tempfile::tempdir().unwrap();
    entries
        .add_root("builtin", root.path(), RootPolicy::default())
        .unwrap();
    entries
        .add_entry_files(
            "builtin",
            "clock",
            Arc::new(|_: &Path| Err(std::io::Error::other("never"))),
        )
        .unwrap();
    entries.remove_entry("builtin", "clock");
    assert!(entries.set_files_dir(root.path()).is_empty());
}

#[test]
fn check_reports_each_file_and_resolves_components_against_the_app() {
    let (dir, external, _) = setup();
    let policy = RootPolicy::default();
    assert_eq!(check(external.path(), &policy), Vec::new());

    // A component only the app brings: a warning alone, an error in an app
    // without it, nothing in one with it.
    fs::remove_file(external.path().join("components/badge.luau")).unwrap();
    let alone = check(external.path(), &policy);
    assert_eq!(alone.len(), 1, "{alone:?}");
    assert_eq!(alone[0].severity, Severity::Warning);
    let roots = Roots::from(dir.path());
    assert_eq!(
        check_in(external.path(), &policy, &in_app(&roots)),
        Vec::new()
    );
    let bare_app = app(&[]);
    let bare = Roots::from(bare_app.path());
    let missing = check_in(external.path(), &policy, &in_app(&bare));
    assert_eq!(missing[0].severity, Severity::Error);

    write(
        external.path(),
        "components/broken.luau",
        "return component {",
    );
    write(external.path(), "style.luau", "return style { fonts = 1 }");
    write(
        external.path(),
        "main.luau",
        "return mod { view = function() error('boom') end }",
    );
    let found = check_in(external.path(), &policy, &in_app(&roots));
    let files: Vec<_> = found
        .iter()
        .map(|found| found.file.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    assert!(files.contains(&"broken.luau".to_owned()), "{found:?}");
    assert!(files.contains(&"style.luau".to_owned()), "{found:?}");
    assert!(files.contains(&"main.luau".to_owned()), "{found:?}");
    assert!(found.iter().all(|found| found.severity == Severity::Error));

    let empty = tempfile::tempdir().unwrap();
    assert_eq!(check(empty.path(), &policy).len(), 1);
}
