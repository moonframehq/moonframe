//! The pi provider end to end: the real pi and a mock model, in folders of
//! their own, so they need no account and touch none of your sessions. They
//! need pi, so `just test` skips them: `just test-pilot` runs them, as CI
//! does on Linux with a pinned pi (see ci.yml). The
//! scenarios of each test follow on from each other, as a session would.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use {
    serde_json::{Value, json},
    std::{fs, path::Path, thread, time::Duration},
    support::{DOT_PNG, Rig, count, git, has_item, items, start_rig},
};

fn events(value: &Value) -> Vec<Value> {
    value["events"].as_array().cloned().unwrap_or_default()
}

fn rows(value: &Value) -> Vec<Value> {
    value["threads"].as_array().cloned().unwrap_or_default()
}

fn commands(value: &Value) -> Vec<Value> {
    value["commands"].as_array().cloned().unwrap_or_default()
}

fn finds_pi_and_its_models(rig: &Rig) {
    let value = rig.app.value();
    assert_eq!(value["models"][0]["id"], "mock-1");
    let version = value["runtime"]["version"].as_str().unwrap();
    assert!(version.contains('.'), "{version}");
}

fn a_workspace_appears_with_its_branch(rig: &mut Rig) {
    rig.app.send(&json!({ "add_workspace": rig.project }));
    let workspace = rig.app.until("workspace", |v| {
        let list = v["workspaces"].as_array().unwrap();
        list.iter().find(|item| item["branch"] == "main").cloned()
    });
    assert_eq!(workspace["name"], "tidy-cart");
    assert_eq!(workspace["git"], true);
}

// What the composer really sends: the host turns an empty Luau table into `{}`.
fn an_empty_attachment_list_from_the_host_is_no_attachments(rig: &mut Rig) {
    rig.app
        .send(&json!({ "set_model": { "provider": "mock", "id": "mock-1" } }));
    rig.app.send(&json!({ "new_thread": {
        "workspace": rig.project, "mode": "local", "prompt": "hello there", "images": {},
    }}));
    rig.app.when("new thread sent", |v| {
        v["thread"]["status"] == "idle" && has_item(v, "user", |i| i["text"] == "hello there")
    });
    rig.app
        .send(&json!({ "prompt": "and again", "images": {} }));
    rig.app.when("prompt sent", |v| {
        v["thread"]["status"] == "idle" && has_item(v, "user", |i| i["text"] == "and again")
    });
    let selected = rig.app.value()["selected"].clone();
    rig.app.send(&json!({ "delete_thread": selected }));
    rig.app.when("cleared", |v| v["thread"].is_null());
}

fn a_thread_runs_pi_tools_an_answer_a_turn_s_changes_a_title(rig: &mut Rig) {
    rig.app
        .send(&json!({ "set_model": { "provider": "mock", "id": "mock-1" } }));
    rig.app.send(&json!({ "new_thread": {
        "workspace": rig.project, "mode": "local", "prompt": "fix the price please",
    }}));
    rig.app
        .when("running", |v| v["thread"]["status"] == "running");
    rig.app.when("finished", |v| {
        v["thread"]["status"] == "idle" && !items(v, "changes").is_empty()
    });
    let value = rig.app.value();
    let tools = items(&value, "tool");
    let names: Vec<&Value> = tools.iter().map(|tool| &tool["tool"]).collect();
    assert_eq!(names, [&json!("read"), &json!("edit"), &json!("bash")]);
    assert_eq!(tools[0]["title"], "Read src/price.js");
    assert_eq!(tools[1]["added"], 1);
    assert_eq!(tools[1]["deleted"], 1);
    assert!(tools.iter().all(|tool| tool["status"] == "done"));
    let answer = items(&value, "text").last().cloned().unwrap();
    assert!(answer["text"].as_str().unwrap().contains("Fixed"));
    assert_eq!(answer["answer"], true);
    let changes = &items(&value, "changes")[0];
    assert_eq!(changes["count"], 1);
    assert_eq!(changes["files"][0]["path"], "src/price.js");
    let summary = &items(&value, "run")[0]["summary"];
    assert_eq!(summary, "Explored 1 file, edited 1 file, ran 1 command");
    let price = fs::read_to_string(rig.project.join("src/price.js")).unwrap();
    assert!(price.contains("padStart"));
    rig.app.when("generated title", |v| {
        v["thread"]["title"] == "Fix The Price Please"
    });
    assert_eq!(events(&rig.app.value()).last().unwrap()["kind"], "finished");
}

fn session_files_are_pi_s_own_and_survive_a_fresh_load(rig: &mut Rig) {
    let value = rig.app.value();
    let id = value["selected"].clone();
    let before = items(&value, "tool").len();
    rig.app.send(&json!({ "select": false }));
    rig.app.when("deselected", |v| v["thread"].is_null());
    rig.app.send(&json!({ "select": id }));
    rig.app
        .when("reloaded", |v| items(v, "tool").len() == before);
    assert_eq!(fs::read_dir(rig.agent.join("sessions")).unwrap().count(), 1);
    let value = rig.app.value();
    let prompt = &items(&value, "user")[0];
    assert_eq!(prompt["text"], "fix the price please");
    assert!(
        prompt["entry"].is_string(),
        "a prompt has its entry id for forking"
    );
}

fn expanding_a_tool_row_brings_its_output(rig: &mut Rig) {
    let edit = items(&rig.app.value(), "tool")[1].clone();
    rig.app.send(&json!({ "expand": edit["id"] }));
    let open = rig.app.until("open", |v| {
        items(v, "tool")
            .into_iter()
            .find(|i| i["id"] == edit["id"] && i["open"] == true)
    });
    assert!(open["patch"].as_str().unwrap().contains("padStart"));
    rig.app.send(&json!({ "expand": edit["id"] }));
}

fn streaming(value: &Value) -> bool {
    value["thread"]["running"] == true && has_item(value, "text", |i| i["streaming"] == true)
}

fn queued(value: &Value, kind: &str) -> usize {
    value["thread"]["queued"][kind]
        .as_array()
        .map_or(0, Vec::len)
}

fn a_follow_up_queues_during_a_run_and_stop_stops(rig: &mut Rig) {
    rig.app.send(&json!({ "prompt": "go slow now" }));
    rig.app.when("streaming", streaming);
    rig.app
        .send(&json!({ "prompt": "and then this", "behavior": "follow_up" }));
    rig.app.when("queued", |v| queued(v, "followUp") == 1);
    rig.app.send(&json!({ "clear_queue": true }));
    rig.app.when("cleared", |v| queued(v, "followUp") == 0);
    rig.app.send(&json!({ "abort": true }));
    rig.app.when("stopped", |v| v["thread"]["running"] == false);
    rig.app
        .when("note", |v| has_item(v, "note", |i| i["text"] == "Stopped"));
}

fn a_shell_command_goes_through_pi(rig: &mut Rig) {
    rig.app
        .send(&json!({ "prompt": "!echo hello from a shell" }));
    let bash = rig
        .app
        .until("bash item", |v| items(v, "bash").first().cloned());
    assert!(
        bash["output"]
            .as_str()
            .unwrap()
            .contains("hello from a shell")
    );
    assert_eq!(bash["code"], 0);
}

fn forking_makes_a_new_thread_from_an_earlier_prompt(rig: &mut Rig) {
    let value = rig.app.value();
    let source = value["selected"].clone();
    let entry = items(&value, "user")[0]["entry"].clone();
    rig.app
        .send(&json!({ "fork": { "thread": source, "entry": entry } }));
    let forked = rig.app.until("forked", |v| {
        (v["selected"] != source && v["thread"].is_object()).then(|| v["thread"].clone())
    });
    assert!(
        forked["title"].as_str().unwrap().contains("Fork of"),
        "{forked}"
    );
    let listed = rows(&rig.app.value());
    assert_eq!(listed.iter().filter(|row| row["id"] == source).count(), 1);
    rig.app.send(&json!({ "select": source }));
    rig.app.when("back", |v| v["selected"] == source);
}

fn a_worktree_thread_runs_in_a_checkout_of_its_own(rig: &mut Rig) {
    rig.app.send(&json!({ "new_thread": {
        "workspace": rig.project, "mode": "worktree", "prompt": "hello there",
    }}));
    let thread = rig.app.until("worktree thread", |v| {
        (v["thread"]["worktree"] == true).then(|| v["thread"].clone())
    });
    assert!(
        thread["branch"].as_str().unwrap().starts_with("pilot/"),
        "{thread}"
    );
    assert_ne!(thread["cwd"], json!(rig.project));
    rig.app.when("answered", |v| {
        v["thread"]["status"] == "idle" && !items(v, "text").is_empty()
    });
    assert!(git(&rig.project, &["worktree", "list"]).contains("pilot/"));
}

fn a_thread_started_with_the_cli_shows_up(rig: &mut Rig) {
    let stamp = "2026-11-07T12:00:00.000Z";
    let session = json!({
        "type": "session", "version": 3, "id": "99999999-0000-4000-8000-000000000000",
        "timestamp": stamp, "cwd": rig.project,
    });
    let message = json!({
        "type": "message", "id": "aaaa0001", "parentId": null, "timestamp": stamp,
        "message": { "role": "user", "content": "from the cli", "timestamp": 1_762_550_000_000_i64 },
    });
    let sessions = rig.agent.join("sessions");
    let dir = fs::read_dir(&sessions)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().ends_with("tidy-cart--"))
        .unwrap();
    fs::write(dir.join("cli.jsonl"), format!("{session}\n{message}\n")).unwrap();
    rig.app
        .until_within("adopted", Duration::from_secs(8), |v| {
            rows(v)
                .iter()
                .find(|row| row["title"] == "from the cli")
                .cloned()
        });
}

fn pin_archive_and_delete(rig: &mut Rig) {
    let listed = rows(&rig.app.value());
    let id = listed
        .iter()
        .find(|row| row["title"] == "from the cli")
        .unwrap()["id"]
        .clone();
    let row = |v: &Value| rows(v).into_iter().find(|row| row["id"] == id);
    rig.app.send(&json!({ "pin": { "id": id, "on": true } }));
    rig.app.when("pinned", |v| {
        row(v).is_some_and(|row| row["pinned"] == true)
    });
    rig.app
        .send(&json!({ "archive": { "id": id, "on": true } }));
    rig.app.when("archived", |v| {
        row(v).is_some_and(|row| row["archived"] == true)
    });
    rig.app.send(&json!({ "delete_thread": id }));
    rig.app.when("deleted", |v| row(v).is_none());
}

fn renaming_reaches_pi(rig: &mut Rig) {
    let id = rig.app.value()["selected"].clone();
    rig.app
        .send(&json!({ "rename": { "id": id, "title": "A better name" } }));
    rig.app
        .until_within("renamed", Duration::from_secs(8), |v| {
            (v["thread"]["title"] == "A better name").then_some(())
        });
}

fn connected(value: &Value, id: &str) -> bool {
    let known = value["providers"]["known"].as_array();
    known.is_some_and(|known| {
        known
            .iter()
            .any(|item| item["id"] == id && item["connected"] == true)
    })
}

fn a_key_lands_in_pi_s_auth_json_private_and_comes_back_out(rig: &mut Rig) {
    rig.app
        .send(&json!({ "save_key": { "provider": "groq", "key": "gsk-test" } }));
    rig.app
        .until_within("connected", Duration::from_secs(5), |v| {
            connected(v, "groq").then_some(())
        });
    let auth: Value =
        serde_json::from_str(&fs::read_to_string(rig.agent.join("auth.json")).unwrap()).unwrap();
    assert_eq!(
        auth["groq"],
        json!({ "type": "api_key", "key": "gsk-test" })
    );
    assert!(
        !rig.app.value().to_string().contains("gsk-test"),
        "a key is never published"
    );
    rig.app.send(&json!({ "forget_key": "groq" }));
    rig.app
        .until_within("forgotten", Duration::from_secs(5), |v| {
            (!connected(v, "groq")).then_some(())
        });
}

fn a_schedule_runs_its_prompt_as_a_new_thread(rig: &mut Rig) {
    rig.app.send(&json!({ "schedule": {
        "name": "Nightly", "prompt": "hello nightly", "workspace": rig.project,
        "cadence": "daily", "time": "03:00",
    }}));
    let task = rig.app.until("saved", |v| {
        v["schedules"].as_array().unwrap().first().cloned()
    });
    assert_eq!(task["when"], "Every day at 03:00");
    assert!(task["next"].as_i64().unwrap() > rig.app.value()["now"].as_i64().unwrap());
    rig.app.send(&json!({ "run_schedule": task["id"] }));
    rig.app.when("ran", |v| {
        rows(v).iter().any(|row| {
            let title = row["title"].as_str().unwrap_or_default();
            title.contains("Nightly") || title.contains("hello nightly")
        })
    });
    rig.app
        .when("stamped", |v| v["schedules"][0]["last_run"].is_number());
}

fn has_command(value: &Value, name: &str) -> bool {
    commands(value)
        .iter()
        .any(|command| command["name"] == name)
}

fn turning_a_skill_off_reaches_pi(rig: &mut Rig) {
    let skill = rig.agent.join("skills/tidy-up");
    fs::create_dir_all(&skill).unwrap();
    let text = "---\nname: tidy-up\ndescription: Tidies things up.\n---\nTidy.\n";
    fs::write(skill.join("SKILL.md"), text).unwrap();
    rig.app.send(&json!({ "refresh": true }));
    let listed = rig.app.until("listed", |v| {
        let list = v["skills"].as_array().unwrap();
        list.iter().find(|item| item["name"] == "tidy-up").cloned()
    });
    assert_eq!(listed["enabled"], true);
    rig.app
        .send(&json!({ "set_skill": { "id": listed["id"], "enabled": true } }));
    rig.app
        .when("a command while on", |v| has_command(v, "skill:tidy-up"));
    rig.app
        .send(&json!({ "set_skill": { "id": listed["id"], "enabled": false } }));
    rig.app.when("off", |v| {
        let list = v["skills"].as_array().unwrap();
        list.iter()
            .any(|item| item["name"] == "tidy-up" && item["enabled"] == false)
    });
    rig.app
        .when("no command when off", |v| !has_command(v, "skill:tidy-up"));
    rig.app
        .send(&json!({ "set_skill": { "id": listed["id"], "enabled": true } }));
}

fn steering_goes_in_during_a_run(rig: &mut Rig) {
    rig.app.send(&json!({ "prompt": "go slow again" }));
    rig.app.when("streaming", streaming);
    rig.app
        .send(&json!({ "prompt": "actually stop soon", "behavior": "steer" }));
    rig.app.when("steering", |v| queued(v, "steering") == 1);
    rig.app.send(&json!({ "clear_queue": true }));
    rig.app.send(&json!({ "abort": true }));
    rig.app.when("stopped", |v| v["thread"]["running"] == false);
}

fn a_picture_goes_with_a_prompt(rig: &mut Rig) {
    let png = rig.root.join("dot.png");
    fs::write(&png, DOT_PNG).unwrap();
    rig.app
        .send(&json!({ "prompt": "what is this", "images": [png] }));
    rig.app.when("attached", |v| {
        has_item(v, "user", |i| {
            i["images"] == 1 && i["text"] == "what is this"
        })
    });
    rig.app
        .when("answered", |v| v["thread"]["running"] == false);
}

fn something_not_a_picture_is_refused_with_a_reason(rig: &mut Rig) {
    let seen = events(&rig.app.value())
        .last()
        .map_or(0, |event| event["seq"].as_i64().unwrap());
    let path = rig.project.join("package.json");
    rig.app
        .send(&json!({ "prompt": "and this", "images": [path] }));
    let toast = rig.app.until("toast", |v| {
        let later = |event: &&Value| event["seq"].as_i64().unwrap() > seen;
        events(v)
            .iter()
            .filter(later)
            .find(|event| event["kind"] == "toast")
            .cloned()
    });
    assert!(
        toast["body"].as_str().unwrap().contains("not one"),
        "{toast}"
    );
}

fn the_start_page_s_choices_are_for_the_next_thread_not_the_selected_one(rig: &mut Rig) {
    let before = rig.app.value()["thread"]["thinking"].clone();
    let level = if before == "high" {
        "low"
    } else {
        "high"
    };
    rig.app
        .send(&json!({ "set_thinking": level, "draft": true }));
    rig.app
        .when("remembered", |v| v["prefs"]["last_thinking"] == level);
    assert_eq!(rig.app.value()["thread"]["thinking"], before);
}

fn an_extension_s_question_waits_on_the_user_and_the_answer_reaches_it(rig: &mut Rig) {
    fs::create_dir_all(rig.agent.join("extensions")).unwrap();
    let extension = r#"export default function (pi) {
	pi.registerCommand("ask", {
		description: "Asks something",
		handler: async (_args, ctx) => {
			const answer = await ctx.ui.select("Pick a colour", ["Red", "Blue"]);
			ctx.ui.notify("You picked " + answer, "info");
			ctx.ui.setStatus("ask", "picked " + answer);
		},
	});
}
"#;
    fs::write(rig.agent.join("extensions/ask.ts"), extension).unwrap();
    rig.app.send(&json!({ "prompt": "/reload" }));
    rig.app.when("extension loaded", |v| has_command(v, "ask"));
    rig.app.send(&json!({ "prompt": "/ask" }));
    let dialog = rig.app.until("dialog", |v| {
        v["thread"]["dialog"]
            .is_object()
            .then(|| v["thread"]["dialog"].clone())
    });
    assert_eq!(dialog["title"], "Pick a colour");
    assert_eq!(dialog["options"], json!(["Red", "Blue"]));
    assert_eq!(rig.app.value()["thread"]["status"], "waiting");
    rig.app
        .send(&json!({ "answer": { "dialog": dialog["id"], "value": "Blue" } }));
    rig.app.when("answered", |v| {
        events(v)
            .iter()
            .any(|event| event["body"] == "You picked Blue")
    });
    rig.app.when("status", |v| {
        v["thread"]["statuses"]
            .as_array()
            .unwrap()
            .contains(&json!("picked Blue"))
    });
    assert!(rig.app.value()["thread"]["dialog"].is_null());
}

fn what_is_published_stays_under_the_host_s_limits(rig: &Rig) {
    let (bytes, values) = rig.app.largest();
    assert!(bytes < 1024 * 1024, "{bytes} bytes");
    assert!(values < 10_000, "{values} values");
    assert!(count(&rig.app.value()) < 10_000);
}

#[test]
#[ignore = "needs pi: just test-pilot"]
fn a_session_with_pi() {
    let dir = tempfile::tempdir().unwrap();
    let mut rig = start_rig(dir.path());
    finds_pi_and_its_models(&rig);
    a_workspace_appears_with_its_branch(&mut rig);
    an_empty_attachment_list_from_the_host_is_no_attachments(&mut rig);
    a_thread_runs_pi_tools_an_answer_a_turn_s_changes_a_title(&mut rig);
    session_files_are_pi_s_own_and_survive_a_fresh_load(&mut rig);
    expanding_a_tool_row_brings_its_output(&mut rig);
    a_follow_up_queues_during_a_run_and_stop_stops(&mut rig);
    a_shell_command_goes_through_pi(&mut rig);
    forking_makes_a_new_thread_from_an_earlier_prompt(&mut rig);
    a_worktree_thread_runs_in_a_checkout_of_its_own(&mut rig);
    a_thread_started_with_the_cli_shows_up(&mut rig);
    pin_archive_and_delete(&mut rig);
    renaming_reaches_pi(&mut rig);
    a_key_lands_in_pi_s_auth_json_private_and_comes_back_out(&mut rig);
    a_schedule_runs_its_prompt_as_a_new_thread(&mut rig);
    turning_a_skill_off_reaches_pi(&mut rig);
    steering_goes_in_during_a_run(&mut rig);
    a_picture_goes_with_a_prompt(&mut rig);
    something_not_a_picture_is_refused_with_a_reason(&mut rig);
    the_start_page_s_choices_are_for_the_next_thread_not_the_selected_one(&mut rig);
    an_extension_s_question_waits_on_the_user_and_the_answer_reaches_it(&mut rig);
    what_is_published_stays_under_the_host_s_limits(&rig);
}

// Threads, from threads.test.mjs: a project, and what can go wrong in one.

fn a_new_thread_with_only_a_picture_sends_it(rig: &mut Rig) {
    let png = rig.root.join("dot.png");
    fs::write(&png, DOT_PNG).unwrap();
    rig.app.send(&json!({ "new_thread": {
        "workspace": rig.project, "mode": "local", "prompt": "", "images": [png],
    }}));
    rig.app.when("picture sent", |v| {
        v["thread"]["status"] == "idle" && has_item(v, "user", |i| i["images"] == 1)
    });
}

fn a_new_thread_s_command_runs_in_the_shell_not_as_a_prompt(rig: &mut Rig) {
    rig.app.send(&json!({ "new_thread": {
        "workspace": rig.project, "mode": "local", "prompt": "!echo from the start page",
        "images": {},
    }}));
    let bash = rig.app.until("ran", |v| items(v, "bash").first().cloned());
    assert!(
        bash["output"]
            .as_str()
            .unwrap()
            .contains("from the start page")
    );
    assert!(items(&rig.app.value(), "user").is_empty());
    rig.app
        .send(&json!({ "prompt": "hello there", "images": {} }));
    rig.app.when("answered", |v| {
        v["thread"]["status"] == "idle" && !items(v, "text").is_empty()
    });
    let shells = items(&rig.app.value(), "bash").len();
    assert_eq!(shells, 1, "the file takes over, without a copy");
}

fn a_thread_with_only_shell_output_stays_listed_after_leaving_it(rig: &mut Rig) {
    let first = rig.app.value()["selected"].clone();
    rig.app.send(&json!({ "new_thread": {
        "workspace": rig.project, "mode": "local", "prompt": "!echo only a shell", "images": {},
    }}));
    let shell_only = rig.app.until("ran", |v| {
        (v["selected"] != first && !items(v, "bash").is_empty()).then(|| v["selected"].clone())
    });
    rig.app.send(&json!({ "select": first }));
    rig.app.when("left", |v| v["selected"] == first);
    let listed = rows(&rig.app.value());
    assert!(
        listed.iter().any(|row| row["id"] == shell_only),
        "still in the list"
    );
    rig.app.send(&json!({ "select": shell_only }));
    rig.app.when("and it comes back", |v| {
        v["selected"] == shell_only && !items(v, "bash").is_empty()
    });
}

fn a_failed_send_s_words_wait_for_their_own_thread_and_come_back_once(rig: &mut Rig) {
    let other = rig.app.value()["selected"].clone();
    rig.app.send(&json!({ "new_thread": {
        "workspace": rig.project, "mode": "local", "prompt": "hello there", "images": {},
    }}));
    let failing = rig.app.until("a second thread", |v| {
        let entered = has_item(v, "user", |i| i["entry"].is_string());
        (v["selected"] != other && v["thread"]["status"] == "idle" && entered)
            .then(|| v["selected"].clone())
    });
    rig.app.send(&json!({ "select": other }));
    rig.app.when("moved away", |v| v["selected"] == other);

    let wrong = rig.project.join("package.json");
    rig.app
        .send(&json!({ "thread": failing, "prompt": "keep these words", "images": [wrong] }));
    rig.app.when("refused", |v| {
        events(v).iter().any(|e| {
            e["body"]
                .as_str()
                .unwrap_or_default()
                .contains("is not one")
        })
    });
    assert!(
        rig.app.value()["returned"].is_null(),
        "not offered to the open thread"
    );

    rig.app.send(&json!({ "select": failing }));
    let id = rig.app.until("back on its thread", |v| {
        (v["selected"] == failing && v["returned"]["text"] == "keep these words")
            .then(|| v["returned"]["id"].clone())
    });
    rig.app
        .send(&json!({ "returned_taken": { "thread": failing, "id": "an older one" } }));
    rig.app.send(&json!({ "select": other }));
    rig.app.when("away again", |v| v["selected"] == other);
    rig.app.send(&json!({ "select": failing }));
    rig.app.when("an old answer takes nothing", |v| {
        v["selected"] == failing && v["returned"]["id"] == id
    });

    rig.app
        .send(&json!({ "returned_taken": { "thread": failing, "id": id } }));
    rig.app.when("taken once", |v| v["returned"].is_null());

    // The mods keep the last id across launches, and a provider starts over.
    rig.app
        .send(&json!({ "prompt": "keep these words", "images": [wrong] }));
    let again = rig.app.until("failed again", |v| {
        v["returned"].is_object().then(|| v["returned"].clone())
    });
    assert_eq!(again["text"], "keep these words");
    assert_ne!(again["id"], id, "same words, a new id");
}

fn a_worktree_outlives_its_first_thread_while_a_fork_uses_it(rig: &mut Rig) {
    rig.app.send(&json!({ "new_thread": {
        "workspace": rig.project, "mode": "worktree", "prompt": "hello there", "images": {},
    }}));
    let original = rig.app.until("worktree thread", |v| {
        let entered = has_item(v, "user", |i| i["entry"].is_string());
        (v["thread"]["worktree"] == true && v["thread"]["status"] == "idle" && entered)
            .then(|| v["thread"].clone())
    });
    let list = original["items"].as_array().unwrap();
    let entry = list.iter().find(|i| i["entry"].is_string()).unwrap()["entry"].clone();
    rig.app
        .send(&json!({ "fork": { "thread": original["id"], "entry": entry } }));
    let fork = rig.app.until("forked", |v| {
        (v["selected"] != original["id"] && v["thread"].is_object()).then(|| v["thread"].clone())
    });
    assert_eq!(fork["cwd"], original["cwd"]);
    let cwd = original["cwd"].as_str().unwrap();

    rig.app.send(&json!({ "delete_thread": original["id"] }));
    rig.app.when("original gone", |v| {
        !rows(v).iter().any(|row| row["id"] == original["id"])
    });
    assert!(Path::new(cwd).exists(), "the fork still has its checkout");

    rig.app.send(&json!({ "delete_thread": fork["id"] }));
    rig.app.when("fork gone", |v| {
        !rows(v).iter().any(|row| row["id"] == fork["id"])
    });
    for _ in 0..500 {
        if !Path::new(cwd).exists() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("the last thread removes the checkout");
}

#[test]
#[ignore = "needs pi: just test-pilot"]
fn threads_in_a_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut rig = start_rig(dir.path());
    rig.app
        .send(&json!({ "set_model": { "provider": "mock", "id": "mock-1" } }));
    rig.app.send(&json!({ "add_workspace": rig.project }));
    rig.app.when("project", |v| {
        v["workspaces"].as_array().unwrap().len() == 1
    });
    a_new_thread_with_only_a_picture_sends_it(&mut rig);
    a_new_thread_s_command_runs_in_the_shell_not_as_a_prompt(&mut rig);
    a_thread_with_only_shell_output_stays_listed_after_leaving_it(&mut rig);
    a_failed_send_s_words_wait_for_their_own_thread_and_come_back_once(&mut rig);
    a_worktree_outlives_its_first_thread_while_a_fork_uses_it(&mut rig);
}

// Chats, from chats.test.mjs: a thread needs no project, so nothing here adds one.

fn finished(value: &Value, text: &str) -> bool {
    value["thread"]["status"] == "idle" && has_item(value, "user", |i| i["text"] == text)
}

fn with_no_project_a_thread_runs_in_the_app_s_own_folder(rig: &mut Rig) {
    let value = rig.app.value();
    assert!(value["workspaces"].as_array().unwrap().is_empty());
    let chats = &value["chats"];
    assert_eq!(chats["name"], "No project");
    assert_eq!(chats["git"], false);
    let path = chats["path"].as_str().unwrap().to_owned();
    assert!(Path::new(&path).exists());

    rig.app.send(&json!({ "new_thread": {
        "workspace": path, "mode": "local", "prompt": "just a question", "images": {},
    }}));
    rig.app
        .when("chat answered", |v| finished(v, "just a question"));
    let value = rig.app.value();
    let thread = &value["thread"];
    assert_eq!(thread["cwd"], json!(path));
    assert_eq!(thread["workspace"], "No project");
    assert_eq!(
        thread["git"], false,
        "a chat offers no fork into a worktree"
    );
    let row = rows(&value)
        .into_iter()
        .find(|row| row["id"] == value["selected"])
        .unwrap();
    assert_eq!(row["workspace"], "No project");
    let sessions = fs::read_dir(rig.agent.join("sessions")).unwrap().count();
    assert_eq!(sessions, 1, "pi keeps its session as usual");
}

fn a_thread_that_names_no_folder_at_all_also_works(rig: &mut Rig) {
    rig.app.send(&json!({ "new_thread": {
        "mode": "local", "prompt": "no folder named", "images": {},
    }}));
    rig.app.when("answered", |v| finished(v, "no folder named"));
    let value = rig.app.value();
    assert_eq!(value["thread"]["cwd"], value["chats"]["path"]);
}

fn a_worktree_choice_left_over_from_a_project_is_ignored(rig: &mut Rig) {
    let chats = rig.app.value()["chats"]["path"].clone();
    rig.app.send(&json!({ "new_thread": {
        "workspace": chats, "mode": "worktree", "prompt": "stale mode", "images": {},
    }}));
    rig.app.when("answered", |v| finished(v, "stale mode"));
    assert_eq!(rig.app.value()["thread"]["cwd"], chats);
}

fn a_scheduled_task_can_have_no_project(rig: &mut Rig) {
    let value = rig.app.value();
    let before = rows(&value).len();
    rig.app.send(&json!({ "schedule": {
        "name": "daily note", "prompt": "write a note", "workspace": value["chats"]["path"],
        "worktree": true,
    }}));
    rig.app
        .when("saved", |v| v["schedules"].as_array().unwrap().len() == 1);
    let value = rig.app.value();
    assert_eq!(value["schedules"][0]["workspace_name"], "No project");
    rig.app
        .send(&json!({ "run_schedule": value["schedules"][0]["id"] }));
    rig.app.when("ran", |v| rows(v).len() > before);
}

#[test]
#[ignore = "needs pi: just test-pilot"]
fn chats_with_no_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut rig = start_rig(dir.path());
    rig.app
        .send(&json!({ "set_model": { "provider": "mock", "id": "mock-1" } }));
    with_no_project_a_thread_runs_in_the_app_s_own_folder(&mut rig);
    a_thread_that_names_no_folder_at_all_also_works(&mut rig);
    a_worktree_choice_left_over_from_a_project_is_ignored(&mut rig);
    a_scheduled_task_can_have_no_project(&mut rig);
}
