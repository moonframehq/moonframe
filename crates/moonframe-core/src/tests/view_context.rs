use super::{state::TALLY, *};

/// A mod whose view reads one provider key, `git`.
const BRANCH: &str = r#"
return mod {
  init = function(previous) return previous or { open = true, picked = "main" } end,
  update = function(state, msg) if msg == "close" then state.open = false end end,
  view = function(state, app)
    local branch = app.git and app.git.branch or "?"
    return ui.panel { ui.text(branch), ui.button { "Close", msg = "close" } }
  end,
}
"#;

#[test]
fn a_view_context_holds_the_live_state_and_what_the_view_read() {
    let dir = app(&[("branch", BRANCH)]);
    let mut workspace = Workspace::open(dir.path(), false);
    workspace.set_host("git", Data::map([("branch", Data::from("feat/x"))]));
    workspace.set_host("unread", Data::from("not read by the view"));
    workspace.send("branch", "branch", Data::from("close"));

    let context = workspace.view_context("branch").unwrap();
    assert_eq!(
        context.state,
        Some(Ok(Data::map([
            ("open", Data::Bool(false)),
            ("picked", Data::from("main")),
        ])))
    );
    assert_eq!(context.props, Data::Nil);
    let reads = context.reads.unwrap();
    assert!(reads.contains(&(
        "git".to_owned(),
        Data::map([("branch", Data::from("feat/x"))])
    )));
    assert!(reads.iter().all(|(key, _)| key != "unread"));
    assert!(context.problems.is_empty());
    assert_eq!(workspace.view_context("nobody"), None);
}

#[test]
fn a_view_context_holds_what_an_instance_was_drawn_with() {
    let dir = app(&[("tally", TALLY)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function()
            return ui.mount { "tally", id = "b", props = { label = "B", start = 10 } }
        end }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    let context = workspace.view_context("tally#b").unwrap();
    assert_eq!(
        context.props,
        Data::map([("label", Data::from("B")), ("start", Data::Integer(10))])
    );
    assert_eq!(
        context.state,
        Some(Ok(Data::map([("count", Data::Integer(10))])))
    );
}

#[test]
fn a_view_context_holds_the_mod_problems() {
    let dir = app(&[(
        "broken",
        "return mod { view = function() error('boom') end }",
    )]);
    let workspace = Workspace::open(dir.path(), false);
    let context = workspace.view_context("broken").unwrap();
    assert!(
        context
            .problems
            .iter()
            .any(|problem| problem.contains("boom")),
        "{:?}",
        context.problems
    );
}

#[test]
fn a_mod_reads_another_mods_context_only_when_it_is_the_apps_own() {
    let dir = app(&[
        ("branch", BRANCH),
        (
            "asker",
            "return mod { view = function() return ui.text 'x' end }",
        ),
    ]);
    let workspace = Workspace::open(dir.path(), false);
    // The user's own reader, the mod that drew it, and one of the app's own.
    assert!(workspace.view_context_for("branch", None).is_some());
    assert!(
        workspace
            .view_context_for("branch", Some("branch"))
            .is_some()
    );
    assert!(
        workspace
            .view_context_for("branch", Some("asker"))
            .is_some()
    );
    // A module the app does not hold from its own folders reads nothing.
    assert_eq!(workspace.view_context_for("branch", Some("stranger")), None);
}

#[test]
fn a_package_inside_the_users_folder_reads_no_other_mods_context() {
    let builtin = app(&[("branch", BRANCH)]);
    let user = tempfile::tempdir().unwrap();
    // Where installed packages live: inside the user's own folder.
    let package = user.path().join("packages/snoop");
    write(
        &package,
        "mods/snoop/main.luau",
        "return mod { view = function() return ui.text 'x' end }",
    );
    write(
        user.path(),
        "mods/mine/main.luau",
        "return mod { view = function() return ui.text 'y' end }",
    );
    let roots = crate::Roots::with_packages(
        builtin.path().to_owned(),
        [package],
        Some(user.path().to_owned()),
    );
    let workspace = Workspace::open(roots, false);
    assert!(
        workspace
            .slot("snoop")
            .is_some_and(|slot| slot.source().is_some())
    );
    assert_eq!(workspace.view_context_for("branch", Some("snoop")), None);
    assert!(workspace.view_context_for("branch", Some("mine")).is_some());
}

#[test]
fn a_mod_in_its_own_window_reports_that_window() {
    let report = r#"return mod { view = function(_, app)
        return ui.text(tostring(app.window.maximized))
    end }"#;
    let dir = app(&[("tool", report)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let own = crate::WindowState {
        maximized: true,
        ..Default::default()
    };
    workspace.set_mod_window("tool", Some(own));
    let reads = workspace.view_context("tool").unwrap().reads.unwrap();
    let window = reads.iter().find(|(key, _)| key == "window").unwrap();
    assert_eq!(window.1.get("maximized"), Some(&Data::Bool(true)));
}

#[test]
fn a_view_context_reports_the_app_keys_as_the_view_got_them() {
    let read = r#"return mod { view = function(_, app)
        return ui.text(tostring(app.dark) .. tostring(app.git))
    end }"#;
    let dir = app(&[("reader", read)]);
    let mut workspace = Workspace::open(dir.path(), false);
    // A host key named like one `app` sets itself is not what the view saw.
    workspace.set_host("dark", Data::from("shadowed"));
    workspace.set_host("git", Data::from("main"));
    let reads = workspace.view_context("reader").unwrap().reads.unwrap();
    let value = |name: &str| {
        reads
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(value("dark"), Some(Data::Bool(false)));
    assert_eq!(value("git"), Some(Data::from("main")));
}

#[test]
fn a_mod_reads_the_context_of_its_own_instances() {
    let dir = app(&[("tally", TALLY)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function()
            return ui.mount { "tally", id = "b", props = { label = "B", start = 10 } }
        end }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    assert!(workspace.may_read_context("tally#b", Some("tally")));
    assert!(workspace.may_read_context("tally#b/tabs#main", Some("tally#b")));
    assert!(!workspace.may_read_context("tally#b", Some("stranger")));
}
