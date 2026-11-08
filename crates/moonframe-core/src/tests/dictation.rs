//! Exercise the shipped mic and feedback mod through real instance routing.

use super::*;

const BUTTON: &str = "feedback/dictation#prompt";

fn map(fields: &[(&str, Data)]) -> Data {
    Data::Map(
        fields
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone()))
            .collect(),
    )
}

fn fixture(source: &str) -> (tempfile::TempDir, Workspace) {
    let dir = app(&[("feedback", source)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function() return ui.mount "feedback" end }"#,
    );
    let components = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/components");
    for name in [
        "agent-run",
        "avatar",
        "dictation",
        "dropdown",
        "feedback",
        "kbd",
        "menu",
    ] {
        write(
            dir.path(),
            &format!("components/{name}.luau"),
            &fs::read_to_string(components.join(format!("{name}.luau"))).unwrap(),
        );
    }
    let mut workspace = Workspace::open(dir.path(), false);
    speech(&mut workspace, "idle");
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    (dir, workspace)
}

fn speech(workspace: &mut Workspace, state: &str) {
    workspace.set_host(
        "speech",
        map(&[
            ("available", Data::Bool(true)),
            ("state", state.into()),
            ("owner", "feedback".into()),
        ]),
    );
}

fn button(workspace: &mut Workspace, message: Data) {
    workspace.send(BUTTON, BUTTON, message);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
}

fn reply(workspace: &mut Workspace, action: &str, ok: bool, fields: &[(&str, Data)]) {
    let mut fields = fields.to_vec();
    fields.extend([
        ("reply", "dictation".into()),
        ("action", action.into()),
        ("ok", Data::Bool(ok)),
    ]);
    workspace.send("provider:speech", BUTTON, map(&fields));
}

fn sent(workspace: &mut Workspace, action: &str) -> bool {
    workspace.take_outbox().iter().any(|(provider, _, message)| {
        provider == "speech" && matches!(message, Data::Map(fields) if fields.get(action) == Some(&Data::Bool(true)))
    })
}

#[test]
fn closing_feedback_cancels_a_microphone_still_opening() {
    let (_dir, mut workspace) = fixture(include_str!("../../../../app/mods/feedback/main.luau"));
    workspace.send("feedback", "feedback", "open".into());
    button(&mut workspace, "start".into());
    assert!(sent(&mut workspace, "start"));
    workspace.send("feedback", "feedback", "close".into());
    assert!(sent(&mut workspace, "cancel"));
    // The start acknowledgment can arrive after the dialog has closed.
    reply(&mut workspace, "start", true, &[]);
    reply(
        &mut workspace,
        "transcript",
        false,
        &[("cancelled", Data::Bool(true))],
    );
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
}

#[test]
fn hidden_feedback_cancels_a_late_listening_notification() {
    let (_dir, mut workspace) = fixture(include_str!("../../../../app/mods/feedback/main.luau"));
    workspace.send(
        BUTTON,
        "feedback",
        map(&[("msg", "listening".into()), ("value", Data::Bool(true))]),
    );
    assert!(sent(&mut workspace, "cancel"));
}

#[test]
fn a_failed_start_clears_the_parents_listening_state() {
    let (_dir, mut workspace) = fixture(include_str!("../../../../app/mods/feedback/main.luau"));
    workspace.send("feedback", "feedback", "open".into());
    button(&mut workspace, "start".into());
    assert!(sent(&mut workspace, "start"));
    reply(
        &mut workspace,
        "start",
        false,
        &[("error", "No microphone".into())],
    );
    workspace.send("feedback", "feedback", "close".into());
    assert!(!sent(&mut workspace, "cancel"));
}

#[test]
fn reload_cancels_pending_recording_and_transcribing_takes() {
    for phase in ["opening", "recording", "transcribing"] {
        let (_dir, mut workspace) =
            fixture(include_str!("../../../../app/mods/feedback/main.luau"));
        workspace.send("feedback", "feedback", "open".into());
        button(&mut workspace, "start".into());
        assert!(sent(&mut workspace, "start"));
        if phase != "opening" {
            reply(&mut workspace, "start", true, &[]);
            speech(&mut workspace, phase);
        }
        workspace.reload();
        assert!(sent(&mut workspace, "cancel"), "{phase}");
        assert!(
            workspace.problems().is_empty(),
            "{:?}",
            workspace.problems()
        );
        if phase == "opening" {
            reply(&mut workspace, "start", true, &[]);
            assert!(sent(&mut workspace, "cancel"));
        }
        reply(
            &mut workspace,
            "transcript",
            false,
            &[("cancelled", Data::Bool(true))],
        );
        speech(&mut workspace, "idle");
        workspace.send("feedback", "feedback", "open".into());
        button(&mut workspace, "start".into());
        assert!(sent(&mut workspace, "start"), "usable again after {phase}");
    }
}

#[test]
fn explicit_engine_readiness_overrides_the_default_in_both_directions() {
    let (_dir, mut workspace) = fixture(
        r#"
local dictation = use "dictation"
return mod { view = function()
    return dictation { id = "prompt", engine = "cloud" }
end }
"#,
    );
    for (default_ready, chosen_ready) in [(false, true), (true, false)] {
        workspace.set_host(
            "speech",
            map(&[
                ("available", Data::Bool(default_ready)),
                ("state", "idle".into()),
                (
                    "engines",
                    Data::List(vec![map(&[
                        ("name", "cloud".into()),
                        ("ready", Data::Bool(chosen_ready)),
                        ("reason", "Set the cloud key".into()),
                    ])]),
                ),
            ]),
        );
        let Some(Node::Stack(row)) = workspace.slot(BUTTON).and_then(crate::Slot::view) else {
            panic!("no mic button");
        };
        assert_eq!(
            row.pointer
                .as_ref()
                .and_then(|pointer| pointer.press.as_ref())
                .is_some(),
            chosen_ready
        );
    }
}

#[test]
fn releasing_a_hold_after_startup_still_sends_release() {
    let (_dir, mut workspace) = fixture(include_str!("../../../../app/mods/feedback/main.luau"));
    workspace.send("feedback", "feedback", "open".into());
    button(
        &mut workspace,
        map(&[
            ("press", "start".into()),
            ("phase", "down".into()),
            ("button", "left".into()),
        ]),
    );
    assert!(sent(&mut workspace, "start"));
    reply(&mut workspace, "start", true, &[]);
    speech(&mut workspace, "recording");
    button(
        &mut workspace,
        map(&[
            ("press", "stop".into()),
            ("phase", "up".into()),
            ("button", "left".into()),
        ]),
    );
    assert!(sent(&mut workspace, "release"));
}
