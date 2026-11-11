//! Runs on the app itself (`app = true`): the agent is told about the app
//! the host runs, edits its folder without asking under `allow = "app"`,
//! and a change that breaks the app is undone.

use super::*;

/// A rig whose stand-in agent's tool is of `kind` on `path` (from the
/// rig's folder), with `app/` as the app's writable folder and `check` as
/// its check.
fn app_rig(kind: &str, path: &str, check: &str, agent: &str) -> Rig {
    // As agents send them: a file tool names its file, a shell its command.
    let key = if kind == "execute" {
        "command"
    } else {
        "file_path"
    };
    let agent = agent.replace(
        r#""kind":"edit","status":"in_progress""#,
        &format!(
            r#""kind":"{kind}","status":"in_progress","locations":[{{"path":"{path}"}}],"rawInput":{{"{key}":"{path}"}}"#
        ),
    );
    let mut rig = rig_with(&agent, json!({ "feedback": "app" }));
    let root = rig._dir.path().to_owned();
    let app = root.join("app");
    std::fs::create_dir_all(app.join("mods/home")).unwrap();
    std::fs::write(app.join("mods/home/main.luau"), "return mod {}\n").unwrap();
    rig.acp.editable = Some(Editable {
        title: "Notes".to_owned(),
        layers: vec![("app".to_owned(), app.clone())],
        app: app.clone(),
        writable: app.clone(),
        workdir: root,
        check: check.to_owned(),
        guide: Some("Pages are in mods/home.".to_owned()),
        state: None,
    });
    rig.acp.writable = Some(app);
    rig
}

#[test]
fn a_run_on_the_app_is_told_about_it_and_edits_it_without_asking() {
    let mut rig = app_rig("edit", "app/mods/home/main.luau", "true", AGENT);
    rig.send(
        "feedback",
        json!({ "start": "Make it red", "app": true, "instructions": "Be quick." }),
    );
    let run = rig.wait_for("done");
    assert_eq!(answered(&rig), "yes");
    let prompt = rig.sent()[2]["params"]["prompt"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        prompt.starts_with("Be quick.\n\n## This app: Notes"),
        "{prompt}"
    );
    assert!(prompt.contains("Check the app with `true`"), "{prompt}");
    assert!(prompt.contains("Pages are in mods/home."), "{prompt}");
    assert!(prompt.contains("mods/home/main.luau (1)"), "{prompt}");
    assert!(
        prompt.ends_with("The user's request:\nMake it red"),
        "{prompt}"
    );
    // How long each part took, and the tool.
    let timing = &run["timing"];
    for mark in ["spawned", "initialized", "session", "first_token", "ended"] {
        assert!(timing[mark].is_u64(), "{mark} in {timing}");
    }
    assert!(run["tools"][0]["ms"].is_u64(), "{run}");
}

#[test]
fn a_run_not_on_the_app_is_told_nothing_of_it() {
    let mut rig = app_rig("edit", "app/x.luau", "true", AGENT);
    rig.send("feedback", json!({ "start": "Hello" }));
    rig.wait_for("done");
    let prompt = rig.sent()[2]["params"]["prompt"][0]["text"].clone();
    assert_eq!(prompt, "Hello");
}

#[test]
fn edits_outside_the_app_and_other_commands_still_ask() {
    for (kind, path) in [("edit", "crates/lib.rs"), ("execute", "rm -rf app")] {
        let mut rig = app_rig(kind, path, "true", AGENT);
        rig.send("feedback", json!({ "start": "Go", "app": true }));
        let run = rig.wait_for("waiting");
        assert!(run["question"].is_object(), "{kind}: {run}");
    }
}

#[test]
fn the_apps_check_runs_without_asking() {
    let mut rig = app_rig("execute", "notes --check", "notes --check", AGENT);
    rig.send("feedback", json!({ "start": "Go", "app": true }));
    rig.wait_for("done");
    assert_eq!(answered(&rig), "yes");
}

#[test]
fn a_change_that_breaks_the_app_is_undone() {
    // The agent breaks the app a moment after it starts, once the check
    // before it has run.
    let agent = AGENT.replace(
        r#"reply '{"jsonrpc":"2.0","id":3,"#,
        r#"sleep 0.5; : >broken; reply '{"jsonrpc":"2.0","id":3,"#,
    );
    let check = "if [ -e broken ]; then echo 'mods/home/main.luau:3: oops'; exit 1; fi";
    let mut rig = app_rig("edit", "app/mods/home/main.luau", check, &agent);
    let notes = Arc::new(Notes::default());
    rig.acp.keeper = Some(notes.clone());
    rig.send("feedback", json!({ "start": "Break it", "app": true }));
    let id = rig.wait_for("done")["id"].as_str().unwrap().to_owned();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rig.run()["undone"] != true {
        assert!(Instant::now() < deadline, "still {}", rig.run());
        std::thread::sleep(Duration::from_millis(20));
        rig.acp.tick(&rig.host);
    }
    let run = rig.run();
    assert!(
        run["warning"]
            .as_str()
            .unwrap()
            .contains("mods/home/main.luau:3: oops"),
        "{run}"
    );
    assert_eq!(run["checking"], false);
    assert!(run["timing"]["check"].is_u64(), "{run}");
    assert_eq!(
        notes.calls.lock().unwrap().last(),
        Some(&format!("undo {id}"))
    );
}

#[test]
fn a_change_that_keeps_the_app_working_stays() {
    let mut rig = app_rig("edit", "app/mods/home/main.luau", "echo fine", AGENT);
    let notes = Arc::new(Notes::default());
    rig.acp.keeper = Some(notes.clone());
    rig.send("feedback", json!({ "start": "Make it red", "app": true }));
    let id = rig.wait_for("done")["id"].as_str().unwrap().to_owned();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !rig.run()["timing"]["check"].is_u64() {
        assert!(Instant::now() < deadline, "still {}", rig.run());
        std::thread::sleep(Duration::from_millis(20));
        rig.acp.tick(&rig.host);
    }
    assert_eq!(rig.run()["undone"], false);
    assert_eq!(*notes.calls.lock().unwrap(), [
        format!("before {id}"),
        format!("after {id}")
    ]);
}
