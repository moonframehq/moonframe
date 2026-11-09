//! What changed in the checkout: status, counts, the base to compare with,
//! and its worktrees.

use {
    super::Repo,
    crate::config::base_name,
    serde_json::{Value, json},
};

/// A file's row in `files`.
fn row(path: &str, status: &str, staged: bool, unstaged: bool) -> Value {
    json!({
        "path": path,
        "status": status,
        "staged": staged,
        "unstaged": unstaged,
        "added": 0,
        "deleted": 0,
    })
}

/// Staged and unstaged per file, from porcelain v1.
pub(super) fn parse_status(text: &str, prefix: &str) -> Vec<Value> {
    let mut found = Vec::new();
    let mut parts = text.split('\0');
    while let Some(part) = parts.next() {
        let mut letters = part.chars();
        let (Some(x), Some(y)) = (letters.next(), letters.next()) else {
            continue;
        };
        if part.len() < 4 {
            continue;
        }
        if x == 'R' || x == 'C' {
            parts.next();
        }
        // Status gives paths from the repository's top; everything else here
        // works from the project folder, which may be below it.
        let Some(path) = part.get(3..).and_then(|path| path.strip_prefix(prefix)) else {
            continue;
        };
        let status = match (x, y) {
            ('?', _) => "?".to_owned(),
            ('U', _) | (_, 'U') => "U".to_owned(),
            (' ', other) | (other, _) => other.to_string(),
        };
        found.push(row(path, &status, x != ' ' && x != '?', y != ' '));
    }
    found
}

fn add_counts(files: &mut [Value], numstat: &str) {
    for line in numstat.lines() {
        let mut parts = line.split('\t');
        let (Some(plus), Some(minus), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if let Some(file) = files.iter_mut().find(|file| file["path"] == path) {
            file["added"] = json!(plus.parse::<i64>().unwrap_or(0));
            file["deleted"] = json!(minus.parse::<i64>().unwrap_or(0));
        }
    }
}

impl Repo {
    /// What to diff against: HEAD for the working tree's own changes.
    pub(super) fn range(&self) -> Vec<String> {
        let compare = &self.compare;
        if compare.mode == "range"
            && let Some((from, to)) = &compare.commits
        {
            return vec![from.clone(), to.clone()];
        }
        match &self.base {
            Some(base) if compare.mode == "branch" => vec![base.clone()],
            _ => vec!["HEAD".to_owned()],
        }
    }

    /// The branch to compare with: `(name, merge base)`.
    pub(super) fn base_branch(&self) -> Option<(String, String)> {
        for name in [
            "origin/HEAD",
            "origin/main",
            "origin/master",
            "main",
            "master",
        ] {
            let wanted = format!("{name}^{{commit}}");
            let Ok(sha) = self.git(&["rev-parse", "--verify", "-q", &wanted]) else {
                continue;
            };
            let short = if name == "origin/HEAD" {
                let abbreviated = self.git(&["rev-parse", "--abbrev-ref", name]);
                abbreviated.map_or_else(|_| name.to_owned(), |text| text.trim().to_owned())
            } else {
                name.to_owned()
            };
            if self.branch.as_deref() == Some(short.as_str()) {
                continue;
            }
            if let Ok(merge) = self.git(&["merge-base", "HEAD", sha.trim()]) {
                return Some((short, merge.trim().to_owned()));
            }
        }
        None
    }

    pub(super) fn read_status(&self) -> Result<Vec<Value>, String> {
        if self.compare.mode == "uncommitted" {
            let prefix = self.git(&["rev-parse", "--show-prefix"])?;
            let args = [
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--",
                ".",
            ];
            let mut files = parse_status(&self.git(&args)?, prefix.trim());
            let args = ["diff", "--numstat", "--no-renames", "--relative", "HEAD"];
            add_counts(&mut files, &self.git(&args).unwrap_or_default());
            return Ok(files);
        }
        let range = self.range();
        let refs: Vec<&str> = range.iter().map(String::as_str).collect();
        let mut args = vec!["diff", "--no-renames", "--relative", "--name-status"];
        args.extend(&refs);
        let mut files = Vec::new();
        for line in self.git(&args)?.lines() {
            let Some((status, path)) = line.split_once('\t') else {
                continue;
            };
            let letter = status.get(..1).unwrap_or_default();
            files.push(row(path, letter, false, false));
        }
        let mut args = vec!["diff", "--numstat", "--no-renames", "--relative"];
        args.extend(&refs);
        add_counts(&mut files, &self.git(&args)?);
        Ok(files)
    }

    pub(super) fn read_worktrees(&self) -> Result<Vec<Value>, String> {
        let mut list: Vec<Value> = Vec::new();
        for line in self.git(&["worktree", "list", "--porcelain"])?.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                list.push(json!({
                    "path": path,
                    "name": base_name(path),
                    "branch": null,
                    "current": self.at.as_deref() == Some(path),
                }));
            } else if let (Some(branch), Some(item)) =
                (line.strip_prefix("branch "), list.last_mut())
            {
                let short = branch.strip_prefix("refs/heads/").unwrap_or(branch);
                item["branch"] = json!(short);
            }
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_staged_unstaged_and_renames() {
        let text = "M  a.txt\0 M b.txt\0?? c.txt\0R  new.txt\0old.txt\0UU d.txt\0";
        let files = parse_status(text, "");
        let seen: Vec<(&str, &str, bool, bool)> = files
            .iter()
            .map(|file| {
                (
                    file["path"].as_str().unwrap_or_default(),
                    file["status"].as_str().unwrap_or_default(),
                    file["staged"] == true,
                    file["unstaged"] == true,
                )
            })
            .collect();
        assert_eq!(seen, [
            ("a.txt", "M", true, false),
            ("b.txt", "M", false, true),
            ("c.txt", "?", false, true),
            ("new.txt", "R", true, false),
            ("d.txt", "U", true, true),
        ]);
    }

    #[test]
    fn paths_come_from_the_project_folder() {
        let files = parse_status(" M src/a.js\0 M other.txt\0", "src/");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["path"], "a.js");
    }
}
