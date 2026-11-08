//! Regression coverage using the shipped components and demo update code.

use super::*;

fn fixture(source: &str, components: &[&str]) -> (tempfile::TempDir, Workspace) {
    let dir = app(&[("tasks", source)]);
    let builtins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/components");
    for name in components {
        write(
            dir.path(),
            &format!("components/{name}.luau"),
            &fs::read_to_string(builtins.join(format!("{name}.luau"))).unwrap(),
        );
    }
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    (dir, workspace)
}

fn demo(name: &str, components: &[&str]) -> (tempfile::TempDir, Workspace) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../app/components/demos/{name}.luau"));
    fixture(&fs::read_to_string(path).unwrap(), components)
}

fn tree(workspace: &Workspace) -> &Node {
    let slot = workspace.slot("tasks").unwrap();
    assert!(slot.error().is_none(), "{:?}", slot.error());
    assert!(slot.broken().is_empty(), "{:?}", slot.broken());
    slot.view().unwrap()
}

fn walk<'a>(node: &'a Node, path: &mut Vec<usize>, visit: &mut impl FnMut(&'a Node, &[usize])) {
    visit(node, path);
    let children: Vec<&Node> = match node {
        Node::Stack(stack) => stack.children.iter().collect(),
        Node::List(list) => list.rows.iter().collect(),
        Node::Popover(popover) => popover
            .anchor
            .iter()
            .chain(std::iter::once(popover.content.as_ref()))
            .collect(),
        _ => vec![],
    };
    for (index, child) in children.into_iter().enumerate() {
        path.push(index);
        walk(child, path, visit);
        path.pop();
    }
}

fn map(fields: &[(&str, Data)]) -> Data {
    Data::Map(
        fields
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

fn send(workspace: &mut Workspace, fields: &[(&str, Data)]) {
    workspace.send("tasks", "tasks", map(fields));
}

fn list(workspace: &Workspace) -> crate::List {
    let mut found = None;
    walk(tree(workspace), &mut vec![], &mut |node, _| {
        if let Node::List(list) = node {
            found = Some(list.clone());
        }
    });
    found.expect("grouped table should use a virtual list")
}

#[test]
fn grouped_tables_virtualize_rows_and_fold_without_exceeding_the_element_budget() {
    let (_dir, mut workspace) = fixture(
        r#"
local table_view = use "table"
return mod {
  init = function() return { folded = false } end,
  update = function(state) state.folded = not state.folded end,
  view = function(state)
    local rows = {}
    for i = 1, 1000 do rows[i] = { id = tostring(i), a = "a", b = "b", c = "c", d = "d", e = "e" } end
    return table_view { id = "tasks", height = 300,
      columns = { { key = "a" }, { key = "b" }, { key = "c" }, { key = "d" }, { key = "e" } },
      groups = { { id = "todo", title = "To Do", rows = rows, collapsed = state.folded, add = "add" } },
    }
  end,
}"#,
        &["table"],
    );
    let first = list(&workspace);
    assert_eq!(first.count, 1000 + 2); // tasks, heading and Add
    assert!(first.rows.len() < 1000);
    workspace.set_list_window(
        "tasks",
        "tasks",
        crate::ListWindow {
            start: 990,
            len: 30,
        },
    );
    let last = list(&workspace);
    assert_eq!(last.start, 990);
    assert_eq!(last.rows.len(), last.count - last.start);
    workspace.send("tasks", "tasks", Data::from("fold"));
    let mut lists = 0;
    walk(tree(&workspace), &mut vec![], &mut |node, _| {
        if matches!(node, Node::List(_)) {
            lists += 1;
        }
    });
    assert_eq!(lists, 0);
}

#[test]
fn grouped_select_all_reflects_full_and_partial_selection_including_folded_rows() {
    let (_dir, mut workspace) = fixture(
        r#"
local table_view = use "table"
return mod {
  init = function() return { checks = { a = true, b = true } } end,
  update = function(state) state.checks.b = nil end,
  view = function(state)
    return table_view { checkable = true, checks = state.checks, columns = { { key = "title" } },
      groups = {
        { id = "todo", rows = { { id = "a", title = "A" } } },
        { id = "done", collapsed = true, rows = { { id = "b", title = "B" } } },
      },
    }
  end,
}"#,
        &["table"],
    );
    let header = |workspace: &Workspace| {
        let mut result = None;
        walk(tree(workspace), &mut vec![], &mut |node, _| {
            if let Node::Stack(stack) = node
                && stack.look.class.iter().any(|class| class == "table-check")
                && result.is_none()
            {
                assert!(stack.selected);
                let Node::Icon(icon) = &stack.children[0] else {
                    panic!("missing check state");
                };
                result = Some(icon.name.name().to_owned());
            }
        });
        result.unwrap()
    };
    assert_eq!(header(&workspace), "check");
    workspace.send("tasks", "tasks", Data::from("uncheck"));
    assert_eq!(header(&workspace), "minus");
}

#[test]
fn gantt_resize_handles_keep_their_paths_across_the_label_threshold() {
    let (_dir, mut workspace) = demo("gantt", &["gantt", "avatar"]);
    let handle = |workspace: &Workspace| {
        let mut found = None;
        walk(tree(workspace), &mut vec![], &mut |node, path| {
            if let Node::Stack(stack) = node
                && stack
                    .pointer
                    .as_ref()
                    .and_then(|pointer| pointer.press.as_ref())
                    == Some(&map(&[("resize", Data::from("brief"))]))
            {
                found = Some(path.to_vec());
            }
        });
        found.unwrap()
    };
    let before = handle(&workspace);
    for days in [-1, -2, 1] {
        send(
            &mut workspace,
            &[
                ("resize", Data::from("brief")),
                ("phase", Data::from("move")),
                ("value", Data::Integer(days)),
            ],
        );
        assert_eq!(handle(&workspace), before);
    }
}

fn carried_rows(workspace: &Workspace, field: &str, group: Option<&str>) -> Vec<String> {
    let mut ids = vec![];
    walk(tree(workspace), &mut vec![], &mut |node, _| {
        if let Node::Stack(stack) = node
            && let Some(Data::Map(carry)) = stack
                .pointer
                .as_ref()
                .and_then(|pointer| pointer.carry.as_ref())
            && group.is_none_or(|group| carry.get("group") == Some(&Data::from(group)))
            && let Some(Data::String(id)) = carry.get(field)
        {
            ids.push(id.clone());
        }
    });
    ids
}

#[test]
fn table_demo_dropping_after_the_predecessor_is_a_no_op() {
    let (_dir, mut workspace) = demo("table", &["table", "switch"]);
    send(
        &mut workspace,
        &[
            ("move", Data::Bool(true)),
            ("group", Data::from("todo")),
            ("phase", Data::from("drop")),
            ("carry", map(&[("row", Data::from("web112"))])),
        ],
    );
    let before = carried_rows(&workspace, "row", Some("todo"));
    assert_eq!(before, ["web121", "web124", "web112"]);
    send(
        &mut workspace,
        &[
            ("move", Data::from("web121")),
            ("group", Data::from("todo")),
            ("phase", Data::from("drop")),
            ("y", Data::Number(0.75)),
            ("carry", map(&[("row", Data::from("web124"))])),
        ],
    );
    assert_eq!(carried_rows(&workspace, "row", Some("todo")), before);
}

#[test]
fn kanban_demo_drops_on_empty_column_space_append_after_hovering_a_card() {
    let (_dir, mut workspace) = demo("kanban-column", &["kanban-column", "board-card", "avatar"]);
    send(
        &mut workspace,
        &[
            ("over_card", Data::from("web121")),
            ("phase", Data::from("over")),
            ("y", Data::Number(0.25)),
        ],
    );
    send(
        &mut workspace,
        &[
            ("over_column", Data::from("todo")),
            ("phase", Data::from("over")),
        ],
    );
    send(
        &mut workspace,
        &[
            ("over_card", Data::from("web121")),
            ("phase", Data::from("leave")),
        ],
    );
    send(
        &mut workspace,
        &[
            ("over_column", Data::from("todo")),
            ("phase", Data::from("drop")),
            ("carry", map(&[("card", Data::from("web112"))])),
        ],
    );
    let ids = carried_rows(&workspace, "card", None);
    assert_eq!(&ids[..4], ["web121", "web124", "web130", "web112"]);
}

#[test]
fn mention_fields_track_selection_and_guard_the_completion() {
    let (_dir, workspace) = fixture(
        r#"
local mention = use "mention-field"
return mod { view = function()
  return mention { id = "comment", value = "hi @sa", caret = 6,
    change = "type", people = { { name = "Sarah Chen" } } }
end }
"#,
        &["mention-field", "avatar"],
    );
    let mut input = None;
    walk(tree(&workspace), &mut vec![], &mut |node, _| {
        if let Node::Input(field) = node {
            input = Some(field);
        }
    });
    let input = input.unwrap();
    assert_eq!(input.select, input.change);
    let (_, Data::Map(choose)) = input
        .keys
        .iter()
        .find(|(key, _)| key.key == "enter")
        .unwrap()
    else {
        panic!("no completion");
    };
    let crate::Request::Edit(edit) = crate::Request::parse("edit", &choose["edit"]).unwrap() else {
        panic!("not an edit");
    };
    assert!(edit.applies("hi @sa", 6..6));
    assert!(!edit.applies("hi @sa", 5..5));
}

#[test]
fn kanban_hover_preserves_card_paths_and_in_flow_layout() {
    let (_dir, mut workspace) = demo("kanban-column", &["kanban-column", "board-card", "avatar"]);
    let cards = |workspace: &Workspace| {
        let mut cards = vec![];
        walk(tree(workspace), &mut vec![], &mut |node, path| {
            if let Node::Stack(stack) = node
                && stack.pointer.as_ref().is_some_and(|p| p.carry.is_some())
            {
                cards.push((path.to_vec(), stack.clone()));
            }
        });
        cards
    };
    let before = cards(&workspace);
    for (target, y) in [("web121", 0.01), ("web124", 0.75), ("web130", 0.99)] {
        send(
            &mut workspace,
            &[
                ("over_card", Data::from(target)),
                ("phase", Data::from("over")),
                ("y", Data::Number(y)),
            ],
        );
        assert_eq!(cards(&workspace), before);
        let mut markers = 0;
        walk(tree(&workspace), &mut vec![], &mut |node, _| {
            if let Node::Stack(stack) = node
                && stack
                    .look
                    .class
                    .iter()
                    .any(|class| class == "kanban-column-marker")
            {
                assert!(
                    stack.frame.float.is_some(),
                    "a marker must not shift the drop target"
                );
                assert!(stack.pointer.is_none(), "the card keeps its drop target");
                markers += 1;
            }
        });
        assert_eq!(markers, 1);
    }
    send(
        &mut workspace,
        &[
            ("over_card", Data::from("web121")),
            ("phase", Data::from("drop")),
            ("y", Data::Number(0.01)),
            ("carry", map(&[("card", Data::from("web112"))])),
        ],
    );
    assert_eq!(
        &carried_rows(&workspace, "card", None)[..4],
        ["web112", "web121", "web124", "web130"]
    );
}

#[test]
fn leaving_an_old_calendar_or_table_target_keeps_the_new_indicator() {
    for (name, components, old, new) in [
        (
            "week-calendar",
            vec!["week-calendar", "segmented", "month-grid"],
            vec![("move", Data::from("2026-11-03"))],
            vec![("move", Data::from("2026-11-02"))],
        ),
        (
            "table",
            vec!["table", "switch"],
            vec![
                ("move", Data::from("web124")),
                ("group", Data::from("todo")),
                ("y", Data::Number(0.2)),
            ],
            vec![
                ("move", Data::from("web121")),
                ("group", Data::from("todo")),
                ("y", Data::Number(0.2)),
            ],
        ),
        (
            "table",
            vec!["table", "switch"],
            vec![
                ("move", Data::Bool(true)),
                ("group", Data::from("progress")),
            ],
            vec![("move", Data::Bool(true)), ("group", Data::from("todo"))],
        ),
    ] {
        let (_dir, mut workspace) = demo(name, &components);
        let mut event = |fields: &[(&str, Data)], phase: &str| {
            let mut fields = fields.to_vec();
            fields.push(("phase", Data::from(phase)));
            send(&mut workspace, &fields);
            tree(&workspace).clone()
        };
        event(&old, "over");
        let entered = event(&new, "over");
        assert_eq!(event(&old, "leave"), entered, "{name}: stale leave");
        assert_ne!(event(&new, "leave"), entered, "{name}: current leave");
    }
}

#[test]
fn long_editor_text_remains_available_in_a_scrollable_preview() {
    let (_dir, mut workspace) = demo("text-editor", &["text-editor", "segmented"]);
    let text = "A paragraph of the description.\n\n".repeat(100);
    send(
        &mut workspace,
        &[
            ("msg", Data::from("type")),
            ("value", Data::from(text.as_str())),
        ],
    );
    send(&mut workspace, &[("view", Data::from("preview"))]);
    let mut preview = None;
    walk(tree(&workspace), &mut vec![], &mut |node, _| {
        if let Node::Markdown(markdown) = node {
            preview = Some(markdown);
        }
    });
    let preview = preview.unwrap();
    assert_eq!(preview.text, text);
    assert!(preview.scroll && preview.grow);
}

#[test]
fn picker_navigation_reveals_the_actual_child_including_disabled_rows_and_create() {
    let (_dir, mut workspace) = fixture(
        r#"
local picker = use "picker"
return mod {
  init = function() return { highlighted = nil, query = "Option" } end,
  update = function(state, msg)
    if msg.highlight then state.highlighted = msg.highlight end
    if msg.query then state.query = msg.query end
  end,
  view = function(state)
    local options = {}
    for i = 1, 100 do
      options[i] = { id = tostring(i), label = "Option " .. i, disabled = i % 3 == 0 }
    end
    return picker { open = true, options = options, query = state.query,
      highlighted = state.highlighted, create = "create" }
  end,
}
"#,
        &["picker", "menu", "avatar", "pill", "kbd"],
    );
    let inspect = |workspace: &Workspace, key: &str| {
        let Node::Popover(popover) = tree(workspace) else {
            panic!("no picker")
        };
        let mut target = None;
        walk(&popover.content, &mut vec![], &mut |node, _| {
            if let Node::Stack(stack) = node
                && let Some(index) = stack.scroll_to
            {
                assert_eq!(stack.frame.max_height, Some(300.));
                assert_ne!(stack.frame.scroll, crate::Scroll::No);
                let Node::Stack(row) = &stack.children[index - 1] else {
                    panic!("not a row")
                };
                assert!(row.selected);
                assert!(!row.frame.grow && !row.frame.shrink);
                target = row.action.clone();
            }
        });
        let enter = &popover
            .keys
            .0
            .iter()
            .find(|(k, _)| k.key == "enter")
            .unwrap()
            .1;
        assert_eq!(
            target,
            Some(Action::Send {
                to: None,
                msg: enter.clone()
            })
        );
        popover
            .keys
            .0
            .iter()
            .find(|(k, _)| k.key == key)
            .unwrap()
            .1
            .clone()
    };
    // Past the viewport, all the way to Create, then back up through it.
    for key in ["down", "up"] {
        for _ in 0..100 {
            let msg = inspect(&workspace, key);
            workspace.send("tasks", "tasks", msg);
        }
    }
    send(&mut workspace, &[("query", Data::from("Option 98"))]);
    inspect(&workspace, "down"); // filtered-out highlight falls back to the match
}
