use {
    super::*,
    herdr_chat_protocol::{CommandId, PermissionId},
};

#[test]
fn messages_become_herdr_commands() {
    let prompt = command(
        "prompt",
        &json!({ "prompt": "hi", "files": ["/a/b.png"] }),
        None,
    )
    .unwrap();
    assert_eq!(prompt, Command::Prompt {
        text: "hi".to_owned(),
        attachments: vec![Attachment {
            path: "/a/b.png".to_owned()
        }],
        origin: Some(Origin::new(Device::Desktop, Via::Local)),
    });
    assert!(command("prompt", &json!({ "prompt": "  " }), None).is_err());
    assert_eq!(
        command(
            "setting",
            &json!({ "setting": "model", "value": "opus" }),
            None
        )
        .unwrap(),
        Command::SetOption {
            id: "model".to_owned(),
            value: OptionValue::Choice {
                id: "opus".to_owned()
            },
        }
    );
    assert_eq!(
        command(
            "answer",
            &json!({ "answer": "c", "permission": 3, "option": "allow" }),
            None
        )
        .unwrap(),
        Command::Answer {
            option: "allow".to_owned(),
            permission: PermissionId(3)
        }
    );
}

#[test]
fn a_question_is_answered_with_labels_or_the_users_words() {
    let message = json!({
        "answer": "c", "permission": 4, "answers": ["Tabs", "my own words"],
    });
    assert_eq!(
        command("answer", &message, None).unwrap(),
        Command::AnswerQuestion {
            answers: vec!["Tabs".to_owned(), "my own words".to_owned()],
            permission: PermissionId(4),
        }
    );
    let wrong = json!({ "answer": "c", "permission": 4, "answers": [1] });
    assert!(command("answer", &wrong, None).is_err());
}

#[test]
fn queued_prompts_go_now_activities_stop_and_agents_switch() {
    let id = CommandId::random();
    let queued = serde_json::to_value(id).unwrap();
    assert_eq!(
        command(
            "send_now",
            &json!({ "send_now": queued, "chat": "c" }),
            None
        )
        .unwrap(),
        Command::SendNow { id }
    );
    assert!(command("send_now", &json!({ "send_now": "nope" }), None).is_err());
    assert_eq!(
        command(
            "stop_activity",
            &json!({ "stop_activity": "call-1", "chat": "c" }),
            None
        )
        .unwrap(),
        Command::StopActivity {
            call_id: "call-1".to_owned()
        }
    );
    assert!(command("stop_activity", &json!({ "stop_activity": "" }), None).is_err());
    assert_eq!(
        command("agent", &json!({ "agent": "codex", "chat": "c" }), None).unwrap(),
        Command::SetAgent {
            agent: Agent::Codex
        }
    );
    assert!(command("agent", &json!({ "agent": "nobody" }), None).is_err());
}

#[test]
fn the_chat_is_named_by_cancel_and_answer_or_else_by_chat() {
    assert_eq!(chat_of("cancel", &json!({ "cancel": "a" })), Some("a"));
    assert_eq!(
        chat_of("answer", &json!({ "answer": "b", "permission": 1 })),
        Some("b")
    );
    assert_eq!(
        chat_of("send_now", &json!({ "send_now": "q", "chat": "c" })),
        Some("c")
    );
}
