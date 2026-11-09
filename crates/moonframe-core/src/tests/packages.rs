use super::*;

#[test]
fn the_users_folder_goes_over_the_apps() {
    let builtin = app(&[("styled", STYLED), ("counter", COUNTER)]);
    write(builtin.path(), "style.luau", STYLE);
    let user = tempfile::tempdir().unwrap();
    // Replaces the built-in counter, adds a mod, and restyles one property
    // of one rule and one color.
    write(
        user.path(),
        "mods/counter/main.luau",
        r#"return mod { region = "left", view = function() return ui.text "mine" end }"#,
    );
    write(
        user.path(),
        "mods/extra/main.luau",
        r#"return mod { view = function() return ui.text "extra" end }"#,
    );
    write(
        user.path(),
        "style.luau",
        r##"return style { radius = 9, mono = { size = 15 }, colors = { accent = "#ff0000" }, rules = { panel = { radius = 4 } } }"##,
    );
    let roots = crate::Roots::new([builtin.path().to_owned(), user.path().to_owned()]);
    let workspace = Workspace::open(roots, false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.mod_names().collect::<Vec<_>>(), [
        "counter", "extra", "styled"
    ]);
    let Some(Node::Text(text)) = workspace.slot("counter").unwrap().view() else {
        panic!("the user's counter did not replace the app's");
    };
    assert_eq!(text.text, "mine");

    let sheet = workspace.stylesheet();
    assert_eq!((sheet.radius, sheet.font_size), (Some(9.0), Some(13.0)));
    // The user's size, over the app's family.
    assert_eq!(
        (sheet.mono.size, sheet.mono.family.as_deref()),
        (Some(15.0), Some("Fira Code"))
    );
    assert_eq!(sheet.overrides, [(Token::Accent, Shade {
        light: 0xff0000ff,
        dark: 0xff0000ff
    })]);
    let style = sheet
        .resolve(
            Part::Panel,
            &crate::Look::default(),
            false,
            &Rules::default(),
        )
        .base;
    // The user's radius, over the app's background and shadow.
    assert_eq!(style.radius, Some(Corners::all(4.0)));
    assert_eq!(style.shadow, Some(Shadow::Preset(ShadowPreset::Small)));
    assert!(style.background.is_some());
}

#[test]
fn a_users_broken_copy_falls_back_to_the_apps_until_it_loads() {
    let builtin = app(&[("counter", COUNTER)]);
    let user = tempfile::tempdir().unwrap();
    write(
        user.path(),
        "mods/counter/main.luau",
        "return mod { view = ",
    );
    let roots = crate::Roots::new([builtin.path().to_owned(), user.path().to_owned()]);
    let workspace = Workspace::open(roots, false);
    let slot = workspace.slot("counter").unwrap();
    // The app's counter draws, under the error of the user's copy.
    assert!(slot.view().is_some());
    assert!(slot.source().unwrap().starts_with(builtin.path()));
    let error = slot.error().unwrap().to_string();
    assert!(
        error.contains("showing") && error.contains(&*user.path().to_string_lossy()),
        "{error}"
    );
}

#[test]
fn an_app_package_brings_its_shell_and_keeps_only_the_builtin_mods_it_names() {
    const PANEL: &str = r#"return mod { view = function() return ui.text "x" end }"#;
    let builtin = app(&[("explorer", PANEL), ("terminal", PANEL)]);
    write(builtin.path(), "components/card.luau", BADGE_COMPONENT);
    write(builtin.path(), "style.luau", "return style { radius = 3 }");
    let dir = tempfile::tempdir().unwrap();
    let (pi, needed, global, profile) = (
        dir.path().join("pi"),
        dir.path().join("needed"),
        dir.path().join("global"),
        dir.path().join("global/profiles/pi"),
    );
    write(&pi, "main.luau", SHELL_SOURCE);
    write(&pi, "mods/sessions/main.luau", PANEL);
    write(&needed, "mods/changes/main.luau", PANEL);
    // The user's own mods and shell stay out of every profile; their
    // stylesheet follows them in.
    write(&global, "main.luau", "error('the global shell')");
    write(&global, "mods/nytimes/main.luau", PANEL);
    write(&global, "style.luau", "return style { radius = 9 }");
    fs::create_dir_all(&profile).unwrap();

    let roots = crate::Roots::profile(
        builtin.path().to_owned(),
        [needed.clone()],
        Some(crate::AppLayer {
            path: pi.clone(),
            keep: vec!["terminal".to_owned()],
        }),
        Some(global.clone()),
        profile.clone(),
    );
    assert_eq!(roots.file("main.luau"), Some(pi.join("main.luau")));
    assert_eq!(roots.candidates("main.luau"), [pi.join("main.luau")]);
    assert!(roots.entries("components", "luau").contains_key("card"));
    let workspace = Workspace::open(roots, false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.mod_names().collect::<Vec<_>>(), [
        "changes", "sessions", "terminal"
    ]);
    assert_eq!(workspace.stylesheet().radius, Some(9.0));

    // The profile's own shell wins over the app's.
    write(&profile, "main.luau", SHELL_SOURCE);
    let roots = crate::Roots::profile(
        builtin.path().to_owned(),
        [],
        Some(crate::AppLayer {
            path: pi,
            keep: vec![],
        }),
        Some(global.clone()),
        profile.clone(),
    );
    assert_eq!(roots.file("main.luau"), Some(profile.join("main.luau")));

    // A profile without an app starts as the built-in app.
    fs::remove_file(profile.join("main.luau")).unwrap();
    let roots = crate::Roots::profile(builtin.path().to_owned(), [], None, Some(global), profile);
    assert_eq!(
        roots.file("main.luau"),
        Some(builtin.path().join("main.luau"))
    );
    let workspace = Workspace::open(roots, false);
    assert_eq!(workspace.mod_names().collect::<Vec<_>>(), [
        "explorer", "terminal"
    ]);
}

#[test]
fn a_hosted_app_stands_on_the_toolbox_under_its_packages_and_the_user() {
    const PANEL: &str = r#"return mod { view = function() return ui.text "x" end }"#;
    let toolbox = app(&[("explorer", PANEL), ("terminal", PANEL)]);
    write(toolbox.path(), "components/card.luau", BADGE_COMPONENT);
    let dir = tempfile::tempdir().unwrap();
    let (host, package, user) = (
        dir.path().join("host"),
        dir.path().join("extras"),
        dir.path().join("user"),
    );
    write(&host, "main.luau", SHELL_SOURCE);
    write(&host, "mods/sessions/main.luau", PANEL);
    write(&host, "mods/notes/main.luau", "error('the host notes')");
    // A package the user installed replaces the host's mod of its name.
    write(&package, "mods/notes/main.luau", PANEL);
    write(&user, "style.luau", "return style { radius = 7 }");

    let roots = crate::Roots::hosted(
        toolbox.path().to_owned(),
        crate::AppLayer {
            path: host.clone(),
            keep: vec!["terminal".to_owned()],
        },
        [package],
        Some(user),
    );
    assert_eq!(roots.file("main.luau"), Some(host.join("main.luau")));
    assert!(roots.entries("components", "luau").contains_key("card"));
    let workspace = Workspace::open(roots, false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    // The toolbox's explorer stays out; its terminal is kept.
    assert_eq!(workspace.mod_names().collect::<Vec<_>>(), [
        "notes", "sessions", "terminal"
    ]);
    assert_eq!(workspace.stylesheet().radius, Some(7.0));
}

#[test]
fn packages_lie_between_the_app_and_the_user_and_never_replace_the_shell() {
    let builtin = app(&[]);
    let dir = tempfile::tempdir().unwrap();
    let (single, folder, user) = (
        dir.path().join("My.Clock"),
        dir.path().join("extras"),
        dir.path().join("user"),
    );
    write(
        &single,
        "main.luau",
        r#"return mod { view = function() return ui.text "clock" end }"#,
    );
    write(
        &single,
        "style.luau",
        "error('a single mod has no stylesheet')",
    );
    write(
        &folder,
        "mods/extra/main.luau",
        r#"return mod { view = function() return ui.text "extra" end }"#,
    );
    write(&folder, "style.luau", "return style { radius = 7 }");
    fs::create_dir_all(&user).unwrap();
    let roots = crate::Roots::with_packages(
        builtin.path().to_owned(),
        [single.clone(), folder.clone()],
        Some(user),
    );
    assert_eq!(roots.layers().count(), 4);
    assert_eq!(
        roots.file("main.luau"),
        Some(builtin.path().join("main.luau"))
    );
    assert_eq!(roots.files("style.luau"), [folder.join("style.luau")]);
    let workspace = Workspace::open(roots, false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.mod_names().collect::<Vec<_>>(), [
        "extra", "my-clock"
    ]);
    assert_eq!(workspace.stylesheet().radius, Some(7.0));
}

#[test]
fn the_developer_tools_come_with_the_feedback_mod_or_on_their_own() {
    const PANEL: &str = r#"return mod { view = function() return ui.text "x" end }"#;
    let builtin = app(&[
        ("devtools", PANEL),
        ("feedback", PANEL),
        ("terminal", PANEL),
    ]);
    let brings = |keep: &[&str]| {
        let app = tempfile::tempdir().unwrap();
        let roots = crate::Roots::hosted(
            builtin.path().to_owned(),
            crate::AppLayer {
                path: app.path().to_owned(),
                keep: keep.iter().map(|name| (*name).to_owned()).collect(),
            },
            [],
            None,
        );
        !roots.mod_candidates("devtools").is_empty()
    };
    assert!(brings(&["feedback"]));
    assert!(brings(&["devtools"]));
    assert!(!brings(&["terminal"]));
    assert!(!brings(&[]));
}
