//! One file's diff against HEAD (or, untracked, against nothing) as hunks
//! of numbered lines, bounded so it fits what the host takes in one value.

use {
    crate::Repo,
    serde_json::{Value, json},
};

/// The host takes at most 10,000 values and 1 MiB per publish, so a diff
/// stops at this many lines, each cut to `MAX_TEXT` characters.
const MAX_LINES: usize = 1200;
const MAX_TEXT: usize = 300;
const MAX_BYTES: usize = 600_000;
/// Values a diff may hold, leaving the rest of a publish its room however
/// many hunks those lines are in; each line and each hunk is `ITEM_VALUES`.
const MAX_VALUES: usize = 6500;
const ITEM_VALUES: usize = 5;

pub(crate) fn read(repo: &Repo, base: &str, path: &str) -> Value {
    let literal = format!(":(literal){path}");
    let mut text = repo
        .text(
            &[
                "-c",
                "core.quotePath=false",
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                base,
                "--",
                &literal,
            ],
            "diff a file",
        )
        .unwrap_or_default();
    let untracked = text.is_empty()
        && repo.root.join(path).is_file()
        && repo
            .text(
                &["ls-files", "--error-unmatch", "--", &literal],
                "check a file",
            )
            .is_none();
    if untracked {
        // `--no-index` exits 1 when the files differ, which they do.
        text = repo
            .text_any_status(&[
                "diff",
                "--no-index",
                "--no-color",
                "--no-ext-diff",
                "--",
                "/dev/null",
                path,
            ])
            .unwrap_or_default();
    }
    parse(path, &text)
}

pub(crate) fn parse(path: &str, text: &str) -> Value {
    let mut hunks: Vec<Value> = Vec::new();
    let mut lines: Vec<Value> = Vec::new();
    let mut header: Option<(String, u64, u64)> = None;
    let (mut old, mut new, mut count, mut size) = (0u64, 0u64, 0usize, 0usize);
    let mut values = 0usize;
    let (mut binary, mut truncated) = (false, false);
    let close = |header: &mut Option<(String, u64, u64)>,
                 lines: &mut Vec<Value>,
                 hunks: &mut Vec<Value>| {
        if let Some((text, old_start, new_start)) = header.take() {
            hunks.push(json!({
                "header": text, "old_start": old_start, "new_start": new_start,
                "lines": std::mem::take(lines),
            }));
        }
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("@@ -") {
            if count >= MAX_LINES || size >= MAX_BYTES || values >= MAX_VALUES {
                truncated = true;
                break;
            }
            values += ITEM_VALUES;
            close(&mut header, &mut lines, &mut hunks);
            let start = |text: &str| {
                text.split([',', ' '])
                    .next()
                    .and_then(|number| number.parse().ok())
                    .unwrap_or(0)
            };
            old = start(rest);
            new = rest.split_once(" +").map_or(0, |(_, rest)| start(rest));
            header = Some((line.to_owned(), old, new));
            continue;
        }
        if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            binary = true;
            continue;
        }
        if header.is_none() {
            continue;
        }
        let (kind, body) = match line.chars().next() {
            Some('+') => ("add", &line[1..]),
            Some('-') => ("del", &line[1..]),
            Some(' ') => ("ctx", &line[1..]),
            None => ("ctx", ""),
            // "\ No newline at end of file"
            Some(_) => continue,
        };
        if count >= MAX_LINES || size >= MAX_BYTES || values >= MAX_VALUES {
            truncated = true;
            break;
        }
        count += 1;
        values += ITEM_VALUES;
        let text: String = body.chars().take(MAX_TEXT).collect();
        size += text.len() + 40;
        lines.push(match kind {
            "add" => {
                new += 1;
                json!({ "kind": "add", "new": new - 1, "text": text })
            },
            "del" => {
                old += 1;
                json!({ "kind": "del", "old": old - 1, "text": text })
            },
            _ => {
                old += 1;
                new += 1;
                json!({ "kind": "ctx", "old": old - 1, "new": new - 1, "text": text })
            },
        });
    }
    close(&mut header, &mut lines, &mut hunks);
    json!({
        "path": path, "binary": binary, "lines": count, "truncated": truncated, "hunks": hunks,
    })
}
