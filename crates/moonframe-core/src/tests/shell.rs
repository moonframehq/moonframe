use super::*;

#[test]
fn the_shell_asks_for_its_window_and_views_see_it() {
    use crate::{Chrome, Platform, Titlebar, WindowState};
    let dir = app(&[]);
    let workspace = Workspace::open(dir.path(), false);
    assert_eq!(workspace.chrome(), Chrome::default());
    assert_eq!(workspace.chrome().titlebar, Titlebar::System);

    let shell = r#"return mod {
      window = { titlebar = "hidden", traffic_lights = { x = 12, y = 16 } },
      view = function(_, app)
        return ui.row { drag = true,
          ui.text(`{app.window.platform} {app.window.titlebar} {app.window.maximized}`),
          ui.button { "x", command = "close-window" },
          ui.button { "-", command = "minimize" },
          ui.button { "+", command = "maximize" },
        }
      end,
    }"#;
    write(dir.path(), "main.luau", shell);
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.chrome(), Chrome {
        titlebar: Titlebar::Hidden,
        traffic_lights: Some((12.0, 16.0)),
        size: None,
    });
    let row = |workspace: &Workspace| {
        let Some(Node::Stack(row)) = workspace.shell().view() else {
            panic!("no row");
        };
        row.clone()
    };
    let text = |workspace: &Workspace| match &row(workspace).children[0] {
        Node::Text(text) => text.text.clone(),
        other => panic!("expected text, got {other:?}"),
    };
    assert!(row(&workspace).drag);
    let platform = Platform::current().name();
    assert_eq!(text(&workspace), format!("{platform} system false"));
    workspace.set_window(WindowState {
        titlebar: Titlebar::Hidden,
        maximized: true,
        ..WindowState::default()
    });
    assert_eq!(text(&workspace), format!("{platform} hidden true"));
    let commands: Vec<_> = row(&workspace).children[1..]
        .iter()
        .map(|child| match child {
            Node::Button(button) => button.action.clone(),
            other => panic!("expected a button, got {other:?}"),
        })
        .collect();
    assert_eq!(commands, [
        Some(Action::Command(Command::CloseWindow)),
        Some(Action::Command(Command::Minimize)),
        Some(Action::Command(Command::Maximize)),
    ]);

    for (window, expected) in [
        (r#"{ titlebar = "none" }"#, "unknown `none`"),
        (r#"{ traffic_lights = { x = 500 } }"#, "traffic_lights.x"),
        (r#"{ frameless = true }"#, "unknown key `frameless`"),
    ] {
        let source =
            format!("return mod {{ window = {window}, view = function() return ui.text 'x' end }}");
        write(dir.path(), "main.luau", &source);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace.shell().error().unwrap().to_string();
        assert!(error.contains(expected), "{window}: {error}");
    }

    // A mod is not the shell: its `window` is its own window's, and leaves
    // the main window's title bar alone.
    write(dir.path(), "main.luau", SHELL_SOURCE);
    let source = r#"return mod { window = { titlebar = "hidden" }, view = function() return ui.text "x" end }"#;
    write(dir.path(), "mods/sneaky/main.luau", source);
    let workspace = Workspace::open(dir.path(), false);
    assert!(workspace.slot("sneaky").unwrap().error().is_none());
    assert_eq!(workspace.chrome(), Chrome::default());
}

#[test]
fn the_shell_names_its_menus_and_about() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          about = { version = "1.2", description = "Sessions", website = "https://example.com" },
          menus = {
            { title = "File", items = {
              { label = "New", msg = "new", to = "agent", shortcut = "secondary-t" },
              "-",
              { label = "Minimize", command = "minimize" },
            } },
          },
          view = function() return ui.text "shell" end,
        }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let menus = workspace.menus();
    assert_eq!(menus.len(), 1);
    assert_eq!(menus[0].title, "File");
    let crate::MenuEntry::Item(new) = &menus[0].items[0] else {
        panic!("{:?}", menus[0].items);
    };
    assert_eq!(new.label, "New");
    assert!(
        new.shortcut
            .as_ref()
            .is_some_and(|chord| chord.secondary && chord.key == "t")
    );
    assert!(matches!(&new.action, Action::Send { to: Some(to), .. } if to == "agent"));
    assert_eq!(menus[0].items[1], crate::MenuEntry::Separator);
    assert!(
        matches!(&menus[0].items[2], crate::MenuEntry::Item(item) if item.action == Action::Command(Command::Minimize))
    );
    assert_eq!(workspace.about().version.as_deref(), Some("1.2"));

    // A mod may not set them, and a bad key is named.
    write(
        dir.path(),
        "mods/rogue/main.luau",
        r#"return mod { menus = {}, view = function() return ui.text "x" end }"#,
    );
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          menus = { { title = "File", items = { { label = "New", msg = "new", shortcut = "hyper-t" } } } },
          view = function() return ui.text "shell" end,
        }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    let problems: Vec<String> = workspace
        .problems()
        .iter()
        .map(|(name, error)| format!("{name}: {error}"))
        .collect();
    assert!(
        problems
            .iter()
            .any(|problem| problem.starts_with("rogue") && problem.contains("only the shell")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("shortcut") && problem.contains("unknown modifier")),
        "{problems:?}"
    );
}

#[test]
fn a_menu_shortcut_may_be_one_typing_key() {
    use crate::{Chord, Pressed};
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          menus = { { title = "Mail", items = {
            { label = "Next", msg = "next", shortcut = "j" },
            { label = "Compose", msg = "compose", shortcut = "shift-c" },
            { label = "Find", msg = "find", shortcut = "secondary-f" },
          } } },
          view = function() return ui.text "shell" end,
        }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let typed: Vec<bool> = workspace.menus()[0]
        .items
        .iter()
        .map(|entry| match entry {
            crate::MenuEntry::Item(item) => item.shortcut.as_ref().unwrap().types_text(),
            crate::MenuEntry::Separator => false,
        })
        .collect();
    assert_eq!(typed, [true, true, false]);
    let j = Pressed {
        key: "j",
        text: Some("j"),
        ctrl: false,
        alt: false,
        shift: false,
        cmd: false,
    };
    let crate::MenuEntry::Item(next) = &workspace.menus()[0].items[0] else {
        unreachable!()
    };
    assert!(next.shortcut.as_ref().unwrap().matches(&j, true));
    // Overlays and fields still never take what a key types.
    assert!(Chord::parse("j").is_err());
}

/// Copies the folder `from` into `to`, as the app is shipped.
fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Every stack in `node`, depth first.
fn stacks<'n>(node: &'n Node, found: &mut Vec<&'n crate::Stack>) {
    match node {
        Node::Stack(stack) => {
            found.push(stack);
            for child in &stack.children {
                stacks(child, found);
            }
        },
        Node::Panel(panel) => panel.children.iter().for_each(|child| stacks(child, found)),
        Node::Popover(popover) => popover.anchor.iter().for_each(|child| stacks(child, found)),
        Node::Split(split) => split
            .panes
            .iter()
            .flat_map(|pane| &pane.children)
            .for_each(|child| stacks(child, found)),
        _ => {},
    }
}

#[test]
fn the_apps_tabs_are_keyed_and_the_curved_look_joins_the_main_card() {
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    for look in ["pills", "curved"] {
        let dir = tempfile::tempdir().unwrap();
        copy_folder(&shipped, dir.path());
        let sheet = dir.path().join("style.luau");
        let source = fs::read_to_string(&sheet).unwrap();
        assert!(source.contains(r#"looks = { tabs = "pills" }"#));
        let source = source.replace(
            r#"looks = { tabs = "pills" }"#,
            &format!(r#"looks = {{ tabs = "{look}" }}"#),
        );
        fs::write(&sheet, source).unwrap();
        let workspace = Workspace::open_with(dir.path(), false, Some(Box::new(Memory::default())));
        assert!(
            workspace.problems().is_empty() && workspace.style_error().is_none(),
            "{look}: {:?} {:?}",
            workspace.problems(),
            workspace.style_error()
        );

        let strip = workspace.slot("shell/tabbed#main").unwrap().view().unwrap();
        let mut found = Vec::new();
        stacks(strip, &mut found);
        let keys: Vec<_> = found
            .iter()
            .filter_map(|stack| stack.key.as_deref())
            .collect();
        assert_eq!(keys, ["main-1", "_add"], "{look}");
        let shoulders = found
            .iter()
            .filter(|stack| {
                stack
                    .look
                    .class
                    .iter()
                    .any(|class| class == "tabs-shoulder")
            })
            .count();
        assert_eq!(
            shoulders,
            if look == "curved" {
                2
            } else {
                0
            },
            "{look}"
        );

        let shell = workspace.slot(SHELL).unwrap().view().unwrap();
        let mut found = Vec::new();
        stacks(shell, &mut found);
        let joined = found
            .iter()
            .filter(|stack| {
                stack
                    .look
                    .class
                    .iter()
                    .any(|class| class == "toolbar-joined")
            })
            .count();
        // The center's bar, and one over each side panel.
        assert_eq!(
            joined,
            if look == "curved" {
                3
            } else {
                0
            },
            "{look}"
        );
    }
}

#[test]
fn the_layout_menu_picks_the_tab_look_and_moves_the_title_bar() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let mut workspace =
        Workspace::open_with(root.as_path(), false, Some(Box::new(Memory::default())));
    let classes = |workspace: &Workspace, name: &str, class: &str| {
        let mut found = Vec::new();
        stacks(workspace.slot(name).unwrap().view().unwrap(), &mut found);
        found
            .iter()
            .filter(|stack| stack.look.class.iter().any(|own| own == class))
            .count()
    };
    let joined_card = |workspace: &Workspace| {
        matches!(workspace.slot("editor").unwrap().view(),
            Some(Node::Panel(panel)) if panel.look.class.iter().any(|class| class == "tab-card-joined"))
    };
    assert_eq!(classes(&workspace, SHELL, "toolbar-joined"), 0);
    assert!(!joined_card(&workspace));

    // The title bar's button opens the picker in a window of its own.
    let mut found = Vec::new();
    stacks(workspace.slot(SHELL).unwrap().view().unwrap(), &mut found);
    assert!(
        found
            .iter()
            .any(|stack| stack.action == Some(Action::Window("looks".to_owned())))
    );

    let pick = |key: &str, value: &str| Data::Map([(key.to_owned(), Data::from(value))].into());
    workspace.send("looks", "looks", pick("look", "curved"));
    assert_eq!(classes(&workspace, SHELL, "toolbar-joined"), 3);
    assert_eq!(classes(&workspace, "shell/tabbed#main", "tabs-shoulder"), 2);
    assert!(joined_card(&workspace), "{:?}", workspace.problems());

    // Back to the stylesheet's look, but with the bar over the editor.
    workspace.send("looks", "looks", pick("look", "stylesheet"));
    workspace.send("looks", "looks", pick("layout", "joined"));
    assert_eq!(classes(&workspace, "shell/tabbed#main", "tabs-shoulder"), 0);
    assert_eq!(classes(&workspace, SHELL, "toolbar-joined"), 3);
    assert!(joined_card(&workspace));

    // A mount names a mod, never a path.
    let dir = app(&[(
        "escape",
        r#"return mod { view = function() return ui.column { ui.mount { "../../etc", id = "x" } } end }"#,
    )]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(broken(&workspace, "escape").contains("is not a mod name"));
    assert!(workspace.roots().mod_candidates("../escape").is_empty());

    // `window` stands alone on a control.
    let dir = app(&[(
        "bad",
        r#"return mod { view = function() return ui.row { window = "looks", msg = "x" } end }"#,
    )]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("bad")
        .unwrap()
        .error()
        .map(ToString::to_string);
    assert!(
        error
            .as_deref()
            .is_some_and(|error| error.contains("takes no `msg`")),
        "{error:?}"
    );
}

#[test]
fn the_apps_tab_bar_opens_kinds_keeps_its_tabs_and_publishes_them() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(root.as_path(), false, Some(Box::new(memory.clone())));
    // The title bar's strip is the shell's; the main area draws its content.
    let key = "shell/tabbed#main";
    let tabs = |workspace: &Workspace| -> Vec<(String, String)> {
        let Some(Data::Map(state)) = memory.0.borrow().0.get(&format!("mod:{key}")).cloned() else {
            panic!("no saved tabs; problems: {:?}", workspace.problems());
        };
        let Some(Data::List(tabs)) = state.get("tabs").cloned() else {
            panic!("no tabs: {state:?}");
        };
        tabs.into_iter()
            .map(|tab| match tab {
                Data::Map(tab) => match (tab.get("id"), tab.get("kind")) {
                    (Some(Data::String(id)), Some(Data::String(kind))) => {
                        (id.clone(), kind.clone())
                    },
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            })
            .collect()
    };
    // It starts with a terminal, and its kinds include the browser and the explorer.
    assert_eq!(tabs(&workspace), [(
        "main-1".to_owned(),
        "component:terminal".to_owned()
    )]);
    assert!(workspace.slot(key).is_some(), "{:?}", workspace.problems());

    // + then a pick opens a browser tab, selected, drawn as the browser component.
    workspace.send(key, key, Data::from("add"));
    let pick = Data::Map([("pick".to_owned(), Data::from("component:browser"))].into());
    workspace.send(key, key, pick);
    assert_eq!(
        tabs(&workspace)[1],
        ("main-2".to_owned(), "component:browser".to_owned())
    );
    // The change is published on its topic.
    let topic = memory.0.borrow().0.get("topic:tabs/main").cloned();
    assert!(
        matches!(&topic, Some(Data::Map(fields)) if fields.get("count") == Some(&Data::Integer(2))),
        "{topic:?}"
    );

    // The main area followed the strip over `tabs/main`: it draws the browser.
    let editor = memory.0.borrow().0.get("mod:editor").cloned();
    assert!(
        matches!(&editor, Some(Data::Map(state)) if matches!(state.get("tab"), Some(Data::Map(tab)) if tab.get("id") == Some(&Data::from("main-2")))),
        "{editor:?}"
    );
    // What the browser reports there comes back to the strip, on the topic.
    let report = Data::Map(
        [
            ("tab_report".to_owned(), Data::from("main-2")),
            ("url".to_owned(), Data::from("https://example.com/")),
            ("title".to_owned(), Data::from("Example")),
        ]
        .into(),
    );
    workspace.send("editor", "editor", report);
    let strip = memory.0.borrow().0.get(&format!("mod:{key}")).cloned();
    assert!(format!("{strip:?}").contains("Example"), "{strip:?}");

    // A shell that exits (Ctrl-D) reports `exited`, and its tab closes.
    let pick = Data::Map([("pick".to_owned(), Data::from("component:terminal"))].into());
    workspace.send(key, key, pick);
    assert_eq!(tabs(&workspace).len(), 3);
    // Its tab is named after the folder its shell reports, then the title.
    let shell = |fields: &[(&str, &str)]| {
        let mut report = vec![("tab_report".to_owned(), Data::from("main-3"))];
        report.extend(
            fields
                .iter()
                .map(|(name, value)| ((*name).to_owned(), Data::from(*value))),
        );
        Data::Map(report.into_iter().collect())
    };
    let strip_text = || {
        let strip = memory.0.borrow().0.get(&format!("mod:{key}")).cloned();
        format!("{strip:?}")
    };
    workspace.send(
        "editor",
        "editor",
        shell(&[("event", "folder"), ("path", "/Users/me/my-project/")]),
    );
    assert!(strip_text().contains("my-project"), "{}", strip_text());
    assert!(!strip_text().contains("/Users/me"), "{}", strip_text());
    workspace.send(
        "editor",
        "editor",
        shell(&[("event", "title"), ("title", "vim notes.md")]),
    );
    assert!(strip_text().contains("vim notes.md"), "{}", strip_text());
    // A prompt says nothing about the tab; a notice is for the user.
    workspace.send("editor", "editor", shell(&[("event", "prompt")]));
    assert!(strip_text().contains("vim notes.md"), "{}", strip_text());
    workspace.take_requests();
    workspace.send(
        "editor",
        "editor",
        shell(&[("event", "notice"), ("body", "Waiting for you")]),
    );
    assert!(
        workspace
            .take_requests()
            .iter()
            .any(|(_, request)| matches!(
                request,
                crate::Request::Notify(notice) if notice.body == "Waiting for you"
            )),
        "no notification"
    );
    // A tab not on show tells what its shell does, until it is shown.
    let hidden = |fields: Vec<(&str, Data)>| {
        let mut report = vec![("tab_report".to_owned(), Data::from("main-1"))];
        report.extend(
            fields
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value)),
        );
        Data::Map(report.into_iter().collect())
    };
    let event = |name: &str| ("event", Data::from(name));
    workspace.send("editor", "editor", hidden(vec![event("started")]));
    assert!(
        strip_text().contains("\"running\": Bool(true)"),
        "{}",
        strip_text()
    );
    workspace.send(
        "editor",
        "editor",
        hidden(vec![event("finished"), ("took_ms", Data::Integer(5))]),
    );
    assert!(!strip_text().contains("running"), "{}", strip_text());
    assert!(strip_text().contains("\"done\""), "{}", strip_text());
    workspace.take_requests();
    workspace.send(
        "editor",
        "editor",
        hidden(vec![
            event("finished"),
            ("command", Data::from("cargo test")),
            ("status", Data::Integer(101)),
            ("took_ms", Data::Integer(12_000)),
        ]),
    );
    let long = workspace.take_requests();
    assert!(
        long.iter().any(|(_, request)| matches!(
            request,
            crate::Request::Notify(notice)
                if notice.title == "Command failed" && !notice.body.contains("cargo test")
        )),
        "no notification of the long command, or one holding its command line: {long:?}"
    );
    workspace.send(
        "editor",
        "editor",
        hidden(vec![event("notice"), ("body", Data::from("Need you"))]),
    );
    assert!(strip_text().contains("\"blocked\""), "{}", strip_text());
    workspace.send(
        key,
        key,
        Data::Map([("select".to_owned(), Data::from("main-1"))].into()),
    );
    assert!(!strip_text().contains("\"blocked\""), "{}", strip_text());
    workspace.send(
        key,
        key,
        Data::Map([("select".to_owned(), Data::from("main-3"))].into()),
    );

    // However long a program's text, it reaches the user, cut to fit.
    workspace.take_requests();
    let long = "é".repeat(1024);
    workspace.send(
        "editor",
        "editor",
        shell(&[("event", "notice"), ("title", &long), ("body", &long)]),
    );
    let requests = workspace.take_requests();
    let notice = requests.iter().find_map(|(_, request)| match request {
        crate::Request::Notify(notice) => Some(notice),
        _ => None,
    });
    let notice = notice.expect("a long notice was refused");
    assert_eq!(notice.title, "é".repeat(128));
    assert_eq!(notice.body, long);
    let exited = Data::Map(
        [
            ("tab_report".to_owned(), Data::from("main-3")),
            ("exited".to_owned(), Data::Bool(true)),
        ]
        .into(),
    );
    workspace.send("editor", "editor", exited);
    assert!(
        tabs(&workspace).iter().all(|(id, _)| id != "main-3"),
        "{:?}",
        tabs(&workspace)
    );

    // A mod kind: the explorer opens as an instance of its own.
    let pick = Data::Map([("pick".to_owned(), Data::from("mod:explorer"))].into());
    workspace.send(key, key, pick);
    assert!(
        workspace.slot("explorer#main-4").is_some(),
        "{:?}",
        workspace.problems()
    );

    // Closing the browser keeps the others; and they come back at the next launch.
    workspace.send(
        key,
        key,
        Data::Map([("close".to_owned(), Data::from("main-2"))].into()),
    );
    assert_eq!(tabs(&workspace).len(), 2);
    drop(workspace);
    let workspace = Workspace::open_with(root.as_path(), false, Some(Box::new(memory.clone())));
    assert_eq!(tabs(&workspace).len(), 2);
    assert!(
        workspace.slot("explorer#main-4").is_some(),
        "the selected explorer tab is drawn again"
    );
}
