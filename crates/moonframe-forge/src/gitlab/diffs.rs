//! Complete line counts only: paginate file diffs and refuse totals when
//! GitLab omitted a patch or a hunk is truncated.

use serde_json::Value;

pub(super) fn read(
    changed_files: Option<&Value>,
    mut page: impl FnMut(usize) -> Option<Value>,
) -> Option<(u64, u64)> {
    // GitLab uses "1000+" when the change count itself was capped.
    let expected = changed_files
        .and_then(Value::as_str)
        .map(str::parse::<usize>)
        .transpose()
        .ok()?;
    let (mut files, mut added, mut removed) = (0, 0, 0);
    for number in 1..=100 {
        let value = page(number)?;
        let diffs = value.as_array()?;
        for diff in diffs {
            if diff["collapsed"] == true || diff["too_large"] == true {
                return None;
            }
            let (plus, minus) = count(diff["diff"].as_str()?)?;
            added += plus;
            removed += minus;
        }
        files += diffs.len();
        if diffs.len() < 100 {
            return expected
                .is_none_or(|expected| files == expected)
                .then_some((added, removed));
        }
    }
    None
}

fn length(range: &str, sign: char) -> Option<u64> {
    let range = range.strip_prefix(sign)?;
    let (start, count) = range.split_once(',').unwrap_or((range, "1"));
    start.parse::<u64>().ok()?;
    count.parse().ok()
}

fn count(text: &str) -> Option<(u64, u64)> {
    if text.is_empty() {
        // Binary files, mode changes and pure renames have no text hunks.
        return Some((0, 0));
    }
    let (mut old, mut new, mut added, mut removed) = (0, 0, 0, 0);
    let mut in_hunk = false;
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            if old != 0 || new != 0 {
                return None;
            }
            let (ranges, _) = header.split_once(" @@")?;
            let (before, after) = ranges.split_once(' ')?;
            old = length(before, '-')?;
            new = length(after, '+')?;
            in_hunk = true;
        } else if in_hunk {
            match line.as_bytes().first()? {
                b'+' => {
                    new = new.checked_sub(1)?;
                    added += 1;
                },
                b'-' => {
                    old = old.checked_sub(1)?;
                    removed += 1;
                },
                b' ' => {
                    old = old.checked_sub(1)?;
                    new = new.checked_sub(1)?;
                },
                b'\\' if line == "\\ No newline at end of file" => {},
                _ => return None,
            }
        }
    }
    (in_hunk && old == 0 && new == 0).then_some((added, removed))
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn increment_and_decrement_lines_are_not_headers() {
        assert_eq!(
            count("--- a/a\n+++ b/a\n@@ -1,2 +1,2 @@\n---counter;\n-old\n+++counter;\n+new\n"),
            Some((2, 2))
        );
        assert_eq!(
            count("@@ -0,0 +1 @@\n+++counter;\n\\ No newline at end of file\n"),
            Some((1, 0))
        );
        assert_eq!(count("@@ -1 +0,0 @@\n---counter;\n"), Some((0, 1)));
    }

    #[test]
    fn hunks_must_be_complete() {
        assert_eq!(
            count("@@ -1,2 +1,2 @@\n context\n-old\n+new\n@@ -9 +9 @@\n-a\n+b\n"),
            Some((2, 2))
        );
        assert_eq!(count("@@ -1,2 +1,2 @@\n-old\n+new\n"), None);
        assert_eq!(count("@@ -1 +1 @@\n-old\n+new\n+extra\n"), None);
    }

    #[test]
    fn counts_all_pages_and_never_returns_a_partial_total() {
        let diff = json!({ "diff": "@@ -0,0 +1 @@\n+line\n" });
        let mut requested = Vec::new();
        let total = read(Some(&json!("101")), |page| {
            requested.push(page);
            Some(if page == 1 {
                json!(vec![diff.clone(); 100])
            } else {
                json!([diff])
            })
        });
        assert_eq!(requested, [1, 2]);
        assert_eq!(total, Some((101, 0)));
        assert_eq!(
            read(None, |page| (page == 1)
                .then(|| json!(vec![diff.clone(); 100]))),
            None
        );
        assert_eq!(read(Some(&json!("101")), |_| Some(json!([diff]))), None);
        assert_eq!(
            read(Some(&json!("1000+")), |_| panic!(
                "capped diffs need no requests"
            )),
            None
        );
    }

    #[test]
    fn omitted_patches_have_no_totals() {
        for diff in [
            json!({ "collapsed": true, "diff": "" }),
            json!({ "too_large": true, "diff": "" }),
            json!({}),
            json!({ "diff": "@@ -0,0 +1,2 @@\n+truncated\n" }),
        ] {
            assert_eq!(read(None, |_| Some(json!([diff]))), None);
        }
        assert_eq!(read(None, |_| Some(json!([{ "diff": "" }]))), Some((0, 0)));
    }
}
