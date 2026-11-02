use super::*;

fn button_action(workspace: &Workspace, name: &str) -> Action {
    let Some(Node::Panel(panel)) = workspace.slot(name).unwrap().view() else {
        panic!("{name} drew no panel");
    };
    let Node::Button(button) = &panel.children[1] else {
        panic!("no button");
    };
    button.action.clone().unwrap()
}

#[test]
fn loads_the_shell_and_places_mods_in_regions() {
    let dir = app(&[("counter", COUNTER)]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.region("left"), ["counter"]);
    let Some(Node::Stack(column)) = workspace.shell().view() else {
        panic!("no column");
    };
    let Node::Split(split) = &column.children[0] else {
        panic!("no split");
    };
    assert_eq!(split.direction, Direction::Row);
    assert_eq!(split.panes.len(), 2);
    assert_eq!(split.panes[0].size, Some(200.0));
    assert!(split.panes[0].fixed && split.panes[0].visible);
    assert_eq!(panel_text(&workspace, "counter"), "count 0");
}

#[test]
fn messages_update_state_and_send_on() {
    let dir = app(&[("counter", COUNTER)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let action = button_action(&workspace, "counter");
    assert_eq!(workspace.dispatch("counter", &action), None);
    assert_eq!(panel_text(&workspace, "counter"), "count 1");
    // The counter told the shell to hide the left pane.
    let Some(Node::Stack(column)) = workspace.shell().view() else {
        panic!("no column");
    };
    let Node::Split(split) = &column.children[0] else {
        panic!("no split");
    };
    assert!(!split.panes[0].visible);
}

#[test]
fn commands_go_back_to_the_app() {
    let dir = app(&[]);
    let mut workspace = Workspace::open(dir.path(), false);
    let quit = Action::Command(Command::Quit);
    assert_eq!(workspace.dispatch(SHELL, &quit), Some(Command::Quit));
}

#[test]
fn reload_keeps_state_and_picks_up_new_mods() {
    let dir = app(&[("counter", COUNTER)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let action = button_action(&workspace, "counter");
    workspace.dispatch("counter", &action);
    write(
        dir.path(),
        "mods/counter/main.luau",
        &COUNTER.replace("`count {state.count}`", "`total {state.count}`"),
    );
    write(dir.path(), "mods/other/main.luau", COUNTER);
    workspace.reload();
    assert_eq!(panel_text(&workspace, "counter"), "total 1");
    assert_eq!(workspace.region("left"), ["counter", "other"]);
}

#[test]
fn a_broken_edit_keeps_the_last_good_view() {
    let dir = app(&[("counter", COUNTER)]);
    let mut workspace = Workspace::open(dir.path(), false);
    write(dir.path(), "mods/counter/main.luau", "return mod { view = ");
    workspace.reload();
    let problems = workspace.problems();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].0, "counter");
    assert_eq!(panel_text(&workspace, "counter"), "count 0");
}

#[test]
fn typos_are_reported_where_they_are() {
    let source = r#"return mod { view = function()
        return ui.column { ui.row { ui.button { lable = "x" } } }
    end }"#;
    let dir = app(&[("typo", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = broken(&workspace, "typo");
    assert!(
        error.contains("column[1].row[1].button: unknown key `lable`"),
        "{error}"
    );
}

#[test]
fn icons_are_checked_against_the_bundled_set() {
    assert!(
        crate::ICONS.is_sorted(),
        "ICONS must stay sorted for binary search"
    );
    let source = r#"return mod { view = function()
        return ui.row { ui.icon "cpu", ui.icon { name = "network", tone = "muted" } }
    end }"#;
    let dir = app(&[("icons", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("icons").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(row)) = slot.view() else {
        panic!("expected a row");
    };
    let paths: Vec<_> = row
        .children
        .iter()
        .map(|child| match child {
            Node::Icon(icon) => icon.name.path(),
            other => panic!("expected an icon, got {other:?}"),
        })
        .collect();
    assert_eq!(paths, ["icons/cpu.svg", "icons/network.svg"]);
}

#[test]
fn data_crosses_both_ways() {
    let lua = mlua::Lua::new();
    let value: mlua::Value = lua
        .load("return { name = 'a', items = { 1, 2.5, true }, nested = { x = {} } }")
        .eval()
        .unwrap();
    let data = Data::read(&value).unwrap();
    let back = Data::read(&data.to_lua(&lua).unwrap()).unwrap();
    assert_eq!(data, back);
    let mixed: mlua::Value = lua.load("return { 1, x = 2 }").eval().unwrap();
    assert!(Data::read(&mixed).is_err());
    let function: mlua::Value = lua.load("return { f = function() end }").eval().unwrap();
    assert!(Data::read(&function).is_err());
}

#[test]
fn bad_mod_names_are_reported() {
    let dir = app(&[("Bad Name", COUNTER)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("Bad Name")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("not a mod name"), "{error}");
}

#[test]
fn a_nil_child_does_not_hide_the_ones_after_it() {
    let source = r#"return mod { view = function()
  local none = nil
  return ui.row { "a", none, none, none, "b", false, "c", float = { top = -4, right = -4 }, max_width = 200 }
end }"#;
    let Node::Stack(row) = view_of(source).unwrap() else {
        panic!("expected a row");
    };
    let texts: Vec<&str> = row
        .children
        .iter()
        .map(|child| match child {
            Node::Text(text) => text.text.as_str(),
            other => panic!("expected text, got {other:?}"),
        })
        .collect();
    assert_eq!(texts, ["a", "b", "c"]);
    assert_eq!(row.frame.max_width, Some(200.0));
    let both = r#"return mod { view = function() return ui.column { scroll = "both" } end }"#;
    let Node::Stack(column) = view_of(both).unwrap() else {
        panic!("expected a column");
    };
    assert_eq!(column.frame.scroll, crate::Scroll::Both);
    let float = row.frame.float.unwrap();
    assert_eq!(
        (float.top, float.right, float.left),
        (Some(-4.0), Some(-4.0), None)
    );
}

#[test]
fn a_broken_child_is_marked_in_place_and_the_rest_still_draws() {
    let source = r#"return mod { view = function()
        return ui.row { ui.text "before", ui.icon "newspaper", ui.text "after" }
    end }"#;
    let dir = app(&[("news", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = broken(&workspace, "news");
    // A short message with the closest names, not every icon.
    assert!(
        error.contains("row[2].icon.name: no icon named `newspaper`"),
        "{error}"
    );
    assert!(error.contains("IconName"), "{error}");
    assert!(error.len() < 300, "{error}");
    let Some(Node::Stack(row)) = workspace.slot("news").unwrap().view() else {
        panic!("the row still draws");
    };
    assert!(matches!(&row.children[..], [
        Node::Text(_),
        Node::Problem(_),
        Node::Text(_)
    ]));
    // It still counts as a problem for `just check`.
    assert!(workspace.problems().iter().any(|(name, _)| *name == "news"));
    // A near miss gets its suggestion.
    let typo = "return mod { view = function() return ui.row { ui.icon 'cpus' } end }";
    let dir = app(&[("typo", typo)]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        broken(&workspace, "typo").contains("close ones: cpu"),
        "{}",
        broken(&workspace, "typo")
    );
}

#[test]
fn siblings_cannot_share_a_key() {
    let dir = app(&[(
        "keys",
        r#"return mod { view = function()
          return ui.row { ui.column { key = "a" }, ui.column { key = "b" }, ui.column { key = "a" } }
        end }"#,
    )]);
    let workspace = Workspace::open(dir.path(), false);
    let error = broken(&workspace, "keys");
    assert!(error.contains("`a`"), "{error}");
}
