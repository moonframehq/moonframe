use {
    super::*,
    crate::{Action, Request, Workspace},
    std::{fs, path::Path},
};

const SHELL_SOURCE: &str = r#"
return mod {
  view = function(state, app)
    return ui.column { ui.mount "asker", ui.mount "settings" }
  end,
}
"#;

/// A mod that sends `msg` to `to` when it hears `{ go = { to, msg } }`.
const ASKER: &str = r#"
return mod {
  update = function(state, msg)
    if type(msg) == "table" and msg.go then
      return state, { { to = msg.go.to, msg = msg.go.msg } }
    end
  end,
  view = function(state)
    return ui.button { "Go", msg = "go" }
  end,
}
"#;

/// A mod that keeps the page it is sent, and opens in a window of a size.
const SETTINGS: &str = r#"
return mod {
  title = "Settings",
  window = { size = { width = 860, height = 600 }, min_size = { width = 640, height = 480 } },
  init = function() return { page = "general" } end,
  update = function(state, msg)
    if type(msg) == "table" and msg.page then state.page = msg.page end
  end,
  view = function(state) return ui.text(state.page) end,
}
"#;

fn write(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

fn workspace() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "main.luau", SHELL_SOURCE);
    write(dir.path(), "mods/asker/main.luau", ASKER);
    write(dir.path(), "mods/settings/main.luau", SETTINGS);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    (dir, workspace)
}

fn map(fields: &[(&str, Data)]) -> Data {
    Data::Map(
        fields
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

/// Has the mod `asker` send `msg` to `app:window` from its `update`.
fn ask(workspace: &mut Workspace, msg: Data) {
    let go = map(&[("go", map(&[("to", Data::from("app:window")), ("msg", msg)]))]);
    workspace.send("asker", "asker", go);
}

fn error(workspace: &Workspace) -> String {
    workspace
        .slot("asker")
        .unwrap()
        .error()
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// The page the settings mod shows.
fn page(workspace: &Workspace) -> String {
    match workspace.slot("settings").unwrap().view() {
        Some(crate::Node::Text(text)) => text.text.clone(),
        other => panic!("settings shows a text: {other:?}"),
    }
}

#[test]
fn update_opens_raises_and_closes_a_mods_window() {
    let (_dir, mut workspace) = workspace();
    ask(&mut workspace, Data::from("settings"));
    ask(&mut workspace, map(&[("open", Data::from("settings"))]));
    ask(&mut workspace, map(&[("close", Data::from("settings"))]));
    assert_eq!(error(&workspace), "");
    let open = WindowRequest::Open {
        name: "settings".to_owned(),
        send: None,
        place: None,
    };
    assert_eq!(workspace.take_requests(), [
        ("asker".to_owned(), Request::Window(open.clone())),
        ("asker".to_owned(), Request::Window(open)),
        (
            "asker".to_owned(),
            Request::Window(WindowRequest::Close("settings".to_owned()))
        ),
    ]);
}

#[test]
fn a_window_can_ask_for_a_screen_and_to_fill_it() {
    let (_dir, mut workspace) = workspace();
    ask(
        &mut workspace,
        map(&[
            ("open", Data::from("settings")),
            ("display", Data::from("external")),
            ("fullscreen", Data::Bool(true)),
        ]),
    );
    assert_eq!(error(&workspace), "");
    assert_eq!(workspace.take_requests(), [(
        "asker".to_owned(),
        Request::Window(WindowRequest::Open {
            name: "settings".to_owned(),
            send: None,
            place: Some(WindowPlace {
                display: Some(Display::External),
                fullscreen: true,
            }),
        })
    )]);
    ask(
        &mut workspace,
        map(&[
            ("open", Data::from("settings")),
            ("display", Data::from("moon")),
        ]),
    );
    assert!(error(&workspace).contains("`display` is"));
}

#[test]
fn what_a_window_opens_with_reaches_its_mod_first() {
    let (_dir, mut workspace) = workspace();
    assert_eq!(page(&workspace), "general");
    let send = map(&[("page", Data::from("machines"))]);
    ask(
        &mut workspace,
        map(&[("open", Data::from("settings")), ("send", send.clone())]),
    );
    assert_eq!(error(&workspace), "");
    // Delivered by the time the app takes the request, so its window's
    // first frame shows the page.
    assert_eq!(page(&workspace), "machines");
    assert_eq!(workspace.take_requests(), [(
        "asker".to_owned(),
        Request::Window(WindowRequest::Open {
            name: "settings".to_owned(),
            send: Some(send),
            place: None,
        })
    )]);
}

#[test]
fn a_control_can_open_a_window_on_a_page_too() {
    let (_dir, mut workspace) = workspace();
    let send = Action::Send {
        to: Some("app:window".to_owned()),
        msg: map(&[
            ("open", Data::from("settings")),
            ("send", map(&[("page", Data::from("keys"))])),
        ]),
    };
    assert_eq!(workspace.dispatch("asker", &send), None);
    assert_eq!(page(&workspace), "keys");
    assert_eq!(workspace.take_requests().len(), 1);
}

#[test]
fn a_preview_opens_no_window_and_sends_nothing() {
    let (_dir, mut workspace) = workspace();
    let send = Action::Send {
        to: Some("app:window".to_owned()),
        msg: map(&[
            ("open", Data::from("settings")),
            ("send", map(&[("page", Data::from("keys"))])),
        ]),
    };
    assert_eq!(workspace.dispatch("demo:pill", &send), None);
    assert!(workspace.take_requests().is_empty());
    assert_eq!(page(&workspace), "general");
}

#[test]
fn a_bad_window_request_is_the_senders_error() {
    let (_dir, mut workspace) = workspace();
    for (wrong, problem) in [
        (Data::Integer(1), "takes \"<mod>\""),
        (Data::from("shell"), "the shell is the main window"),
        (Data::from("Settings!"), "is not a mod's name"),
        (Data::from("a/b"), "is not a mod's name"),
        (map(&[]), "takes \"<mod>\""),
        (map(&[("open", Data::Integer(2))]), "takes \"<mod>\""),
        (
            map(&[("open", Data::from("a")), ("close", Data::from("a"))]),
            "not both",
        ),
        (
            map(&[("close", Data::from("a")), ("send", Data::from("x"))]),
            "`send` goes with `open`",
        ),
        (
            map(&[("open", Data::from("a")), ("size", Data::Integer(3))]),
            "unknown key `size`",
        ),
    ] {
        ask(&mut workspace, wrong);
        assert!(error(&workspace).contains(problem), "{}", error(&workspace));
    }
    assert!(workspace.take_requests().is_empty());
}

#[test]
fn a_send_to_a_missing_mod_is_the_senders_error() {
    let (_dir, mut workspace) = workspace();
    ask(
        &mut workspace,
        map(&[("open", Data::from("nope")), ("send", Data::from("hi"))]),
    );
    assert!(
        error(&workspace).contains("no mod named `nope`"),
        "{}",
        error(&workspace)
    );
}

#[test]
fn a_mod_says_the_size_of_its_window() {
    let (_dir, workspace) = workspace();
    let meta = workspace.slot("settings").unwrap().meta().unwrap();
    assert_eq!(
        meta.window,
        Some(crate::ModWindowSize {
            size: Some((860., 600.)),
            min_size: Some((640., 480.)),
            ..Default::default()
        })
    );
    let meta = workspace.slot("asker").unwrap().meta().unwrap();
    assert_eq!(meta.window, None);
}

#[test]
fn a_mod_can_hide_its_windows_title_bar() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "main.luau", SHELL_SOURCE);
    write(dir.path(), "mods/asker/main.luau", ASKER);
    write(
        dir.path(),
        "mods/settings/main.luau",
        r#"return mod {
            window = { titlebar = "hidden", traffic_lights = { x = 20, y = 18 } },
            view = function() return ui.text "settings" end,
        }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    let meta = workspace.slot("settings").unwrap().meta().unwrap();
    assert_eq!(
        meta.window,
        Some(crate::ModWindowSize {
            titlebar: crate::Titlebar::Hidden,
            traffic_lights: Some((20., 18.)),
            ..Default::default()
        })
    );
}

#[test]
fn a_mods_window_size_is_checked() {
    for (window, problem) in [
        (
            "{ size = { width = 860 } }",
            "takes both `width` and `height`",
        ),
        ("{ size = { width = 10, height = 600 } }", "width"),
        (
            "{ size = { width = 400, height = 300 }, min_size = { width = 500, height = 200 } }",
            "is larger than `size`",
        ),
        ("{ titlebar = \"glass\" }", "titlebar"),
        ("{ traffic_lights = { x = 900 } }", "traffic_lights"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "main.luau", SHELL_SOURCE);
        write(dir.path(), "mods/asker/main.luau", ASKER);
        write(
            dir.path(),
            "mods/settings/main.luau",
            &format!(
                "return mod {{ window = {window}, view = function() return ui.text 'x' end }}"
            ),
        );
        let workspace = Workspace::open(dir.path(), false);
        let problems = workspace
            .problems()
            .iter()
            .map(|(_, problem)| problem.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(problems.contains(problem), "{window}: {problems}");
    }
}
