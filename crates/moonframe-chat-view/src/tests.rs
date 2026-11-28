use {
    super::*,
    herdr_chat_protocol::{
        Attachment, Choice, CommandId, ConfigOption, Cost, Entry, EntryId, EntryKind, Mode, Modes,
        OptionCategory, OptionKind, Permission, PermissionId, PermissionKind, PermissionOption,
        Queued, Tokens, Tool, ToolContent, ToolKind, ToolStatus, Turn, TurnEnd, TurnSummary, Usage,
    },
};

fn transcript(kinds: Vec<EntryKind>) -> Transcript {
    let mut transcript = Transcript::new("Claude");
    transcript.status = Status::Idle;
    transcript.entries = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| Entry {
            id: EntryId(index as u64 + 1),
            kind,
        })
        .collect();
    transcript
}

fn user(text: &str) -> EntryKind {
    EntryKind::User {
        text: text.to_owned(),
        files: 0,
    }
}

fn agent(text: &str) -> EntryKind {
    EntryKind::Agent {
        text: text.to_owned(),
    }
}

fn thought(text: &str) -> EntryKind {
    EntryKind::Thought {
        text: text.to_owned(),
    }
}

fn tool(call: &str, kind: ToolKind, title: &str, status: ToolStatus) -> Tool {
    Tool {
        call_id: call.to_owned(),
        content: Vec::new(),
        kind,
        locations: Vec::new(),
        parent: None,
        started_ms: None,
        status,
        title: title.to_owned(),
    }
}

fn call(call: &str, kind: ToolKind, title: &str) -> EntryKind {
    EntryKind::Tool(tool(call, kind, title, ToolStatus::Completed))
}

fn turn(ms: u64, end: TurnEnd, total: Option<u64>) -> EntryKind {
    EntryKind::Turn(TurnSummary {
        duration_ms: ms,
        ended_ms: None,
        end,
        tokens: total.map(|total| Tokens {
            total,
            ..Tokens::default()
        }),
    })
}

fn group(row: &Row) -> &ToolGroup {
    match row {
        Row::Tools(group) => group,
        other => panic!("not a group: {other:?}"),
    }
}

#[test]
fn a_run_of_calls_and_thoughts_folds_into_one_group_between_words() {
    let view = prepare(
        &transcript(vec![
            user("Fix the build"),
            thought("**Looking** at the error"),
            call("a", ToolKind::Read, "Read Cargo.toml"),
            call("b", ToolKind::Read, "Read src/lib.rs"),
            call("c", ToolKind::Execute, "cargo build"),
            agent("Fixed it."),
            turn(72_000, TurnEnd::Done, Some(12_400)),
        ]),
        &Limits::FULL,
    );
    let types: Vec<&str> = view.rows.iter().map(row_type).collect();
    assert_eq!(types, ["user", "tools", "answer", "turn"]);
    let tools = group(&view.rows[1]);
    assert_eq!(tools.key, "e2");
    assert_eq!(tools.title, "Read 2 files, ran 1 command");
    assert_eq!(tools.status, "done");
    assert_eq!(tools.count, 3);
    assert!(!tools.live);
    assert_eq!(tools.items.len(), 4);
    assert!(
        matches!(&tools.items[0], ToolItem::Thought { gist, .. } if gist == "Looking at the error")
    );
    let Row::Turn { text, .. } = &view.rows[3] else {
        panic!()
    };
    assert_eq!(text, "Worked 1m 12s · 12.4k tokens");
    assert_eq!(view.last.as_deref(), Some("Fixed it."));
}

fn row_type(row: &Row) -> &'static str {
    match row {
        Row::User { .. } => "user",
        Row::Answer { .. } => "answer",
        Row::Thought { .. } => "thought",
        Row::Tools(_) => "tools",
        Row::Image { .. } => "image",
        Row::Notice { .. } => "notice",
        Row::Turn { .. } => "turn",
    }
}

#[test]
fn one_call_takes_its_own_title_and_lone_thoughts_stay_rows() {
    let view = prepare(
        &transcript(vec![
            user("hi"),
            thought("Thinking it over"),
            agent("Hello"),
            call("a", ToolKind::Search, "grep -r foo"),
        ]),
        &Limits::FULL,
    );
    let types: Vec<&str> = view.rows.iter().map(row_type).collect();
    assert_eq!(types, ["user", "thought", "answer", "tools"]);
    assert_eq!(group(&view.rows[3]).title, "grep -r foo");
}

#[test]
fn the_running_group_is_live_and_the_last_answer_streams() {
    let mut running = transcript(vec![
        user("go"),
        EntryKind::Tool(tool(
            "a",
            ToolKind::Execute,
            "npm test",
            ToolStatus::InProgress,
        )),
    ]);
    running.status = Status::Working;
    running.turn = Some(Turn { started_ms: 1000 });
    let view = prepare(&running, &Limits::FULL);
    let tools = group(&view.rows[1]);
    assert!(tools.live);
    assert_eq!(tools.status, "running");
    assert_eq!(tools.current.as_deref(), Some("npm test"));
    assert_eq!(view.since_ms, Some(1000));
    assert_eq!(view.status, "working");

    running.entries.push(Entry {
        id: EntryId(3),
        kind: agent("All green"),
    });
    let view = prepare(&running, &Limits::FULL);
    assert!(!group(&view.rows[1]).live, "words came after it");
    assert!(matches!(view.rows[2], Row::Answer {
        streaming: true,
        ..
    }));
}

#[test]
fn a_sub_agents_steps_count_in_its_call_and_are_not_rows() {
    let mut step = tool("s1", ToolKind::Read, "Read a.rs", ToolStatus::InProgress);
    step.parent = Some("task".to_owned());
    let done = Tool {
        parent: Some("task".to_owned()),
        ..tool("s0", ToolKind::Read, "Read b.rs", ToolStatus::Completed)
    };
    let view = prepare(
        &transcript(vec![
            user("explore"),
            EntryKind::Tool(tool(
                "task",
                ToolKind::Subagent,
                "Explore the repo",
                ToolStatus::InProgress,
            )),
            EntryKind::Tool(done),
            agent("between"),
            EntryKind::Tool(step),
        ]),
        &Limits::FULL,
    );
    let types: Vec<&str> = view.rows.iter().map(row_type).collect();
    assert_eq!(types, ["user", "tools", "answer"], "steps break no run");
    let ToolItem::Tool(task) = &group(&view.rows[1]).items[0] else {
        panic!()
    };
    assert_eq!(task.steps, 2);
    assert_eq!(task.detail.as_deref(), Some("Read a.rs"));
}

#[test]
fn a_prompt_hides_its_context_frame_and_lists_its_files() {
    let framed = frame("The open message: Re: invoice", "Summarize it");
    let mut text = framed.clone();
    text.push_str("\n\u{1f4ce} /tmp/a.pdf\n\u{1f4ce} /tmp/b.png");
    let view = prepare(
        &transcript(vec![EntryKind::User { text, files: 2 }]),
        &Limits::FULL,
    );
    let Row::User {
        text,
        files,
        context,
        ..
    } = &view.rows[0]
    else {
        panic!()
    };
    assert_eq!(text, "Summarize it");
    assert_eq!(files, &["/tmp/a.pdf", "/tmp/b.png"]);
    assert!(context);
    assert_eq!(
        unframe(&framed),
        (Some("The open message: Re: invoice"), "Summarize it")
    );
    assert_eq!(frame("  ", "plain"), "plain");
    assert_eq!(
        unframe("<app-context>\nno end"),
        (None, "<app-context>\nno end")
    );
}

#[test]
fn permissions_carry_the_agents_options_and_the_calls_title() {
    let mut chat = transcript(vec![EntryKind::Tool(tool(
        "x",
        ToolKind::Execute,
        "rm -rf build",
        ToolStatus::Pending,
    ))]);
    chat.permissions.push(Permission {
        call_id: Some("x".to_owned()),
        id: PermissionId(7),
        options: vec![
            PermissionOption {
                id: "yes".to_owned(),
                kind: PermissionKind::AllowOnce,
                name: "Allow".to_owned(),
            },
            PermissionOption {
                id: "no".to_owned(),
                kind: PermissionKind::RejectOnce,
                name: "Reject".to_owned(),
            },
        ],
        title: "Run a command?".to_owned(),
    });
    let view = prepare(&chat, &Limits::FULL);
    let permission = &view.permissions[0];
    assert_eq!(permission.id, 7);
    assert_eq!(permission.tool.as_deref(), Some("rm -rf build"));
    assert_eq!(
        permission
            .options
            .iter()
            .map(|option| (option.id.as_str(), option.kind, option.allows))
            .collect::<Vec<_>>(),
        [("yes", "allow_once", true), ("no", "reject_once", false)]
    );
}

#[test]
fn the_queue_keeps_ids_to_unqueue_without_context_frames() {
    let mut chat = transcript(vec![]);
    let id = CommandId::random();
    chat.queued.push(Queued {
        attachments: vec![Attachment {
            path: "/tmp/notes.md".to_owned(),
        }],
        id,
        text: frame("context", "and then this"),
    });
    let view = prepare(&chat, &Limits::FULL);
    let queued = &view.queue[0];
    assert_eq!(queued.text, "and then this");
    assert!(queued.context);
    assert_eq!(queued.files, ["notes.md"]);
    // The id reads back as the provider's `unqueue` reads it.
    let back: CommandId = serde_json::from_value(serde_json::json!(queued.id)).unwrap();
    assert_eq!(back, id);
}

#[test]
fn rows_answers_and_groups_stay_within_their_limits() {
    let mut kinds = Vec::new();
    for index in 0..30 {
        kinds.push(user(&format!("prompt {index}")));
        kinds.push(agent(&"word ".repeat(100)));
    }
    let limits = Limits {
        rows: 10,
        answer: 64,
        group: 3,
        ..Limits::FULL
    };
    let view = prepare(&transcript(kinds), &limits);
    assert_eq!(view.rows.len(), 10);
    assert_eq!(view.hidden, 50);
    assert_eq!(view.rows[0].key(), "e51");
    let Row::Answer { text, .. } = &view.rows[1] else {
        panic!()
    };
    assert!(text.starts_with('…') && text.len() <= 64 + '…'.len_utf8());

    let calls = (0..8)
        .map(|index| call(&format!("c{index}"), ToolKind::Read, "Read"))
        .collect();
    let view = prepare(&transcript(calls), &limits);
    let tools = group(&view.rows[0]);
    assert_eq!((tools.count, tools.more, tools.items.len()), (8, 5, 3));
    assert_eq!(tools.title, "Read 8 files");

    let json = serde_json::to_value(prepare(&transcript(vec![]), &Limits::SHORT)).unwrap();
    assert_eq!(json["rows"], serde_json::json!([]));
}

#[test]
fn a_group_keeps_the_diagrams_its_calls_showed_past_its_trim() {
    let answer = |call: &str, title: &str, status: ToolStatus, text: &str| {
        let mut shown = tool(call, ToolKind::Other, title, status);
        shown.content = vec![ToolContent::Text {
            text: text.to_owned(),
        }];
        EntryKind::Tool(shown)
    };
    let mut calls = vec![
        answer(
            "d1",
            "mcp__moonframe-diagrams__diagram_show",
            ToolStatus::Completed,
            "Showing `plan` (3 nodes) to the user.\n[run session-1]",
        ),
        answer(
            "d2",
            "diagram_patch",
            ToolStatus::Completed,
            "`plan`: added 1 node.\n[run session-1]",
        ),
        answer(
            "d3",
            "diagram_show",
            ToolStatus::Failed,
            "Showing `broken` (0 nodes)",
        ),
        answer(
            "d4",
            "diagram_show",
            ToolStatus::Completed,
            "Showing `flow` (2 nodes) to the user.",
        ),
        answer(
            "r1",
            "Read notes.md",
            ToolStatus::Completed,
            "`notes` say hi",
        ),
    ];
    calls.extend((0..8).map(|index| call(&format!("c{index}"), ToolKind::Read, "Read")));
    let limits = Limits {
        group: 3,
        ..Limits::FULL
    };
    let view = prepare(&transcript(calls), &limits);
    let tools = group(&view.rows[0]);
    assert_eq!(tools.items.len(), 3);
    let shown = |id: &str, run: Option<&str>| DiagramRef {
        id: id.to_owned(),
        run: run.map(str::to_owned),
    };
    assert_eq!(tools.diagrams, [
        shown("plan", Some("session-1")),
        shown("flow", None)
    ]);
}

#[test]
fn edits_count_their_lines_per_file() {
    let mut edit = tool("e", ToolKind::Edit, "Edit lib.rs", ToolStatus::Completed);
    edit.content = vec![
        ToolContent::Diff {
            path: "src/lib.rs".to_owned(),
            old: Some("a\nb\nc\nd".to_owned()),
            new: "a\nB\nB2\nc\nd".to_owned(),
        },
        ToolContent::Diff {
            path: "src/new.rs".to_owned(),
            old: None,
            new: "one\ntwo".to_owned(),
        },
    ];
    let mut again = tool("f", ToolKind::Edit, "Edit lib.rs", ToolStatus::Completed);
    again.content = vec![ToolContent::Diff {
        path: "src/lib.rs".to_owned(),
        old: Some("x".to_owned()),
        new: "y".to_owned(),
    }];
    let view = prepare(
        &transcript(vec![EntryKind::Tool(edit), EntryKind::Tool(again)]),
        &Limits::FULL,
    );
    let tools = group(&view.rows[0]);
    let lib = &tools.files[0];
    assert_eq!((lib.added, lib.removed), (Some(3), Some(2)));
    assert!(tools.files[1].new);
    assert_eq!(tools.files[1].added, Some(2));
    assert_eq!(tools.title, "Edited 2 files");
}

#[test]
fn modes_models_and_effort_are_pickers_and_the_rest_settings() {
    let mut chat = transcript(vec![]);
    chat.modes = Some(Modes {
        available: vec![Mode {
            description: None,
            id: "plan".to_owned(),
            name: "Plan".to_owned(),
        }],
        current: "plan".to_owned(),
    });
    let select = |id: &str, category, current: &str| ConfigOption {
        category,
        description: None,
        id: id.to_owned(),
        kind: OptionKind::Select {
            choices: vec![Choice {
                description: None,
                group: None,
                id: current.to_owned(),
                name: current.to_uppercase(),
            }],
            current: current.to_owned(),
        },
        name: id.to_owned(),
    };
    chat.options = vec![
        select("model", OptionCategory::Model, "opus"),
        select("effort", OptionCategory::ThoughtLevel, "high"),
        select("permission", OptionCategory::Mode, "ask"),
        ConfigOption {
            category: OptionCategory::Other,
            description: None,
            id: "fast".to_owned(),
            kind: OptionKind::Toggle { current: true },
            name: "Fast".to_owned(),
        },
    ];
    chat.usage = Some(Usage {
        cost: Some(Cost {
            currency: "USD".to_owned(),
            micros: 420_000,
        }),
        size: 200_000,
        used: 45_200,
    });
    let view = prepare(&chat, &Limits::FULL);
    let mode = view.mode.unwrap();
    assert_eq!((mode.send, mode.current_name.as_str()), ("mode", "Plan"));
    let model = view.model.unwrap();
    assert_eq!(
        (model.send, model.current_name.as_str()),
        ("setting", "OPUS")
    );
    assert_eq!(view.effort.unwrap().current, "high");
    let settings: Vec<&str> = view
        .settings
        .iter()
        .map(|picker| picker.id.as_str())
        .collect();
    assert_eq!(settings, ["fast"], "the mode setting is not a second mode");
    assert_eq!(view.settings[0].current_name, "On");
    let usage = view.usage.unwrap();
    assert_eq!((usage.percent, usage.text.as_str()), (22, "45.2k / 200k"));
    assert_eq!(usage.cost.as_deref(), Some("$0.42"));
}

#[test]
fn turns_say_how_they_ended() {
    let view = prepare(
        &transcript(vec![
            turn(3_000, TurnEnd::Cancelled, None),
            turn(850, TurnEnd::Done, Some(0)),
            turn(3_725_000, TurnEnd::Done, Some(1_250_000)),
        ]),
        &Limits::FULL,
    );
    let texts: Vec<&str> = view
        .rows
        .iter()
        .map(|row| match row {
            Row::Turn { text, .. } => text.as_str(),
            _ => "",
        })
        .collect();
    assert_eq!(texts, [
        "Stopped after 3s",
        "Worked 850ms",
        "Worked 1h 02m · 1.2M tokens"
    ]);
}

#[test]
fn failed_and_read_only_chats_take_no_prompts() {
    let mut chat = transcript(vec![]);
    chat.status = Status::Failed {
        message: "agent crashed".to_owned(),
    };
    let view = prepare(&chat, &Limits::FULL);
    assert_eq!((view.status, view.accepts), ("failed", false));
    assert_eq!(view.message.as_deref(), Some("agent crashed"));
    chat.status = Status::Idle;
    chat.read_only = true;
    assert!(!prepare(&chat, &Limits::FULL).accepts);
}
