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
