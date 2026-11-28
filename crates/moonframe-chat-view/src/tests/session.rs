//! The session around the conversation: handoffs, guests, questions, the
//! queue's states, agents to switch to, pull requests and activities.

use {
    super::*,
    herdr_chat_protocol::{
        Activity, ActivityKind, ActivityState, Agent, Guest, GuestPrompt, HandedOver, Harnesses,
        PullRequest, PullRequestAction, Question, QuestionOption, SessionInfo,
    },
};

fn session(id: &str) -> SessionInfo {
    SessionInfo {
        can_steer: false,
        continued: None,
        cwd: None,
        id: id.to_owned(),
        title: None,
    }
}

#[test]
fn a_handoff_is_a_divider_saying_what_was_handed_over() {
    let view = prepare(
        &transcript(vec![
            user("hi"),
            EntryKind::Handoff(HandedOver {
                from: Some(Agent::Claude),
                kept: 12,
                omitted: 3,
                resumed: false,
                to: Agent::Codex,
            }),
            EntryKind::Handoff(HandedOver {
                from: Some(Agent::Codex),
                kept: 1,
                omitted: 0,
                resumed: true,
                to: Agent::Claude,
            }),
        ]),
        &Limits::FULL,
    );
    let Row::Handoff { from, to, text, .. } = &view.rows[1] else {
        panic!("{:?}", view.rows[1])
    };
    assert_eq!((*from, *to), (Some("claude"), "codex"));
    assert_eq!(
        text,
        "Continued in Codex: 12 entries handed over, 3 left out"
    );
    let Row::Handoff { text, resumed, .. } = &view.rows[2] else {
        panic!()
    };
    assert!(resumed);
    assert_eq!(text, "Back to Claude: 1 entry handed over");
}

#[test]
fn guests_are_named_and_questions_carry_their_choices() {
    let mut chat = transcript(vec![EntryKind::User {
        text: "from outside".to_owned(),
        files: 0,
        note: None,
        guest: Some(Guest::new("Bob", "tma")),
    }]);
    let option = |id: &str| PermissionOption {
        id: id.to_owned(),
        kind: PermissionKind::Other,
        name: id.to_owned(),
    };
    chat.permissions.push(Permission {
        call_id: None,
        id: PermissionId(1),
        options: vec![option("Tabs"), option("Spaces")],
        title: "Indent with?".to_owned(),
        question: Some(Question {
            header: Some("Indent".to_owned()),
            multiple: true,
            options: vec![
                QuestionOption {
                    description: Some("Wider".to_owned()),
                    label: "Tabs".to_owned(),
                },
                QuestionOption {
                    description: None,
                    label: "Spaces".to_owned(),
                },
            ],
            text: "Indent with?".to_owned(),
        }),
        guest: None,
    });
    chat.permissions.push(Permission {
        call_id: None,
        id: PermissionId(2),
        options: vec![option("guest_allow"), option("guest_deny")],
        title: "Bob asks to send".to_owned(),
        question: None,
        guest: Some(Box::new(GuestPrompt {
            guest: Guest::new("Bob", "tma"),
            text: "Run the tests".to_owned(),
        })),
    });
    let view = prepare(&chat, &Limits::FULL);
    let Row::User { guest, .. } = &view.rows[0] else {
        panic!()
    };
    assert_eq!(guest.as_deref(), Some("Bob via tma"));
    let question = view.permissions[0].question.as_ref().unwrap();
    assert!(question.multiple);
    assert_eq!(question.header.as_deref(), Some("Indent"));
    assert_eq!(
        question
            .options
            .iter()
            .map(|option| option.label.as_str())
            .collect::<Vec<_>>(),
        ["Tabs", "Spaces"]
    );
    let guest = &view.permissions[1];
    assert_eq!(guest.guest.as_deref(), Some("Bob via tma"));
    assert_eq!(guest.guest_text.as_deref(), Some("Run the tests"));
}

#[test]
fn queued_prompts_say_whether_they_are_held_or_steered() {
    let mut chat = transcript(vec![]);
    for (held, steered) in [(true, false), (false, true)] {
        chat.queued.push(Queued {
            attachments: Vec::new(),
            guest: None,
            held,
            id: CommandId::random(),
            steered,
            text: "next".to_owned(),
        });
    }
    let mut info = session("s");
    info.can_steer = true;
    chat.session = Some(info);
    let view = prepare(&chat, &Limits::FULL);
    assert!(view.can_steer);
    assert_eq!(
        view.queue
            .iter()
            .map(|queued| (queued.held, queued.steered))
            .collect::<Vec<_>>(),
        [(true, false), (false, true)]
    );
}

#[test]
fn agents_to_switch_to_pull_requests_and_activities() {
    let mut chat = transcript(vec![user("hi")]);
    let view = prepare(&chat, &Limits::FULL);
    assert!(view.agents.is_empty(), "no switching offered");
    chat.harnesses = Some(Harnesses {
        available: vec![Agent::Claude, Agent::Codex],
        current: Some(Agent::Codex),
    });
    chat.pull_requests.push(
        PullRequest::new("moonframehq/moonframe", 7, vec![
            PullRequestAction::Created,
            PullRequestAction::Pushed,
        ])
        .unwrap(),
    );
    chat.activities.push(Activity {
        call_id: "a1".to_owned(),
        detail: None,
        finished_ms: None,
        kind: ActivityKind::Shell,
        label: "cargo test".to_owned(),
        started_ms: 5,
        state: ActivityState::Running,
        stoppable: true,
    });
    chat.clients = 2;
    let mut info = session("s");
    info.continued = Some(EntryId(1));
    chat.session = Some(info);
    let view = prepare(&chat, &Limits::FULL);
    assert_eq!(
        view.agents
            .iter()
            .map(|agent| (agent.id, agent.name, agent.current))
            .collect::<Vec<_>>(),
        [("claude", "Claude", false), ("codex", "Codex", true)]
    );
    let pull = &view.pull_requests[0];
    assert_eq!(pull.url, "https://github.com/moonframehq/moonframe/pull/7");
    assert_eq!(pull.actions, ["created", "pushed"]);
    assert_eq!(pull.state, Some("Pushed"));
    assert!(view.activities[0].stoppable);
    assert_eq!(view.clients, 2);
    assert_eq!(view.continued.as_deref(), Some(view.rows[0].key()));
}

#[test]
fn the_clock_says_how_long_the_turn_and_activities_have_run() {
    let mut chat = transcript(vec![user("hi")]);
    let view = prepare(&chat, &Limits::FULL);
    assert!(!ticking(&view), "an idle chat has no clock");
    chat.status = Status::Working;
    chat.turn = Some(Turn { started_ms: 1_000 });
    chat.activities.push(Activity {
        call_id: "a1".to_owned(),
        detail: None,
        finished_ms: None,
        kind: ActivityKind::Subagent,
        label: "Explore".to_owned(),
        started_ms: 0,
        state: ActivityState::Running,
        stoppable: false,
    });
    let mut view = prepare(&chat, &Limits::FULL);
    assert!(ticking(&view));
    clock(&mut view, 43_500);
    assert_eq!(view.working_for.as_deref(), Some("42s"));
    assert_eq!(view.activities[0].running_for.as_deref(), Some("43s"));
    clock(&mut view, 1_200);
    assert_eq!(view.working_for.as_deref(), Some("0s"));
}
