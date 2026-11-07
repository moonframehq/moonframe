//! Recent commits with their parents, for a mod that draws history. Read
//! only once a mod asks, since no one else pays for it.

use {
    crate::Repo,
    serde_json::{Value, json},
};

/// The most commits a mod can ask for.
pub(crate) const MAX_COMMITS: usize = 500;

/// Fields of one commit, split by US (0x1f). The subject goes last: it is
/// the one field a person writes.
const FORMAT: &str = "--format=%H%x1f%h%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s";

/// The newest `limit` commits of every branch, tag and HEAD, children
/// before their parents as a graph needs them; nothing before a first
/// commit.
pub(crate) fn commits(repo: &Repo, limit: usize) -> Vec<Value> {
    let count = limit.clamp(1, MAX_COMMITS).to_string();
    let args = [
        "log",
        "-z",
        "--topo-order",
        "-n",
        &count,
        "--branches",
        "--remotes",
        "--tags",
        "HEAD",
        FORMAT,
    ];
    repo.text(&args, "read the log")
        .map(|text| parse(&text))
        .unwrap_or_default()
}

fn parse(text: &str) -> Vec<Value> {
    text.split('\0').filter_map(commit).collect()
}

fn commit(record: &str) -> Option<Value> {
    let mut fields = record.trim_start_matches('\n').splitn(7, '\x1f');
    let mut next = || fields.next();
    let (sha, short, parents) = (next()?, next()?, next()?);
    let (author, time, refs, subject) = (next()?, next()?, next()?, next()?);
    Some(json!({
        "sha": sha,
        "short": short,
        "parents": parents.split_whitespace().collect::<Vec<_>>(),
        "author": author,
        "time": time.parse::<i64>().unwrap_or_default(),
        "refs": names(refs),
        "subject": subject,
    }))
}

/// `HEAD -> main, origin/main, tag: v1` as `main`, `origin/main`, `v1`.
fn names(refs: &str) -> Vec<&str> {
    refs.split(", ")
        .map(|name| name.strip_prefix("HEAD -> ").unwrap_or(name))
        .map(|name| name.strip_prefix("tag: ").unwrap_or(name))
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_commit_is_read_with_its_parents_and_refs() {
        let text = "aaa\x1fa\x1fbbb ccc\x1fAda\x1f1700\x1fHEAD -> main, origin/main, tag: v1\x1fMerge\0\
                    \nbbb\x1fb\x1f\x1fAda\x1f1600\x1f\x1fFirst, with\x1fodd text";
        let commits = parse(text);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0]["parents"], json!(["bbb", "ccc"]));
        assert_eq!(commits[0]["refs"], json!(["main", "origin/main", "v1"]));
        assert_eq!(commits[0]["time"], 1700);
        assert_eq!(commits[1]["parents"], json!([]));
        assert_eq!(commits[1]["refs"], json!([]));
        assert_eq!(commits[1]["subject"], "First, with\x1fodd text");
    }

    #[test]
    fn nothing_readable_is_no_commits() {
        assert!(parse("").is_empty());
        assert!(parse("half\x1fa record").is_empty());
    }
}
