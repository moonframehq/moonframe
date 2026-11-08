//! A file's diff as the window draws it.

use {
    super::Repo,
    crate::{git::run_git, value::first_chars},
    serde_json::{Value, json},
};

const MAX_DIFF_LINES: usize = 1500;
const MAX_LINE: usize = 400;
const LONGEST_HEADER: usize = 120;

/// The old and new starting lines of a hunk's `@@ -a,b +c,d @@`.
fn hunk_start(line: &str) -> Option<(u64, u64)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let first = |range: &str| range.split(',').next()?.parse().ok();
    Some((first(old)?, first(new)?))
}

/// Hunks of lines, and whether it stopped short.
pub(super) fn parse_diff(text: &str) -> (Vec<Value>, bool) {
    let mut hunks: Vec<Value> = Vec::new();
    let (mut old_line, mut new_line) = (0, 0);
    let mut count = 0;
    let mut truncated = false;
    for raw in text.split('\n') {
        if raw.starts_with("@@") {
            let Some((old, new)) = hunk_start(raw) else {
                continue;
            };
            old_line = old;
            new_line = new;
            hunks.push(json!({ "header": first_chars(raw, LONGEST_HEADER), "lines": [] }));
            continue;
        }
        let Some(Value::Array(lines)) = hunks.last_mut().map(|hunk| &mut hunk["lines"]) else {
            continue;
        };
        if count >= MAX_DIFF_LINES {
            truncated = true;
            break;
        }
        let mut chars = raw.chars();
        let sign = chars.next();
        let body = first_chars(chars.as_str(), MAX_LINE - 1);
        match sign {
            Some('+') => {
                lines.push(json!({ "kind": "add", "new": new_line, "text": body }));
                new_line += 1;
            },
            Some('-') => {
                lines.push(json!({ "kind": "del", "old": old_line, "text": body }));
                old_line += 1;
            },
            Some(' ') => {
                lines
                    .push(json!({ "kind": "ctx", "old": old_line, "new": new_line, "text": body }));
                old_line += 1;
                new_line += 1;
            },
            _ => continue,
        }
        count += 1;
    }
    (hunks, truncated)
}

impl Repo {
    pub(super) fn read_diff(&self, path: &str) -> Result<Value, String> {
        let file = self.files.iter().find(|file| file["path"] == path);
        let Some(file) = file else {
            return Ok(Value::Null);
        };
        if file["status"] == "?" {
            // `--no-index` exits 1 when files differ, which is the point here.
            let args = ["diff", "--no-index", "--", "/dev/null", path];
            let ran = run_git(&self.runtime, self.cwd(), &args, &[], None);
            let (hunks, truncated) = parse_diff(&String::from_utf8_lossy(&ran.stdout));
            return Ok(json!({
                "path": path, "status": "A", "hunks": hunks, "truncated": truncated,
            }));
        }
        let range = self.range();
        let mut args = vec!["diff", "--no-renames", "--relative"];
        args.extend(range.iter().map(String::as_str));
        args.extend(["--", path]);
        let text = self.git(&args)?;
        let binary = text.lines().any(|line| line.starts_with("Binary files "));
        let (hunks, truncated) = parse_diff(&text);
        Ok(json!({
            "path": path,
            "status": file["status"],
            "binary": binary,
            "hunks": hunks,
            "truncated": truncated,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_numbered_from_their_hunk() {
        let text =
            "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -3,2 +3,2 @@ fn x\n same\n-old\n+new\n";
        let (hunks, truncated) = parse_diff(text);
        assert!(!truncated);
        assert_eq!(hunks[0]["header"], "@@ -3,2 +3,2 @@ fn x");
        let lines = &hunks[0]["lines"];
        assert_eq!(
            lines[0],
            json!({ "kind": "ctx", "old": 3, "new": 3, "text": "same" })
        );
        assert_eq!(lines[1], json!({ "kind": "del", "old": 4, "text": "old" }));
        assert_eq!(lines[2], json!({ "kind": "add", "new": 4, "text": "new" }));
    }

    #[test]
    fn a_long_diff_stops_short_and_says_so() {
        let mut text = "@@ -1 +1,2000 @@\n".to_owned();
        text.push_str(&"+x\n".repeat(2000));
        let (hunks, truncated) = parse_diff(&text);
        assert!(truncated);
        assert_eq!(
            hunks[0]["lines"].as_array().map(Vec::len),
            Some(MAX_DIFF_LINES)
        );
    }

    #[test]
    fn a_hunk_of_one_line_has_no_comma() {
        assert_eq!(hunk_start("@@ -1 +2 @@"), Some((1, 2)));
        assert_eq!(hunk_start("@@ nonsense"), None);
    }
}
