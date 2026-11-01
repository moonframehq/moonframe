use super::*;

#[test]
fn bad_values_are_refused() {
    for (view, expected) in [
        (
            "ui.hsplit { ui.text 'x' }",
            "children of a split are ui.pane",
        ),
        ("ui.pane {}", "a pane goes inside"),
        ("ui.text { 'x', tone = 'loud' }", "unknown `loud`"),
        ("{ kind = 'row' }", "without `type`"),
        ("ui.button { 'x', msg = 'a', command = 'quit' }", "not both"),
        (
            "ui.button { 'x', command = 'explode' }",
            "unknown `explode`",
        ),
        ("42", "expected an element"),
        ("ui.icon 'cpus'", "no icon named `cpus`"),
    ] {
        let source = format!("return mod {{ view = function() return {view} end }}");
        let dir = app(&[("bad", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace
            .slot("bad")
            .unwrap()
            .error()
            .map(ToString::to_string);
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{view}: {error:?}"
        );
    }
}

#[test]
fn endless_loops_are_stopped() {
    let source = "return mod { view = function() while true do end end }";
    let dir = app(&[("spin", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.slot("spin").unwrap().error().unwrap().to_string();
    assert!(error.contains("ran out of instructions"), "{error}");
}

#[test]
fn the_sandbox_has_no_escape_hatches() {
    for global in [
        "os",
        "io",
        "debug",
        "require",
        "loadstring",
        "print",
        "getfenv",
    ] {
        let source = format!(
            "return mod {{ view = function() return ui.text(tostring({global} == nil)) end }}"
        );
        let dir = app(&[("probe", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let Some(Node::Text(text)) = workspace.slot("probe").unwrap().view() else {
            panic!("{global}: no text");
        };
        assert_eq!(text.text, "true", "{global} is reachable");
    }
    // The shared library cannot be changed.
    let source = "ui.text = nil\nreturn mod { view = function() return 'x' end }";
    let dir = app(&[("patch", source)]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(workspace.slot("patch").unwrap().error().is_some());
}

#[test]
fn the_type_definitions_cover_what_the_host_accepts() {
    use crate::{
        Align, Anchor, BrowserCommand, Command, Direction, Element, Justify, Placement, Side,
        Space, Surface, TextSize, Tone, TransitionKind, Variant, Weight,
    };
    let definitions = include_str!("../../../../types/moonframe.d.luau");
    let missing: Vec<String> = Element::NAMES
        .iter()
        // `use` and `ui.mount { id = ... }` build instances; no mod calls it.
        .filter(|name| **name != "instance")
        .filter(|name| !definitions.contains(&format!("\t{name}: (")))
        .map(|name| format!("ui.{name}"))
        .chain(
            [
                Tone::NAMES,
                Space::NAMES,
                TextSize::NAMES,
                Weight::NAMES,
                Align::NAMES,
                Justify::NAMES,
                Surface::NAMES,
                Direction::NAMES,
                Variant::NAMES,
                Command::NAMES,
                BrowserCommand::NAMES,
                TransitionKind::NAMES,
                Border::NAMES,
                Side::NAMES,
                Anchor::NAMES,
                Placement::NAMES,
                crate::Fit::NAMES,
                crate::Titlebar::NAMES,
                crate::Platform::NAMES,
            ]
            .concat()
            .into_iter()
            .filter(|name| !definitions.contains(&format!("\"{name}\"")))
            .map(|name| format!("\"{name}\"")),
        )
        .collect();
    assert!(
        missing.is_empty(),
        "types/moonframe.d.luau is missing {missing:?}"
    );
}

#[test]
fn long_strings_are_refused() {
    let long = "x".repeat(20_000);
    let source = format!("return mod {{ view = function() return ui.row {{ '{long}' }} end }}");
    let dir = app(&[("long", &source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = broken(&workspace, "long");
    assert!(error.contains("longer than"), "{error}");

    let lua = mlua::Lua::new();
    // A document fits in a message; past one, it doesn't.
    let doc = mlua::Value::String(lua.create_string("y".repeat(100_000)).unwrap());
    assert!(Data::read(&doc).is_ok());
    let huge = mlua::Value::String(
        lua.create_string("y".repeat(crate::MAX_DOCUMENT + 1))
            .unwrap(),
    );
    assert!(Data::read(&huge).is_err());
    let key = lua.create_table().unwrap();
    key.raw_set("k".repeat(crate::MAX_DOCUMENT + 1), 1).unwrap();
    assert!(Data::read(&mlua::Value::Table(key)).is_err());
    // Nor do many documents in one value.
    let many = lua.create_table().unwrap();
    for _ in 0..5 {
        many.raw_push("z".repeat(crate::MAX_DOCUMENT)).unwrap();
    }
    let error = Data::read(&mlua::Value::Table(many))
        .unwrap_err()
        .to_string();
    assert!(error.contains("bytes of text"), "{error}");
}

#[test]
fn log_lines_cannot_drive_the_terminal() {
    assert_eq!(
        crate::vm::printable("ok\x1b[2Jcleared\ttab"),
        "ok?[2Jcleared\ttab"
    );
    assert_eq!(crate::vm::printable(&"z".repeat(10_000)).len(), 4096);
}

/// Runs `test` with the stack the app reads views on: the main thread's
/// 8 MiB, not a test thread's 2, which a debug build's frames can outgrow
/// well inside the depth limit.
fn on_a_main_thread_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn floating_content_counts_toward_how_deep_a_view_goes() {
    on_a_main_thread_stack(|| {
        // A popover holding itself, and a long chain of overlays, used to
        // recurse until the stack overflowed and the app aborted.
        let deep = |view: &str| {
            let error =
                view_of(&format!("return mod {{ view = function() {view} end }}")).unwrap_err();
            assert!(error.contains("nested deeper"), "{error}");
        };
        deep(r#"local o = ui.popover { ui.text "a" }; o.content = o; return o"#);
        deep(
            r#"local n = ui.text "x"; for _ = 1, 9000 do n = ui.overlay { content = n } end; return n"#,
        );
        deep(r#"local n = ui.text "x"; for _ = 1, 60 do n = ui.column { n } end; return n"#);
    });
}

#[test]
fn sends_to_providers_and_topics_count_toward_the_limit() {
    // Each update sends a pile to a provider and a topic, then itself again.
    let flood = r#"return mod {
      update = function(state, msg)
        local sends = {}
        for i = 1, 100 do
          table.insert(sends, { to = "provider:git", msg = i })
          table.insert(sends, { to = "topic:flood", msg = i })
        end
        table.insert(sends, { to = "flood", msg = "again" })
        return state, sends
      end,
      view = function() return ui.text "flood" end,
    }"#;
    let wide = r#"return mod {
      update = function(state, msg)
        local sends = {}
        for i = 1, 50000 do table.insert(sends, { to = "provider:git", msg = i }) end
        return state, sends
      end,
      view = function() return ui.text "wide" end,
    }"#;
    let dir = app(&[("flood", flood), ("wide", wide)]);
    let mut workspace = Workspace::open(dir.path(), false);

    workspace.send("flood", "flood", Data::from("go"));
    let sent = workspace.take_outbox().len();
    assert!(sent < 1024, "{sent} provider messages from one action");
    let error = workspace
        .slot("flood")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("1024"), "{error}");

    workspace.send("wide", "wide", Data::from("go"));
    assert!(workspace.take_outbox().is_empty());
    let error = workspace.slot("wide").unwrap().error().unwrap().to_string();
    assert!(error.contains("may send up to"), "{error}");
}

#[test]
fn numbers_too_big_for_the_app_are_refused() {
    let dir = app(&[]);
    write(
        dir.path(),
        "style.luau",
        r##"return style { rules = { panel = { gradient = { from = "#000", to = "#fff", angle = 1e300 } } } }"##,
    );
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.style_error().unwrap().to_string();
    assert!(error.contains("finite"), "{error}");
}
