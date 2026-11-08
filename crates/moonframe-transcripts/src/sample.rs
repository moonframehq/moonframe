//! Sessions to show when there are none to read (or `CINDER_SAMPLE` is set).

use {
    crate::sessions::{Group, Session},
    serde_json::{Value, json},
};

/// What `total` says of the samples, as the Python provider said it.
pub(crate) const TOTAL: usize = 10;

/// Each repository's sessions: title, branch, opened a pull request, its
/// folder gone, age.
type Rows = &'static [(&'static str, &'static str, bool, bool, &'static str)];

const GROUPS: &[(&str, &str, Rows)] = &[
    ("lantern", "~/code/lantern", &[
        (
            "Lantern dashboard and web tasks view",
            "feat/dashboard",
            true,
            false,
            "12m",
        ),
        (
            "Next-step plan review for the job queue",
            "main",
            false,
            false,
            "1h",
        ),
    ]),
    ("DragKit", "~/code/DragKit", &[(
        "Cmd-click drag not working on Sonoma",
        "fix/cmd-drag",
        false,
        false,
        "3h",
    )]),
    ("tideline", "~/code/tideline", &[
        (
            "Phase 2 string catalogue migration",
            "i18n/phase-2",
            false,
            false,
            "5h",
        ),
        (
            "tideline-usage catalogue migration",
            "i18n/usage",
            false,
            false,
            "6h",
        ),
        (
            "Activity bar translations",
            "i18n/activity",
            false,
            false,
            "8h",
        ),
        ("I18N.md phase 2 plan", "main", false, false, "9h"),
        (
            "Multilanguage support for the renderer",
            "i18n/core",
            false,
            false,
            "1d",
        ),
        (
            "Merge main and resolve conflicts in the store",
            "merge/main",
            true,
            true,
            "1d",
        ),
        (
            "Merge main and fix the lint score",
            "merge/lint",
            true,
            true,
            "2d",
        ),
        (
            "PR #904 mods and layouts",
            "feat/layouts",
            false,
            false,
            "2d",
        ),
        ("New release push", "release/1.8", false, false, "3d"),
        (
            "tideline Windows daemon connection",
            "fix/windows",
            false,
            false,
            "4d",
        ),
    ]),
];

pub(crate) fn groups() -> Vec<Group> {
    let mut count = 0;
    GROUPS
        .iter()
        .map(|&(name, path, rows)| Group {
            name: name.to_owned(),
            path: path.to_owned(),
            sessions: rows
                .iter()
                .map(|&(title, branch, pr, gone, age)| {
                    count += 1;
                    Session {
                        id: format!("sample-{count}"),
                        title: Value::String(title.to_owned()),
                        branch: branch.to_owned(),
                        age: age.to_owned(),
                        pr,
                        gone,
                        path: None,
                    }
                })
                .collect(),
        })
        .collect()
}

fn tools(calls: &[(&str, &str)]) -> Value {
    let tools: Vec<Value> = calls
        .iter()
        .map(|(name, detail)| json!({ "name": name, "detail": detail }))
        .collect();
    json!({ "kind": "tools", "text": "", "tools": tools })
}

fn text(body: &str) -> Value {
    json!({ "kind": "text", "text": body, "tools": [] })
}

pub(crate) fn transcript(session: &Session, group: &Group) -> Value {
    let title = session.title.as_str().unwrap_or_default();
    json!({
        "id": session.id,
        "title": session.title,
        "project": group.name,
        "branch": session.branch,
        "hidden": 0,
        "turns": [
            {
                "from": "user",
                "parts": [text(&format!(
                    "{title}. Start by reading how it works today, then propose the change."
                ))],
            },
            {
                "from": "assistant",
                "parts": [
                    text("I'll look at the current code first."),
                    tools(&[("Bash", "List the source tree"), ("Read", "src/app.rs"), ("Grep", "fn render")]),
                    text(
                        "Here is what I found:\n\n- The view rebuilds its whole tree on every change.\n\
                         - The store keeps one list per window.\n\nI'd split the list out and keep it in the \
                         store, then render it lazily:\n\n```rust\nlet rows = store.rows(window);\n\
                         list.set_count(rows.len());\n```\n\nShall I go ahead?"
                    ),
                ],
            },
            { "from": "user", "parts": [text("Yes, go ahead and run the tests after.")] },
            {
                "from": "assistant",
                "parts": [
                    tools(&[("Edit", "src/store.rs"), ("Edit", "src/app.rs"), ("Bash", "Run the tests")]),
                    text("Done: the list lives in the store now, and all 214 tests pass."),
                ],
            },
        ],
    })
}
