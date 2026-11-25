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

/// Reaches every mod, as the app's own providers do.
fn anyone(_: &str) -> bool {
    true
}

/// No field drawn: each shows its `value`.
fn no_live(_: &str, _: &str) -> Option<String> {
    None
}

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
    let tree = workspace.view_tree("page", &anyone, &no_live).unwrap();
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
    let lines = workspace
        .view_tree("page", &anyone, &no_live)
        .unwrap()
        .lines();
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
            .view_tree("nobody", &anyone, &no_live)
            .unwrap_err()
            .contains("no mod named")
    );
}

#[test]
fn acting_sends_what_a_persons_act_would() {
    let (_dir, mut workspace) = page();
    let add = find(&workspace, "button", "Add");
    let Ok(Acting::Send { owner, action }) =
        workspace.view_act("page", &anyone, &add, &ViewAct::Click)
    else {
        panic!("a click on Add sends");
    };
    assert_eq!(owner, "page");
    workspace.dispatch(&owner, &action);
    assert!(
        workspace
            .view_tree("page", &anyone, &no_live)
            .unwrap()
            .lines()
            .contains(r#""Added 1""#)
    );

    // A component's own button goes from the component.
    let count = find(&workspace, "button", "Count 0");
    let Ok(Acting::Send { owner, action }) =
        workspace.view_act("page", &anyone, &count, &ViewAct::Click)
    else {
        panic!("a click on the counter sends");
    };
    assert_eq!(owner, "counter#c");
    workspace.dispatch(&owner, &action);
    find(&workspace, "button", "Count 1");

    let field = find(&workspace, "textbox", "Search");
    assert_eq!(
        workspace.view_act("page", &anyone, &field, &ViewAct::Fill("pears".into())),
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
            workspace.view_act("page", &anyone, &field, &ViewAct::Press(key.into()))
        else {
            panic!("{key} on the field sends");
        };
        assert_eq!(sent, Some(Data::from(msg)));
    }

    // Escape inside the open popover dismisses it; an option is chosen by name.
    let pear = find(&workspace, "item", "Pear");
    let sends = |reference: &str, act: ViewAct| match workspace
        .view_act("page", &anyone, reference, &act)
    {
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
        let error = workspace
            .view_act("page", &anyone, reference, &act)
            .unwrap_err();
        assert!(error.contains(problem), "{reference} {act:?}: {error}");
    }
}

#[test]
fn a_read_stays_within_the_mods_it_may_reach() {
    let (_dir, workspace) = page();
    // Only the page itself: its mounted counter is out of reach.
    let own = |name: &str| name == "page";
    let tree = workspace.view_tree("page", &own, &no_live).unwrap();
    assert!(tree.lines().contains(r#"button "Add""#));
    assert!(!tree.lines().contains("Count"), "{}", tree.lines());
    assert!(!tree.owners.contains("counter#c"));
    let count = find(&workspace, "button", "Count 0");
    let error = workspace
        .view_act("page", &own, &count, &ViewAct::Click)
        .unwrap_err();
    assert!(error.contains("no element"), "{error}");
    let refused = workspace
        .view_tree("counter#c", &own, &no_live)
        .unwrap_err();
    assert!(refused.contains("out of reach"), "{refused}");
    // Everything it read is named, for its problems.
    let all = workspace.view_tree("page", &anyone, &no_live).unwrap();
    assert!(all.owners.contains("counter#c") && all.owners.contains("page"));
}

#[test]
fn a_mod_that_mounts_itself_is_read_once() {
    let looping = r#"return mod { view = function()
        return ui.column { ui.text "loop", ui.mount "loop", ui.mount "loop" }
    end }"#;
    let dir = app(&[("loop", looping)]);
    let workspace = Workspace::open(dir.path(), false);
    let tree = workspace.view_tree("loop", &anyone, &no_live).unwrap();
    assert_eq!(tree.nodes.len(), 1, "{}", tree.lines());
}

#[test]
fn a_fields_live_text_shows_over_its_value_and_a_disabled_field_takes_no_key() {
    let form = r#"return mod { view = function()
        return ui.column {
          ui.input { id = "q", placeholder = "Search", value = "old" },
          ui.input { id = "off", placeholder = "Off", disabled = true, submit = "go" },
          ui.input { placeholder = "Bare" },
        }
    end }"#;
    let dir = app(&[("form", form)]);
    let workspace = Workspace::open(dir.path(), false);
    let live = |owner: &str, id: &str| (owner == "form" && id == "q").then(|| "typed".to_owned());
    let lines = workspace.view_tree("form", &anyone, &live).unwrap().lines();
    assert!(
        lines.contains(r#"textbox "Search" value="typed""#),
        "{lines}"
    );
    let tree = workspace.view_tree("form", &anyone, &no_live).unwrap();
    let reference = |name: &str| {
        tree.nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap()
            .reference
            .clone()
    };
    let error = workspace
        .view_act(
            "form",
            &anyone,
            &reference("Off"),
            &ViewAct::Press("enter".into()),
        )
        .unwrap_err();
    assert!(error.contains("disabled"), "{error}");
    let error = workspace
        .view_act(
            "form",
            &anyone,
            &reference("Bare"),
            &ViewAct::Fill("x".into()),
        )
        .unwrap_err();
    assert!(error.contains("no `id` and no `change`"), "{error}");
}

#[test]
fn an_open_layer_takes_a_key_before_its_field_and_an_address_bar_goes_on_enter() {
    let dialog = r#"return mod { view = function()
        return ui.column {
          ui.overlay { open = true, keys = { enter = "confirm" },
            content = ui.column { ui.input { id = "name", placeholder = "Name", submit = "named" } } },
          ui.input { id = "address", placeholder = "Address", browser = "docs" },
        }
    end }"#;
    let dir = app(&[("dialog", dialog)]);
    let workspace = Workspace::open(dir.path(), false);
    let tree = workspace.view_tree("dialog", &anyone, &no_live).unwrap();
    let reference = |name: &str| {
        tree.nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("no {name} in\n{}", tree.lines()))
            .reference
            .clone()
    };
    let enter = ViewAct::Press("enter".into());
    match workspace.view_act("dialog", &anyone, &reference("Name"), &enter) {
        Ok(Acting::Send {
            action: Action::Send { msg, .. },
            ..
        }) => {
            assert_eq!(msg, Data::from("confirm"));
        },
        other => panic!("{other:?}"),
    }
    match workspace.view_act("dialog", &anyone, &reference("Address"), &enter) {
        Ok(Acting::FieldKey {
            msg: None,
            browser: Some(browser),
            ..
        }) => {
            assert_eq!(browser, "docs");
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_shortcut_field_records_the_key_and_a_copy_button_copies() {
    let controls = r#"return mod { view = function()
        return ui.column {
          ui.input { id = "shortcut", placeholder = "Shortcut", capture = "recorded" },
          ui.button { "Copy code", copy = "ABCD-1234" },
          ui.button { "Copy and close", copy = "link", msg = "close" },
        }
    end }"#;
    let dir = app(&[("controls", controls)]);
    let workspace = Workspace::open(dir.path(), false);
    let tree = workspace.view_tree("controls", &anyone, &no_live).unwrap();
    let reference = |name: &str| {
        tree.nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("no {name} in\n{}", tree.lines()))
            .reference
            .clone()
    };
    let act =
        |name: &str, act: ViewAct| workspace.view_act("controls", &anyone, &reference(name), &act);
    match act("Shortcut", ViewAct::Press("cmd-shift-k".into())) {
        Ok(Acting::Send {
            action:
                Action::Send {
                    msg: Data::Map(msg),
                    ..
                },
            ..
        }) => {
            assert_eq!(msg.get("msg"), Some(&Data::from("recorded")));
            assert_eq!(msg.get("keys"), Some(&Data::from("shift-cmd-k")));
        },
        other => panic!("{other:?}"),
    }
    let lone = act("Shortcut", ViewAct::Press("a".into())).unwrap_err();
    assert!(lone.contains("no shortcut"), "{lone}");
    assert_eq!(
        act("Copy code", ViewAct::Click),
        Ok(Acting::Copy {
            owner: "controls".into(),
            text: "ABCD-1234".into(),
            then: None
        })
    );
    match act("Copy and close", ViewAct::Click) {
        Ok(Acting::Copy {
            text,
            then: Some(Action::Send { msg, .. }),
            ..
        }) => {
            assert_eq!((text.as_str(), msg), ("link", Data::from("close")));
        },
        other => panic!("{other:?}"),
    }
}
