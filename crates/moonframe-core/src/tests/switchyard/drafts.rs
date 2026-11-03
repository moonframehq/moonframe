use super::*;

fn started(app: &mut App, request: &Data, id: &str) {
    app.workspace.send(
        "provider:chat",
        "chat",
        Data::Map(
            [
                ("reply".into(), field(request, "id").clone()),
                ("action".into(), Data::from("start")),
                ("ok".into(), Data::Bool(true)),
                ("chat".into(), Data::from(id)),
            ]
            .into(),
        ),
    );
    app.check();
}

#[test]
fn a_new_chat_leaves_the_existing_chats_draft_and_completions_there() {
    let mut app = App::new();
    app.start();
    app.live("{}");
    app.send("chat", "return {msg='draft',value='!cat src/'}");
    app.send("chat", "return {pick_new='test'}");
    let request = app.outbox("chat", "start").remove(0);
    started(&mut app, &request, "two");
    assert_eq!(field(&app.state("chat"), "draft"), &Data::from(""));
    assert_eq!(
        field(&app.outbox("projects", "complete")[0], "complete"),
        &Data::from("")
    );
    app.send("chat", "return {msg='draft',value='for two'}");
    app.send("chat", "return {select_chat='one'}");
    assert_eq!(field(&app.state("chat"), "draft"), &Data::from("!cat src/"));
    assert_eq!(
        field(&app.outbox("projects", "complete")[0], "complete"),
        &Data::from("src")
    );
    app.send("chat", "return {select_chat='two'}");
    assert_eq!(field(&app.state("chat"), "draft"), &Data::from("for two"));
}

#[test]
fn the_first_chat_inherits_its_workspace_draft_even_if_started_while_away() {
    for away in [false, true] {
        let mut app = App::new();
        app.send("chat", "return {msg='draft',value='for the first chat'}");
        app.send("chat", "return {pick_new='test'}");
        let request = app.outbox("chat", "start").remove(0);
        if away {
            app.select("/other");
            app.send("chat", "return {msg='draft',value='elsewhere'}");
        }
        started(&mut app, &request, "one");
        if away {
            assert_eq!(field(&app.state("chat"), "draft"), &Data::from("elsewhere"));
            app.select("/work");
        }
        assert_eq!(
            field(&app.state("chat"), "draft"),
            &Data::from("for the first chat")
        );
    }
}

#[test]
fn reload_restores_waiting_messages_in_order_with_their_outputs_once() {
    let mut app = App::new();
    app.start();
    app.send("chat", "return {add={id='r1',command='ls',text='a b'}}");
    app.send("chat", "return {msg='send',value='first'}");
    app.send("chat", "return {msg='send',value='second'}");
    assert!(app.outbox("chat", "prompt").is_empty());
    app.send("chat", "return {msg='draft',value='typed since'}");
    app.select("/other");
    app.send("chat", "return {msg='draft',value='elsewhere'}");
    app.workspace.reload();
    app.check();
    app.live("{}");
    assert!(
        app.outbox("chat", "prompt").is_empty(),
        "recovered text must not be sent automatically"
    );
    assert_eq!(field(&app.state("chat"), "draft"), &Data::from("elsewhere"));
    app.select("/work");
    assert_eq!(
        field(&app.state("chat"), "draft"),
        &Data::from("first\n\nsecond\n\ntyped since")
    );
    let state = app.state("chat");
    let Data::List(chips) = field(field(&state, "context"), "one") else {
        panic!("missing recovered output")
    };
    assert_eq!(field(&chips[0], "id"), &Data::from("r1"));
    app.workspace.reload();
    app.check();
    assert_eq!(
        field(&app.state("chat"), "draft"),
        &Data::from("first\n\nsecond\n\ntyped since")
    );
}

#[test]
fn reload_recovers_unacknowledged_starts_prompts_and_shell_commands() {
    for (action, text) in [
        ("start", "first prompt"),
        ("prompt", "attached prompt"),
        ("run", "!cat src/main.rs"),
    ] {
        let mut app = App::new();
        if action != "start" {
            app.start();
            app.live("{}");
        }
        if action == "prompt" {
            app.send("chat", "return {add={id='r1',command='ls',text='a b'}}");
        }
        app.send("chat", &format!("return {{msg='send',value={text:?}}}"));
        let provider = if action == "run" {
            "shell"
        } else {
            "chat"
        };
        let request = app.outbox(provider, action).remove(0);
        app.workspace.reload();
        app.check();
        assert_eq!(field(&app.state("chat"), "draft"), &Data::from(text));
        assert!(app.outbox(provider, action).is_empty());
        // A late rejection after reload must not restore a second copy.
        reject(&mut app, &request, action);
        assert_eq!(field(&app.state("chat"), "draft"), &Data::from(text));
        if action == "prompt" {
            let state = app.state("chat");
            let Data::List(chips) = field(field(&state, "context"), "one") else {
                panic!("missing recovered output")
            };
            assert_eq!(field(&chips[0], "text"), &Data::from("a b"));
        }
    }
}
