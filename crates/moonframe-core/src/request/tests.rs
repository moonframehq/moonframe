use {
    super::*,
    crate::{Action, Workspace},
    std::{collections::BTreeMap, fs, path::Path},
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
    assert!(error(&workspace).contains("`app:command` takes one of quit"));
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

#[test]
fn update_sets_the_appearance() {
    let (_dir, mut workspace) = workspace();
    for (name, appearance) in [
        ("system", Appearance::System),
        ("light", Appearance::Light),
        ("dark", Appearance::Dark),
    ] {
        ask(&mut workspace, "app:appearance", Data::from(name));
        assert_eq!(workspace.take_requests(), [(
            "asker".to_owned(),
            Request::Appearance(appearance)
        )]);
    }
    ask(&mut workspace, "app:appearance", Data::from("sepia"));
    assert!(
        error(&workspace).contains("unknown `sepia`"),
        "{}",
        error(&workspace)
    );
    assert!(workspace.take_requests().is_empty());
}

#[test]
fn the_dark_appearance_follows_the_system_only_when_asked() {
    assert!(Appearance::System.is_dark(true));
    assert!(!Appearance::System.is_dark(false));
    assert!(Appearance::Dark.is_dark(false));
    assert!(!Appearance::Light.is_dark(true));
}

#[test]
fn views_read_the_appearance() {
    let (_dir, mut workspace) = workspace();
    workspace.set_appearance(Appearance::Dark, BTreeMap::new());
    let Some(Data::Map(appearance)) = workspace.app_data().get("appearance").cloned() else {
        panic!("no app.appearance");
    };
    assert_eq!(appearance.get("mode"), Some(&Data::from("dark")));
    assert_eq!(
        appearance.get("dark"),
        Some(&Data::Bool(false)),
        "the window says"
    );
}

#[test]
fn update_picks_the_light_and_dark_themes() {
    let (_dir, mut workspace) = workspace();
    let both = Data::Map(
        [
            ("light".to_owned(), Data::from("Rose Pine Dawn")),
            ("dark".to_owned(), Data::from("Rose Pine")),
        ]
        .into(),
    );
    ask(&mut workspace, "app:theme", both);
    assert_eq!(workspace.take_requests(), [(
        "asker".to_owned(),
        Request::Theme {
            light: Some("Rose Pine Dawn".to_owned()),
            dark: Some("Rose Pine".to_owned()),
        }
    )]);
    let dark = Data::Map([("dark".to_owned(), Data::from("Nord"))].into());
    ask(&mut workspace, "app:theme", dark);
    assert_eq!(workspace.take_requests(), [(
        "asker".to_owned(),
        Request::Theme {
            light: None,
            dark: Some("Nord".to_owned()),
        }
    )]);
    for wrong in [
        Data::from("Nord"),
        Data::Map(BTreeMap::new()),
        Data::Map([("sepia".to_owned(), Data::from("Nord"))].into()),
        Data::Map([("dark".to_owned(), Data::Integer(1))].into()),
    ] {
        ask(&mut workspace, "app:theme", wrong);
        assert!(
            error(&workspace).contains("`app:theme` takes"),
            "{}",
            error(&workspace)
        );
    }
    assert!(workspace.take_requests().is_empty());
}

#[test]
fn the_app_can_refuse_a_request() {
    let (_dir, mut workspace) = workspace();
    workspace.refuse("asker", Error::new("no theme called `Nope`"));
    assert_eq!(error(&workspace), "no theme called `Nope`");
}

#[test]
fn update_plays_a_sound_file_or_the_chime() {
    let (_dir, mut workspace) = workspace();
    ask(&mut workspace, "app:sound", Data::from("default"));
    ask(&mut workspace, "app:sound", Data::from("sounds/Done.WAV"));
    assert_eq!(workspace.take_requests(), [
        ("asker".to_owned(), Request::Sound(Sound::Chime)),
        (
            "asker".to_owned(),
            Request::Sound(Sound::File("sounds/Done.WAV".to_owned()))
        ),
    ]);
    for (wrong, problem) in [
        ("https://example.com/a.wav", "is an address"),
        ("notes.txt", "is not a sound file"),
        ("", "is not a sound file"),
    ] {
        ask(&mut workspace, "app:sound", Data::from(wrong));
        assert!(error(&workspace).contains(problem), "{}", error(&workspace));
    }
    ask(&mut workspace, "app:sound", Data::Bool(true));
    assert!(error(&workspace).contains("takes \"default\""));
    assert!(workspace.take_requests().is_empty());
}

#[test]
fn update_posts_a_notification() {
    let (_dir, mut workspace) = workspace();
    let map = |entries: &[(&str, &str)]| {
        Data::Map(
            entries
                .iter()
                .map(|(key, value)| ((*key).to_owned(), Data::from(*value)))
                .collect(),
        )
    };
    ask(
        &mut workspace,
        "app:notify",
        map(&[
            ("title", "Agent done"),
            ("body", "claude finished"),
            ("id", "pane-3"),
        ]),
    );
    ask(&mut workspace, "app:notify", map(&[("title", "Hi")]));
    assert_eq!(workspace.take_requests(), [
        (
            "asker".to_owned(),
            Request::Notify(Notification {
                title: "Agent done".to_owned(),
                body: "claude finished".to_owned(),
                id: Some("pane-3".to_owned()),
            })
        ),
        (
            "asker".to_owned(),
            Request::Notify(Notification {
                title: "Hi".to_owned(),
                body: String::new(),
                id: None,
            })
        ),
    ]);
    let long = "x".repeat(MAX_TITLE + 1);
    for (wrong, problem) in [
        (map(&[("body", "no title")]), "needs a `title`"),
        (map(&[("title", " ")]), "needs a `title`"),
        (map(&[("title", &long)]), "longer than 256"),
        (map(&[("title", "a"), ("id", "a b")]), "`id` is 1 to"),
        (map(&[("title", "a"), ("url", "x")]), "unknown key `url`"),
        (Data::from("hello"), "takes {"),
    ] {
        ask(&mut workspace, "app:notify", wrong);
        assert!(error(&workspace).contains(problem), "{}", error(&workspace));
    }
    assert!(workspace.take_requests().is_empty());
}

#[test]
fn update_opens_a_pick_panel_and_its_answer_is_the_reply() {
    let (_dir, mut workspace) = workspace();
    let msg = |fields: &[(&str, Data)]| {
        Data::Map(
            fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
        )
    };
    let types = Data::List(vec![Data::from("TIF"), Data::from(".jpg")]);
    ask(
        &mut workspace,
        "app:pick",
        msg(&[
            ("kind", Data::from("open")),
            ("multiple", Data::Bool(true)),
            ("types", types),
            ("reply", msg(&[("opened", Data::Bool(true))])),
        ]),
    );
    assert_eq!(error(&workspace), "");
    let requests = workspace.take_requests();
    let [(from, Request::Pick(pick))] = &requests[..] else {
        panic!("expected a pick, got {requests:?}");
    };
    assert_eq!(from, "asker");
    assert_eq!((pick.kind, pick.multiple), (PickKind::Open, true));
    assert_eq!(pick.types, ["tif", "jpg"]);
    // Files of other types are left out; none left is a cancel.
    let picked = vec!["/a/scan.TIF".into(), "/a/notes.txt".into()];
    assert_eq!(
        pick.answer(Some(picked)),
        msg(&[
            ("opened", Data::Bool(true)),
            ("paths", Data::List(vec![Data::from("/a/scan.TIF")])),
        ])
    );
    assert_eq!(
        pick.answer(None),
        msg(&[
            ("opened", Data::Bool(true)),
            ("cancelled", Data::Bool(true))
        ])
    );
}

#[test]
fn a_save_panel_answers_one_path_and_bad_picks_are_the_senders_error() {
    let (_dir, mut workspace) = workspace();
    let msg = |fields: &[(&str, Data)]| {
        Data::Map(
            fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
        )
    };
    ask(
        &mut workspace,
        "app:pick",
        msg(&[
            ("kind", Data::from("save")),
            ("name", Data::from("scan.tif")),
            ("reply", Data::from("saved")),
        ]),
    );
    let requests = workspace.take_requests();
    let [(_, Request::Pick(pick))] = &requests[..] else {
        panic!("expected a pick, got {requests:?}");
    };
    assert_eq!(
        pick.answer(Some(vec!["/out/scan.tif".into()])),
        msg(&[
            ("msg", Data::from("saved")),
            ("path", Data::from("/out/scan.tif")),
        ])
    );
    for (bad, problem) in [
        (msg(&[("kind", Data::from("open"))]), "needs a `reply`"),
        (
            msg(&[("kind", Data::from("disk")), ("reply", Data::Nil)]),
            "unknown kind `disk`",
        ),
        (
            msg(&[
                ("kind", Data::from("save")),
                ("multiple", Data::Bool(true)),
                ("reply", Data::from("x")),
            ]),
            "picks one file",
        ),
        (
            msg(&[("name", Data::from("a")), ("reply", Data::from("x"))]),
            "go with kind = \"save\"",
        ),
        (
            msg(&[("path", Data::from("/etc")), ("reply", Data::from("x"))]),
            "unknown key `path`",
        ),
    ] {
        ask(&mut workspace, "app:pick", bad);
        assert!(error(&workspace).contains(problem), "{}", error(&workspace));
    }
    assert!(workspace.take_requests().is_empty());
}
