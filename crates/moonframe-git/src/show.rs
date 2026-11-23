//! One commit a mod asked to look at: who made it and when, its message,
//! the files it changed with their line counts, and on request one of
//! those files' diff. A merge is compared with its first parent, as
//! `git log --first-parent` would show it, and a first commit with nothing.
//!
//! A commit never changes, so it is read once when asked for, not at every
//! look.

use {
    crate::{Repo, diff, status::Numstat},
    serde_json::{Value, json},
};

/// Files listed for one commit; a larger one says `truncated`.
const MAX_FILES: usize = 400;

/// Commit fields, NUL-parted as in [`crate::log`]; the body last, as the
/// one field that may hold newlines.
const FORMAT: &str = "--format=%H%x00%h%x00%P%x00%an%x00%ae%x00%at%x00%D%x00%s%x00%b";
const FIELDS: usize = 9;

/// Show a merge against its first parent, a first commit against nothing.
const FIRST_PARENT: &str = "--diff-merges=first-parent";

/// One changed file: its status letter, where it is now, and where it was
/// for a rename or copy.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Changed {
    status: char,
    path: String,
    old: Option<String>,
}

/// The commit on show, and the file whose diff is open.
#[derive(Debug)]
pub(crate) struct Shown {
    sha: String,
    files: Vec<Changed>,
    commit: Value,
    diff: Option<Value>,
}

impl Shown {
    /// Reads the commit `asked` names, a hex object name; `None` when it is
    /// not one or the repository has no such commit.
    pub(crate) fn read(repo: &Repo, asked: &str) -> Option<Self> {
        if !is_object_name(asked) {
            return None;
        }
        let spec = format!("{asked}^{{commit}}");
        let sha = repo.text(
            &["rev-parse", "-q", "--verify", "--end-of-options", &spec],
            "find a commit",
        )?;
        let header = repo.text(
            &[
                "show",
                "--no-show-signature",
                "--no-patch",
                "-z",
                FORMAT,
                &sha,
            ],
            "read a commit",
        )?;
        let mut commit = header_json(&header)?;
        let names = repo
            .bytes(
                &[
                    "show",
                    FIRST_PARENT,
                    "--format=",
                    "-M",
                    "--name-status",
                    "-z",
                    &sha,
                ],
                "list a commit's files",
            )
            .unwrap_or_default();
        let numstat = Numstat::parse(
            &repo
                .bytes(
                    &[
                        "show",
                        FIRST_PARENT,
                        "--format=",
                        "-M",
                        "--numstat",
                        "-z",
                        &sha,
                    ],
                    "count a commit's lines",
                )
                .unwrap_or_default(),
        );
        let mut files = name_status(&names);
        let truncated = files.len() > MAX_FILES;
        let count = files.len();
        files.truncate(MAX_FILES);
        let lines: Vec<String> = files
            .iter()
            .map(|file| {
                let (added, deleted) = numstat.of(&file.path);
                format!("{}\t{added}\t{deleted}\t{}", file.status, file.path)
            })
            .collect();
        if let Value::Object(fields) = &mut commit {
            fields.insert("files".to_owned(), json!(lines));
            fields.insert("count".to_owned(), json!(count));
            fields.insert("truncated".to_owned(), json!(truncated));
            fields.insert("added".to_owned(), json!(numstat.added));
            fields.insert("deleted".to_owned(), json!(numstat.deleted));
        }
        Some(Self {
            sha,
            files,
            commit,
            diff: None,
        })
    }

    /// Whether it is the commit `asked` names already.
    pub(crate) fn is(&self, asked: &str) -> bool {
        !asked.is_empty() && self.sha.starts_with(&asked.to_ascii_lowercase())
    }

    /// Opens the diff of `path`, one of the commit's files; any other path
    /// (`../..`, a file it did not change) opens an empty one.
    pub(crate) fn open_diff(&mut self, repo: &Repo, path: &str) {
        let diff = match self.files.iter().find(|file| file.path == path) {
            Some(file) => {
                let mut args = vec![
                    "-c",
                    "core.quotePath=false",
                    "show",
                    "--no-show-signature",
                    FIRST_PARENT,
                    "--format=",
                    "--no-color",
                    "--no-ext-diff",
                    "--no-textconv",
                    "-M",
                    &self.sha,
                    "--",
                ];
                let new = format!(":(literal){}", file.path);
                let old = file.old.as_ref().map(|old| format!(":(literal){old}"));
                args.push(&new);
                if let Some(old) = &old {
                    args.push(old);
                }
                diff::parse(
                    path,
                    &repo.text(&args, "diff a commit's file").unwrap_or_default(),
                )
            },
            None => diff::parse(path, ""),
        };
        self.diff = Some(diff);
    }

    pub(crate) fn close_diff(&mut self) {
        self.diff = None;
    }

    /// What `app.git.commit` holds: the commit, and `diff` while one is open.
    pub(crate) fn json(&self) -> Value {
        let mut commit = self.commit.clone();
        if let (Some(diff), Value::Object(fields)) = (&self.diff, &mut commit) {
            fields.insert("diff".to_owned(), diff.clone());
        }
        commit
    }
}

/// A full or abbreviated hex object name, and nothing git would read as an
/// option or a range.
fn is_object_name(text: &str) -> bool {
    (4..=64).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn header_json(text: &str) -> Option<Value> {
    let fields: Vec<&str> = text.splitn(FIELDS, '\0').collect();
    let [
        sha,
        short,
        parents,
        author,
        email,
        time,
        refs,
        subject,
        body,
    ] = fields.as_slice()
    else {
        return None;
    };
    Some(json!({
        "sha": sha,
        "short": short,
        "parents": parents.split_whitespace().collect::<Vec<_>>(),
        "author": author,
        "email": email,
        "time": time.parse::<i64>().ok()?,
        "refs": crate::log::names(refs),
        "subject": subject,
        "body": body.trim_end_matches(['\0', '\n', '\r']).trim_start_matches('\n'),
    }))
}

/// `--name-status -z`: `M\0path\0`, or `R100\0old\0new\0` for a rename or
/// copy. Status letters past A, C, D, M, R and T read as M.
fn name_status(bytes: &[u8]) -> Vec<Changed> {
    let mut files = Vec::new();
    let mut parts = bytes
        .split(|byte| *byte == 0)
        .map(|part| String::from_utf8_lossy(part).into_owned());
    while let Some(status) = parts.next() {
        let Some(letter) = status.trim_start_matches('\n').chars().next() else {
            continue;
        };
        let (path, old) = if matches!(letter, 'R' | 'C') {
            let old = parts.next();
            (parts.next(), old)
        } else {
            (parts.next(), None)
        };
        let Some(path) = path else {
            break;
        };
        let status = match letter {
            'A' | 'C' => 'A',
            'D' => 'D',
            'R' => 'R',
            _ => 'M',
        };
        files.push(Changed { status, path, old });
    }
    files
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn only_hex_names_are_asked_of_git() {
        assert!(is_object_name("0225fc4c"));
        assert!(is_object_name(&"a".repeat(40)));
        for bad in [
            "",
            "abc",
            "--all",
            "HEAD",
            "a..b",
            "0225fc4c^",
            &"a".repeat(65),
        ] {
            assert!(!is_object_name(bad), "{bad}");
        }
    }

    #[test]
    fn renames_keep_where_the_file_was() {
        let files = name_status(b"M\0src/a.rs\0R087\0old.rs\0new.rs\0A\0b.rs\0D\0c.rs\0T\0d\0");
        let got: Vec<(char, &str, Option<&str>)> = files
            .iter()
            .map(|file| (file.status, file.path.as_str(), file.old.as_deref()))
            .collect();
        assert_eq!(got, vec![
            ('M', "src/a.rs", None),
            ('R', "new.rs", Some("old.rs")),
            ('A', "b.rs", None),
            ('D', "c.rs", None),
            ('M', "d", None),
        ]);
    }

    /// The provider shows a commit asked for, then one of its files' diff,
    /// and never a file the commit did not change.
    #[test]
    fn a_commit_is_shown_with_its_files_and_a_diff() {
        use {
            crate::{
                Git,
                tests::{events, published, repository},
            },
            moonframe_core::Data,
            moonframe_providers::{Handle, Native as _},
            std::{collections::BTreeMap, process::Command},
        };
        let dir = repository();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "new\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "second", "-m", "Why it changed."]);
        let first = git(&["rev-parse", "HEAD~1"]);
        let second = git(&["rev-parse", "HEAD"]);
        let (events, seen) = events();
        let folder = dir.path().join("providers").join("git");
        std::fs::create_dir_all(&folder).unwrap();
        let host = Handle::new("git", &folder, Data::Nil, events);
        let mut provider = Git::default();
        provider.start(&host).unwrap();
        let send = |provider: &mut Git, pairs: &[(&str, &str)]| {
            let fields = pairs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), Data::from(*value)))
                .collect::<BTreeMap<_, _>>();
            provider.receive(&host, "browser", Data::Map(fields));
            published(&seen)
        };
        let state = send(&mut provider, &[("show", &second[..10])]);
        let commit = &state["commit"];
        assert_eq!(commit["sha"], second.as_str());
        assert_eq!(commit["parents"], json!([first]));
        assert_eq!(commit["subject"], "second");
        assert_eq!(commit["body"], "Why it changed.");
        assert_eq!(commit["files"], json!(["M\t1\t0\ta.txt", "A\t1\t0\tb.txt"]));
        assert_eq!(
            (commit["added"].clone(), commit["deleted"].clone()),
            (json!(2), json!(0))
        );
        let state = send(&mut provider, &[("show_diff", "a.txt")]);
        let lines = &state["commit"]["diff"]["hunks"][0]["lines"];
        assert_eq!(lines[1], json!({ "kind": "add", "new": 2, "text": "two" }));
        let state = send(&mut provider, &[("show_diff", "../../etc/hosts")]);
        assert_eq!(state["commit"]["diff"]["lines"], 0);
        // The first commit is compared with nothing.
        let state = send(&mut provider, &[("show", &first)]);
        assert_eq!(state["commit"]["files"], json!(["A\t1\t0\ta.txt"]));
        assert!(state["commit"].get("diff").is_none());
        let state = send(&mut provider, &[("show", "--all")]);
        assert!(state.get("commit").is_none());
    }

    #[test]
    fn the_header_keeps_a_body_of_many_lines() {
        let text = "aaa\0a\0bbb ccc\0Ada\0ada@example.com\x001700\0HEAD -> main, tag: v1\0Merge\0First line\n\nSecond\n";
        let commit = header_json(text).expect("a commit");
        assert_eq!(commit["parents"], json!(["bbb", "ccc"]));
        assert_eq!(commit["refs"], json!(["main", "v1"]));
        assert_eq!(commit["body"], "First line\n\nSecond");
        assert!(header_json("aaa\0a").is_none());
    }
}
