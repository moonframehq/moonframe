use {
    super::*,
    crate::{Action, Workspace},
    std::{fs, path::Path},
};

const SHELL: &str = r#"
return mod {
  view = function(state, app)
    return ui.column { ui.mount "asker" }
  end,
}
"#;

/// A mod that sends `msg` to `to` when it hears `"go"`.
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

fn write(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

fn workspace() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "main.luau", SHELL);
    write(dir.path(), "mods/asker/main.luau", ASKER);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    (dir, workspace)
}

/// Has the mod `asker` send `msg` to `to` from its `update`.
fn ask(workspace: &mut Workspace, to: &str, msg: Data) {
    let go = Data::Map(
        [(
            "go".to_owned(),
            Data::Map([("to".to_owned(), Data::from(to)), ("msg".to_owned(), msg)].into()),
        )]
        .into(),
    );
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

#[test]
fn update_runs_an_app_command_without_a_click() {
    let (_dir, mut workspace) = workspace();
    ask(&mut workspace, "app:command", Data::from("minimize"));
    assert_eq!(error(&workspace), "");
    assert_eq!(workspace.take_requests(), [(
        "asker".to_owned(),
        Request::Command(Command::Minimize)
    )]);
    assert!(workspace.take_requests().is_empty(), "taken once");
}

#[test]
fn a_command_outside_the_allow_list_is_the_senders_error() {
    let (_dir, mut workspace) = workspace();
    ask(&mut workspace, "app:command", Data::from("rm -rf"));
    assert!(
        error(&workspace).contains("unknown `rm -rf`"),
        "{}",
        error(&workspace)
    );
    ask(&mut workspace, "app:command", Data::Integer(1));
    assert!(error(&workspace).contains("takes a command's name"));
    ask(&mut workspace, "app:shell", Data::from("ls"));
    assert!(
        error(&workspace).contains("no `app:shell`"),
        "{}",
        error(&workspace)
    );
    assert!(workspace.take_requests().is_empty());
}

#[test]
fn a_control_can_ask_the_app_too() {
    let (_dir, mut workspace) = workspace();
    let send = Action::Send {
        to: Some("app:command".to_owned()),
        msg: Data::from("show-themes"),
    };
    assert_eq!(workspace.dispatch("asker", &send), None);
    assert_eq!(workspace.take_requests(), [(
        "asker".to_owned(),
        Request::Command(Command::ShowThemes)
    )]);
}

#[test]
fn previews_and_providers_ask_nothing() {
    let (_dir, mut workspace) = workspace();
    // A provider's answer addressed to the app is dropped.
    workspace.send("provider:git", "app:command", Data::from("quit"));
    // So is a preview's, as its commands are.
    let send = Action::Send {
        to: Some("app:command".to_owned()),
        msg: Data::from("quit"),
    };
    assert_eq!(workspace.dispatch("demo:pill", &send), None);
    assert!(workspace.take_requests().is_empty());
}
