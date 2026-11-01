//! The working tree: `git status --porcelain=v2 --branch -z` and `git diff
//! --numstat -z` against HEAD, read into the branch fields and `files`.

use {
    serde_json::{Value, json},
    std::collections::HashMap,
};

/// Entries in `files`; `changed` still counts them all.
pub(crate) const MAX_FILES: usize = 300;

/// What `git status` says about the branch and each changed path.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Status {
    /// HEAD's commit, `None` before the first commit.
    pub(crate) oid: Option<String>,
    /// `HEAD` when detached.
    pub(crate) branch: String,
    pub(crate) detached: bool,
    pub(crate) upstream: Option<String>,
    pub(crate) ahead: u64,
    pub(crate) behind: u64,
    pub(crate) changed: u64,
    pub(crate) added: u64,
    pub(crate) deleted: u64,
    pub(crate) files: Vec<Value>,
    pub(crate) truncated: bool,
}

/// Line counts per path, and their totals, from `--numstat -z`.
#[derive(Debug, Default)]
pub(crate) struct Numstat {
    by_path: HashMap<String, (u64, u64)>,
    pub(crate) added: u64,
    pub(crate) deleted: u64,
}

impl Numstat {
    /// `added\tdeleted\tpath\0`, or for a rename `added\tdeleted\t\0old\0new\0`;
    /// binary files count `-`, read as 0.
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        let mut numstat = Self::default();
        let mut records = bytes.split(|byte| *byte == 0);
        while let Some(record) = records.next() {
            let record = String::from_utf8_lossy(record);
            let mut fields = record.splitn(3, '\t');
            let (Some(added), Some(deleted), Some(path)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let path = if path.is_empty() {
                let _old = records.next();
                records
                    .next()
                    .map(|path| String::from_utf8_lossy(path).into_owned())
                    .unwrap_or_default()
            } else {
                path.to_owned()
            };
            let added = added.parse().unwrap_or(0);
            let deleted = deleted.parse().unwrap_or(0);
            numstat.added += added;
            numstat.deleted += deleted;
            numstat.by_path.insert(path, (added, deleted));
        }
        numstat
    }
}

impl Status {
    pub(crate) fn parse(bytes: &[u8], numstat: &Numstat) -> Self {
        let mut status = Self {
            added: numstat.added,
            deleted: numstat.deleted,
            ..Self::default()
        };
        let mut records = bytes.split(|byte| *byte == 0);
        while let Some(record) = records.next() {
            let record = String::from_utf8_lossy(record);
            if let Some(header) = record.strip_prefix("# ") {
                status.header(header);
                continue;
            }
            // The path is the last field, after a fixed number of others,
            // and may itself hold spaces.
            let (xy, path, old) = match record.as_bytes().first() {
                Some(b'1') => (xy(&record), nth_rest(&record, 8), None),
                Some(b'2') => {
                    let old = records
                        .next()
                        .map(|old| String::from_utf8_lossy(old).into_owned());
                    (xy(&record), nth_rest(&record, 9), old)
                },
                Some(b'u') => ("UU".to_owned(), nth_rest(&record, 10), None),
                Some(b'?') => ("??".to_owned(), nth_rest(&record, 1), None),
                _ => continue,
            };
            let Some(path) = path else {
                continue;
            };
            status.changed += 1;
            if status.files.len() >= MAX_FILES {
                status.truncated = true;
                continue;
            }
            status.files.push(file(&xy, path, old.as_deref(), numstat));
        }
        if status.branch == "(detached)" {
            status.detached = true;
            status.branch = "HEAD".to_owned();
        }
        status
    }

    fn header(&mut self, header: &str) {
        if let Some(oid) = header.strip_prefix("branch.oid ") {
            self.oid = (oid != "(initial)").then(|| oid.to_owned());
        } else if let Some(head) = header.strip_prefix("branch.head ") {
            head.clone_into(&mut self.branch);
        } else if let Some(upstream) = header.strip_prefix("branch.upstream ") {
            self.upstream = Some(upstream.to_owned());
        } else if let Some(counts) = header.strip_prefix("branch.ab ") {
            let mut counts = counts.split(' ');
            self.ahead = number(counts.next(), '+');
            self.behind = number(counts.next(), '-');
        }
    }
}

fn number(field: Option<&str>, sign: char) -> u64 {
    field
        .and_then(|field| field.strip_prefix(sign))
        .and_then(|field| field.parse().ok())
        .unwrap_or(0)
}

fn xy(record: &str) -> String {
    record.get(2..4).unwrap_or("..").to_owned()
}

/// What follows the first `skip` space-separated fields.
fn nth_rest(record: &str, skip: usize) -> Option<&str> {
    record
        .splitn(skip + 1, ' ')
        .nth(skip)
        .filter(|rest| !rest.is_empty())
}

/// One `files` entry: its kind of change, whether it is staged or not, and
/// its lines (none for an untracked file).
fn file(xy: &str, path: &str, old: Option<&str>, numstat: &Numstat) -> Value {
    let (x, y) = {
        let mut chars = xy.chars();
        (chars.next().unwrap_or('.'), chars.next().unwrap_or('.'))
    };
    let kind = if xy == "??" {
        "?"
    } else if matches!(xy, "UU" | "AA" | "DD") || x == 'U' || y == 'U' {
        "U"
    } else if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
        "R"
    } else if x == 'A' || y == 'A' {
        "A"
    } else if x == 'D' || y == 'D' {
        "D"
    } else {
        "M"
    };
    let untracked = kind == "?";
    let (added, deleted) = if untracked {
        (0, 0)
    } else {
        numstat.by_path.get(path).copied().unwrap_or((0, 0))
    };
    let mut entry = json!({
        "path": path,
        "status": kind,
        "staged": !untracked && x != '.',
        "unstaged": untracked || y != '.',
        "added": added,
        "deleted": deleted,
    });
    if let (Some(old), Value::Object(fields)) = (old, &mut entry) {
        fields.insert("old_path".to_owned(), json!(old));
    }
    entry
}
