use {
    super::*,
    crate::{Acting, ViewAct},
};

const PAGE: &str = r#"
return mod {
  init = function() return { added = 0 } end,
  update = function(state, msg) if msg == "add" then state.added += 1 end end,
  view = function(state)
    return ui.column {
      ui.text { "Added " .. state.added },
      ui.button { "Add", msg = "add" },
      ui.row { msg = "open-row", ui.text "Row one", ui.badge "new" },
      ui.input { id = "q", placeholder = "Search", change = "typed", submit = "go", keys = { up = "recall" } },
      ui.mount { "counter", id = "c" },
      ui.popover { open = true, dismiss = "close",
        content = ui.column { ui.item { "Apple", msg = "pick-apple" }, ui.item { "Pear", msg = "pick-pear" } },
        ui.button { "Fruit", msg = "toggle" },
      },
    }
  end,
}"#;

const COUNTER: &str = r#"
return mod {
  init = function() return { n = 0 } end,
  update = function(state, msg) if msg == "inc" then state.n += 1 end end,
  view = function(state) return ui.button { "Count " .. state.n, msg = "inc" } end,
}"#;

fn page() -> (tempfile::TempDir, Workspace) {
    let dir = app(&[("page", PAGE), ("counter", COUNTER)]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    (dir, workspace)
}

/// The ref of the first element with `role` and `name`.
fn find(workspace: &Workspace, role: &str, name: &str) -> String {
    let tree = workspace.view_tree("page").unwrap();
    tree.nodes
        .iter()
        .find(|node| node.role == role && node.name == name)
        .unwrap_or_else(|| panic!("no {role} {name:?} in\n{}", tree.lines()))
        .reference
        .clone()
}

#[test]
fn a_view_reads_as_roles_and_names_through_its_components() {
    let (_dir, workspace) = page();
    let lines = workspace.view_tree("page").unwrap().lines();
    for line in [
        r#"text "Added 0""#,
        r#"button "Add""#,
        r#"button "Row one new""#,
        r#"textbox "Search" value="""#,
        r#"button "Count 0""#,
        "popover",
        r#"  item "Pear""#,
    ] {
        assert!(lines.contains(line), "{line} in\n{lines}");
    }
    // The row's text names it, and is not listed again.
    assert!(!lines.contains(r#"text "Row one""#), "{lines}");
    assert!(
        workspace
            .view_tree("nobody")
            .unwrap_err()
            .contains("no mod named")
    );
}

#[test]
fn acting_sends_what_a_persons_act_would() {
    let (_dir, mut workspace) = page();
    let add = find(&workspace, "button", "Add");
    let Ok(Acting::Send { owner, action }) = workspace.view_act("page", &add, &ViewAct::Click)
    else {
        panic!("a click on Add sends");
    };
    assert_eq!(owner, "page");
    workspace.dispatch(&owner, &action);
    assert!(
        workspace
            .view_tree("page")
            .unwrap()
            .lines()
            .contains(r#""Added 1""#)
    );

    // A component's own button goes from the component.
    let count = find(&workspace, "button", "Count 0");
    let Ok(Acting::Send { owner, action }) = workspace.view_act("page", &count, &ViewAct::Click)
    else {
        panic!("a click on the counter sends");
    };
    assert_eq!(owner, "counter#c");
    workspace.dispatch(&owner, &action);
    find(&workspace, "button", "Count 1");

    let field = find(&workspace, "textbox", "Search");
    assert_eq!(
        workspace.view_act("page", &field, &ViewAct::Fill("pears".into())),
        Ok(Acting::Fill {
            owner: "page".into(),
            id: Some("q".into()),
            to: None,
            change: Some(Data::from("typed")),
            text: "pears".into(),
        })
    );
    for (key, msg) in [("enter", "go"), ("up", "recall")] {
        let Ok(Acting::FieldKey { msg: sent, .. }) =
            workspace.view_act("page", &field, &ViewAct::Press(key.into()))
        else {
            panic!("{key} on the field sends");
        };
        assert_eq!(sent, Data::from(msg));
    }

    // Escape inside the open popover dismisses it; an option is chosen by name.
    let pear = find(&workspace, "item", "Pear");
    let sends = |reference: &str, act: ViewAct| match workspace.view_act("page", reference, &act) {
        Ok(Acting::Send {
            action: Action::Send { msg, .. },
            ..
        }) => msg,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        sends(&pear, ViewAct::Press("escape".into())),
        Data::from("close")
    );
    let popover = find(&workspace, "popover", "");
    assert_eq!(
        sends(&popover, ViewAct::Select("pear".into())),
        Data::from("pick-pear")
    );
    assert_eq!(
        sends(&popover, ViewAct::Select("App".into())),
        Data::from("pick-apple")
    );
}

#[test]
fn an_act_that_cannot_be_says_why() {
    let (_dir, workspace) = page();
    let add = find(&workspace, "button", "Add");
    let field = find(&workspace, "textbox", "Search");
    let popover = find(&workspace, "popover", "");
    for (reference, act, problem) in [
        ("e9.9", ViewAct::Click, "no element `e9.9`"),
        ("nine", ViewAct::Click, "no element ref"),
        (add.as_str(), ViewAct::Fill("x".into()), "only a text field"),
        (
            field.as_str(),
            ViewAct::Press("f9".into()),
            "nothing there takes `f9`",
        ),
        (
            popover.as_str(),
            ViewAct::Select("plum".into()),
            "no one option named",
        ),
        ("e", ViewAct::Click, "nothing happens"),
    ] {
        let error = workspace.view_act("page", reference, &act).unwrap_err();
        assert!(error.contains(problem), "{reference} {act:?}: {error}");
    }
}
