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
