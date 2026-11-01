use super::*;

#[test]
fn a_publish_redraws_only_the_views_that_read_it() {
    // Each view counts how often it ran, in its own state.
    let counting = |key: &str| {
        format!(
            r#"return mod {{
              init = function() return {{ runs = 0 }} end,
              update = function(state) end,
              view = function(state, app)
                state.runs += 1
                local seen = app.{key}
                return ui.text(`{{state.runs}} {{if seen then seen.n else "-"}}`)
              end,
            }}"#
        )
    };
    let dir = app(&[
        ("cpu", &counting("system")),
        ("files", &counting("projects")),
    ]);
    let mut workspace = Workspace::open(dir.path(), false);
    let text = |workspace: &Workspace, name: &str| match workspace.slot(name).unwrap().view() {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("expected text, got {other:?}"),
    };
    let number = |n: f64| Data::Map([("n".to_owned(), Data::Number(n))].into());
    let before = text(&workspace, "files");

    workspace.set_host("system", number(1.0));
    workspace.set_host("system", number(2.0));
    assert!(
        text(&workspace, "cpu").ends_with(" 2"),
        "{}",
        text(&workspace, "cpu")
    );
    // `files` never read `app.system`, so it did not run again.
    assert_eq!(text(&workspace, "files"), before);

    workspace.set_host("projects", number(7.0));
    assert!(text(&workspace, "files").ends_with(" 7"));

    // A message redraws its mod only, with `app` as it was.
    let cpu = text(&workspace, "cpu");
    workspace.send("files", "files", Data::Nil);
    assert_eq!(text(&workspace, "cpu"), cpu);
    assert!(text(&workspace, "files").ends_with(" 7"));
}

fn text_of(node: &Node) -> &str {
    match node {
        Node::Text(text) => &text.text,
        other => panic!("not a text: {other:?}"),
    }
}

/// The text of the first child of mod `name`'s column.
fn first_text(workspace: &Workspace, name: &str) -> String {
    let Some(Node::Stack(column)) = workspace.slot(name).unwrap().view() else {
        panic!("{name} drew no column");
    };
    let Node::Text(text) = &column.children[0] else {
        panic!("no text");
    };
    text.text.clone()
}

#[test]
fn a_view_runs_again_only_when_what_it_read_changes() {
    // `calls` counts the view's runs; it lives as long as the mod's state.
    let source = r#"
local calls = 0
return mod {
  init = function() return { n = 0 } end,
  update = function(state, msg) state.n += 1 end,
  view = function(state, app)
    calls += 1
    local branch = app.git and app.git.branch or "-"
    return ui.column { ui.text(`{calls} {branch} {state.n}`) }
  end,
}"#;
    let dir = app(&[("watch", source)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let start = first_text(&workspace, "watch");
    assert!(start.ends_with(" - 0"), "{start}");
    let calls = |workspace: &Workspace| -> u32 {
        first_text(workspace, "watch")
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let before = calls(&workspace);
    // A key it never read: the view keeps what it drew.
    workspace.set_host(
        "system",
        Data::Map([("cpu".to_owned(), Data::Number(0.5))].into()),
    );
    workspace.set_dark(true);
    assert_eq!(calls(&workspace), before);
    // A key it read, even one that was missing: it runs again.
    workspace.set_host(
        "git",
        Data::Map([("branch".to_owned(), Data::from("main"))].into()),
    );
    assert_eq!(calls(&workspace), before + 1);
    assert!(first_text(&workspace, "watch").ends_with(" main 0"));
    // The same value again changes nothing.
    workspace.set_host(
        "git",
        Data::Map([("branch".to_owned(), Data::from("main"))].into()),
    );
    assert_eq!(calls(&workspace), before + 1);
    // A message to it: its state may have changed.
    workspace.send("watch", "watch", Data::from("bump"));
    assert_eq!(calls(&workspace), before + 2);
    assert!(first_text(&workspace, "watch").ends_with(" main 1"));
    // A reload runs the new source, which starts counting again.
    workspace.reload();
    assert_eq!(calls(&workspace), 1);
}

#[test]
fn a_view_that_goes_through_all_of_app_runs_on_any_change() {
    let source = r#"
local calls = 0
return mod {
  view = function(state, app)
    calls += 1
    local keys = 0
    for _ in app do keys += 1 end
    return ui.column { ui.text(`{calls} {keys}`) }
  end,
}"#;
    let dir = app(&[("all", source)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let before = first_text(&workspace, "all");
    workspace.set_host(
        "system",
        Data::Map([("cpu".to_owned(), Data::Number(0.5))].into()),
    );
    let after = first_text(&workspace, "all");
    assert_ne!(before, after);
    // It saw the new key among the rest.
    let count = |text: &str| -> usize { text.split(' ').nth(1).unwrap().parse().unwrap() };
    assert_eq!(count(&after), count(&before) + 1);
}

#[test]
fn a_list_builds_only_the_rows_it_is_asked_for() {
    let source = r#"
return mod {
  view = function()
    return ui.column {
      ui.list { id = "rows", count = 100000, row_height = 20,
        row = function(i) return ui.text(`row {i}`) end },
    }
  end,
}"#;
    let dir = app(&[("long", source)]);
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let list = |workspace: &Workspace| -> crate::List {
        let Some(Node::Stack(column)) = workspace.slot("long").unwrap().view() else {
            panic!("no column");
        };
        let Node::List(list) = &column.children[0] else {
            panic!("no list");
        };
        list.clone()
    };
    let first = list(&workspace);
    assert_eq!(
        (first.count, first.row_height, first.start),
        (100_000, 20.0, 0)
    );
    assert_eq!(first.rows.len(), crate::ListWindow::FIRST.len);
    assert_eq!(text_of(&first.rows[0]), "row 1");
    let window = crate::ListWindow {
        start: 5000,
        len: 30,
    };
    assert!(workspace.set_list_window("long", "rows", window));
    assert!(!workspace.set_list_window("long", "rows", window));
    let scrolled = list(&workspace);
    assert_eq!((scrolled.start, scrolled.rows.len()), (5000, 30));
    assert_eq!(text_of(&scrolled.rows[0]), "row 5001");
    // Past the end, it stops at the last row.
    workspace.set_list_window("long", "rows", crate::ListWindow {
        start: 99_990,
        len: 30,
    });
    assert_eq!(list(&workspace).rows.len(), 10);
}

#[test]
fn lists_are_checked() {
    for (list, expected) in [
        (
            "ui.list { count = 3, row_height = 20, row = function(i) return 'x' end }, ui.list { count = 1, row_height = 20, row = function() return 'y' end }",
            "two lists with the id `list`",
        ),
        (
            "ui.list { count = -1, row_height = 20, row = function() end }",
            "expected 0 to",
        ),
        (
            "ui.list { count = 3, row = function() return 'x' end }",
            "row_height: missing",
        ),
        ("ui.list { count = 3, row_height = 20 }", "row: missing"),
        (
            "ui.list { count = 3, row_height = 20, row = function(i) error('no row ' .. i) end }",
            "no row 1",
        ),
    ] {
        let source =
            format!("return mod {{ view = function() return ui.column {{ {list} }} end }}");
        let dir = app(&[("bad", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let slot = workspace.slot("bad").unwrap();
        let problem = slot
            .error()
            .map(ToString::to_string)
            .or_else(|| slot.broken().first().map(ToString::to_string))
            .unwrap_or_default();
        assert!(problem.contains(expected), "{list}: {problem}");
    }
}

#[test]
fn memo_keeps_what_it_built_while_its_dependencies_stay_the_same() {
    let source = r#"
local builds = 0
return mod {
  init = function() return { a = 1, b = 1 } end,
  update = function(state, msg) state[msg] += 1 end,
  view = function(state)
    local part = ui.memo("part", { state.a }, function()
      builds += 1
      return ui.text(`a {state.a}`)
    end)
    return ui.column { part, ui.text(`{builds}`) }
  end,
}"#;
    let dir = app(&[("memo", source)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let shown = |workspace: &Workspace| -> (String, String) {
        let Some(Node::Stack(column)) = workspace.slot("memo").unwrap().view() else {
            panic!("no column");
        };
        let text = |node: &Node| match node {
            Node::Text(text) => text.text.clone(),
            other => panic!("{other:?}"),
        };
        (text(&column.children[0]), text(&column.children[1]))
    };
    assert_eq!(shown(&workspace), ("a 1".to_owned(), "1".to_owned()));
    workspace.send("memo", "memo", Data::from("b"));
    assert_eq!(shown(&workspace), ("a 1".to_owned(), "1".to_owned()));
    workspace.send("memo", "memo", Data::from("a"));
    assert_eq!(shown(&workspace), ("a 2".to_owned(), "2".to_owned()));
}

#[test]
fn a_lazy_pane_starts_what_it_mounts_when_it_shows_and_keeps_it_hidden() {
    let host = r#"
return mod {
  init = function() return { open = false } end,
  update = function(state) state.open = not state.open end,
  view = function(state)
    return ui.hsplit {
      ui.pane { ui.text "main" },
      ui.pane { lazy = true, visible = state.open, ui.mount { "counter", id = "side" } },
    }
  end,
}"#;
    let dir = app(&[("host", host), ("counter", COUNTER)]);
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert!(
        workspace.slot("counter#side").is_none(),
        "started before it showed"
    );
    workspace.send("host", "host", Data::from("toggle"));
    assert!(
        workspace.slot("counter#side").is_some(),
        "not started when shown"
    );
    // Its state moves on while it shows...
    workspace.send("host", "counter#side", Data::from("add"));
    workspace.send("host", "host", Data::from("toggle"));
    // ...and it sleeps, kept, once hidden.
    let slot = workspace.slot("counter#side").expect("dropped when hidden");
    let Some(Node::Panel(panel)) = slot.view() else {
        panic!("no panel");
    };
    assert_eq!(text_of(&panel.children[0]), "count 1");
}
