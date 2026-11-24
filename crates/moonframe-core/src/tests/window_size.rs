use {super::*, crate::WindowState};

#[test]
fn a_view_reads_the_windows_size_and_follows_it() {
    let source = r#"return mod { view = function(_, app)
        local w = app.window
        local layout = if w.width and w.width < 768 then "narrow" else "wide"
        return ui.text(`{w.width} {w.height} {layout}`)
    end }"#;
    let dir = app(&[]);
    write(dir.path(), "main.luau", source);
    let mut workspace = Workspace::open(dir.path(), false);
    let text = |workspace: &Workspace| match workspace.slot(SHELL).unwrap().view() {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("expected text, got {other:?}"),
    };
    // Unknown until the renderer says.
    assert_eq!(text(&workspace), "nil nil wide");
    workspace.set_window(WindowState {
        size: Some((1200, 800)),
        ..WindowState::default()
    });
    assert_eq!(text(&workspace), "1200 800 wide");
    workspace.set_window(WindowState {
        size: Some((600, 800)),
        ..WindowState::default()
    });
    assert_eq!(text(&workspace), "600 800 narrow");
}

#[test]
fn a_mod_a_resize_mounts_starts_in_step() {
    let publisher = r#"return mod {
      update = function(state, msg) return state, { { to = "topic:tabs/main", msg = msg } } end,
      view = function() return ui.text "publisher" end,
    }"#;
    let watcher = r#"return mod {
      subscribe = { "tabs/main" },
      init = function() return { heard = "nothing" } end,
      update = function(state, msg)
        if type(msg) == "table" and msg.topic then state.heard = msg.topic .. "=" .. tostring(msg.value) end
      end,
      view = function(state) return ui.text(state.heard) end,
    }"#;
    let dir = app(&[("publisher", publisher), ("watcher", watcher)]);
    // Only a narrow window shows the watcher.
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function(_, app)
            local narrow = app.window.width ~= nil and app.window.width < 768
            return ui.column { narrow and ui.mount { "watcher", id = "drawer" } }
        end }"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    workspace.send("publisher", "publisher", Data::from("early"));
    workspace.set_window(WindowState {
        size: Some((600, 800)),
        ..WindowState::default()
    });
    assert_eq!(
        instance_text(&workspace, "watcher#drawer"),
        "tabs/main=early"
    );
}
