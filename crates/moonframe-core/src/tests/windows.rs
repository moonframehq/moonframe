use {
    super::*,
    crate::{AppLayer, Platform, Roots, Titlebar, WindowState},
};

const REPORT: &str = r#"return mod {
    init = function(previous) return previous or { n = 0 } end,
    update = function(state) state.n += 1 end,
    view = function(state, app)
        local w = app.window
        return ui.text(`{w.titlebar} {w.maximized} {w.fullscreen} {w.active} {state.n}`)
    end,
}"#;

fn text(workspace: &Workspace, name: &str) -> String {
    let slot = workspace.slot(name).unwrap();
    assert!(slot.error().is_none(), "{name}: {:?}", slot.error());
    match slot.view() {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("{name}: expected text, got {other:?}"),
    }
}

fn commands(node: &Node, found: &mut Vec<Action>) {
    match node {
        Node::Stack(row) => {
            found.extend(row.action.clone());
            for child in &row.children {
                commands(child, found);
            }
        },
        Node::Button(button) => found.extend(button.action.clone()),
        _ => {},
    }
}

#[test]
fn mod_windows_keep_their_own_state_through_updates_reload_and_close() {
    let dir = app(&[("first", REPORT), ("second", REPORT)]);
    write(dir.path(), "main.luau", REPORT);
    let mut workspace = Workspace::open(dir.path(), false);
    let main = WindowState::default();
    let first = WindowState {
        titlebar: Titlebar::Hidden,
        maximized: true,
        active: false,
        ..main
    };
    let second = WindowState {
        titlebar: Titlebar::Hidden,
        fullscreen: true,
        ..main
    };
    workspace.set_mod_window("first", Some(first));
    workspace.set_mod_window("second", Some(second));
    assert_eq!(text(&workspace, SHELL), "system false false true 0");
    assert_eq!(text(&workspace, "first"), "hidden true false false 0");
    assert_eq!(text(&workspace, "second"), "hidden false true true 0");

    // Ordinary messages and provider updates must keep the window context.
    workspace.send("first", "first", Data::Nil);
    workspace.set_host("example", Data::Bool(true));
    workspace.reload();
    assert_eq!(text(&workspace, "first"), "hidden true false false 1");
    assert_eq!(text(&workspace, "second"), "hidden false true true 0");

    // Main-window activation and fullscreen changes do not replace either.
    workspace.set_window(WindowState {
        fullscreen: true,
        active: false,
        ..main
    });
    assert_eq!(text(&workspace, SHELL), "system false true false 0");
    assert_eq!(text(&workspace, "first"), "hidden true false false 1");
    assert_eq!(text(&workspace, "second"), "hidden false true true 0");

    // A native state change reaches only that mod; closing restores the main state.
    workspace.set_mod_window(
        "first",
        Some(WindowState {
            active: true,
            ..first
        }),
    );
    assert_eq!(text(&workspace, "first"), "hidden true false true 1");
    workspace.set_mod_window("first", None);
    assert_eq!(text(&workspace, "first"), "system false true false 1");
    assert_eq!(text(&workspace, "second"), "hidden false true true 0");
    workspace.set_mod_window("first", Some(first));
    assert_eq!(text(&workspace, "first"), "hidden true false false 1");
}

#[test]
fn instances_inherit_their_mod_windows_state_and_only_readers_redraw() {
    let parent = r#"local nested = use "nested"
        return mod { view = function() return nested { id = "one" } end }"#;
    let dir = app(&[("first", parent), ("second", parent), ("report", REPORT)]);
    write(dir.path(), "main.luau", REPORT);
    write(
        dir.path(),
        "components/nested.luau",
        r#"return component {
        init = function() return {} end,
        update = function() end,
        build = function(props) return ui.mount { "report", id = props.id } end,
    }"#,
    );
    // Give the two component instances different child mod-instance ids.
    write(
        dir.path(),
        "mods/second/main.luau",
        &parent.replace("one", "two"),
    );
    write(
        dir.path(),
        "mods/static/main.luau",
        r#"local calls = 0
        return mod { view = function()
            calls += 1
            return ui.text(tostring(calls))
        end }"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    let hidden = WindowState {
        titlebar: Titlebar::Hidden,
        ..WindowState::default()
    };
    workspace.set_mod_window("first", Some(hidden));
    assert_eq!(text(&workspace, "report#one"), "hidden false false true 0");
    assert_eq!(text(&workspace, "report#two"), "system false false true 0");
    assert_eq!(text(&workspace, "report"), "system false false true 0");
    workspace.send("report#one", "report#one", Data::Nil);
    assert_eq!(text(&workspace, "report#one"), "hidden false false true 1");
    workspace.set_mod_window("first", None);
    assert_eq!(text(&workspace, "report#one"), "system false false true 1");

    workspace.set_mod_window("static", Some(hidden));
    workspace.set_mod_window(
        "static",
        Some(WindowState {
            maximized: true,
            ..hidden
        }),
    );
    workspace.set_mod_window("static", None);
    assert_eq!(text(&workspace, "static"), "1");
}

#[test]
fn a_mods_title_bar_uses_its_own_controls_and_traffic_light_inset() {
    let dir = app(&[(
        "tool",
        r#"local title_bar = use "title-bar"
        return mod {
            window = { titlebar = "hidden" },
            view = function(_, app) return title_bar { window = app.window, title = "Tool" } end,
        }"#,
    )]);
    write(dir.path(), "main.luau", REPORT);
    let roots = Roots::bundled(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app"),
        Vec::new(),
        AppLayer {
            path: dir.path().to_owned(),
            keep: Vec::new(),
        },
    );
    let mut workspace = Workspace::open(roots, false);
    let title_bar = |workspace: &Workspace| {
        let slot = workspace.slot("tool").unwrap();
        assert!(slot.error().is_none(), "{:?}", slot.error());
        let Some(Node::Stack(row)) = slot.view() else {
            panic!("no title bar")
        };
        row.clone()
    };
    let hidden = WindowState {
        platform: Platform::Windows,
        titlebar: Titlebar::Hidden,
        ..WindowState::default()
    };
    for platform in [Platform::Windows, Platform::Linux] {
        workspace.set_mod_window("tool", Some(WindowState { platform, ..hidden }));
        let row = title_bar(&workspace);
        let mut found = Vec::new();
        commands(&Node::Stack(row), &mut found);
        assert_eq!(
            found,
            [
                Action::Command(Command::Minimize),
                Action::Command(Command::Maximize),
                Action::Command(Command::CloseWindow),
            ]
        );
    }
    for (fullscreen, inset) in [(false, 76.), (true, 10.)] {
        workspace.set_mod_window(
            "tool",
            Some(WindowState {
                platform: Platform::Macos,
                fullscreen,
                ..hidden
            }),
        );
        assert_eq!(title_bar(&workspace).look.style.padding.left, Some(inset));
    }
    assert_eq!(text(&workspace, SHELL), "system false false true 0");
}
