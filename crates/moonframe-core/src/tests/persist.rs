//! `persist`: what a mod keeps across launches.

use {
    super::{app, panel_text},
    crate::{Data, SHELL, Workspace},
    std::collections::BTreeMap,
};

const KEEPER: &str = r#"
return mod {
  persist = { "count" },
  init = function(previous)
    return previous or { count = 0, hover = false }
  end,
  update = function(state, msg)
    if msg == "add" then state.count += 1; state.hover = true end
  end,
  view = function(state)
    return ui.panel { title = "Keeper", ui.text(`count {state.count}`) }
  end,
}
"#;

const FORGETFUL: &str = r#"
return mod {
  init = function(previous) return previous or { seen = 0 } end,
  update = function(state, msg) state.seen += 1 end,
  view = function(state) return ui.text(`seen {state.seen}`) end,
}
"#;

fn count(n: i64) -> Data {
    Data::Map(BTreeMap::from([("count".to_owned(), Data::Integer(n))]))
}

#[test]
fn a_mod_keeps_the_keys_it_names_and_gets_them_back() {
    let dir = app(&[("keeper", KEEPER), ("forgetful", FORGETFUL)]);
    let mut workspace = Workspace::open(dir.path(), false);
    assert_eq!(workspace.persisted().get("keeper"), Some(&count(0)));
    workspace.send("keeper", "keeper", Data::from("add"));
    workspace.send("forgetful", "forgetful", Data::from("go"));
    let kept = workspace.persisted();
    // Only the named key, and only mods that ask; the shell does not.
    assert_eq!(kept.get("keeper"), Some(&count(1)));
    assert!(!kept.contains_key("forgetful"));
    assert!(!kept.contains_key(SHELL));

    let next = Workspace::open_saved(dir.path(), false, kept);
    assert_eq!(panel_text(&next, "keeper"), "count 1");
}

#[test]
fn what_a_missing_or_broken_mod_kept_is_not_lost() {
    let dir = app(&[("keeper", "return mod { persist = true, view = ")]);
    let saved = BTreeMap::from([
        ("keeper".to_owned(), count(5)),
        ("gone".to_owned(), count(7)),
    ]);
    let workspace = Workspace::open_saved(dir.path(), false, saved.clone());
    assert!(workspace.slot("keeper").unwrap().error().is_some());
    assert_eq!(workspace.persisted(), saved);
}

#[test]
fn persist_must_be_true_or_a_list_of_keys() {
    let dir = app(&[(
        "keeper",
        r#"return mod { persist = "count", view = function() return ui.text "x" end }"#,
    )]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("keeper")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("persist"), "{error}");
}
