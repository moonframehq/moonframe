//! Recent commits with their parents, for a mod that draws history. Read
//! only once a mod asks, since no one else pays for it.

use {
    crate::Repo,
    serde_json::{Value, json},
};

/// The most commits a mod can ask for.
pub(crate) const MAX_COMMITS: usize = 500;

/// Fields of one commit. NUL parts them, as `-z` parts commits: it is the one
/// thing an author's name or a subject cannot hold, so none can pose as the
/// next field.
const FORMAT: &str = "--format=%H%x00%h%x00%P%x00%an%x00%at%x00%D%x00%s";
const FIELDS: usize = 7;

/// The newest `limit` commits of every branch, tag and HEAD, children
/// before their parents as a graph needs them; nothing before a first
/// commit.
pub(crate) fn commits(repo: &Repo, limit: usize) -> Vec<Value> {
    let count = limit.clamp(1, MAX_COMMITS).to_string();
    // A repository's own config can set `log.showSignature`, and with it the
    // program git runs to check a signature: reading must never start one.
    let args = [
        "log",
        "--no-show-signature",
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
    let fields: Vec<&str> = text.split('\0').collect();
    let (records, _) = fields.as_chunks::<FIELDS>();
    records.iter().filter_map(commit).collect()
}

/// One commit; `None` for a record that is not what `FORMAT` writes.
fn commit(fields: &[&str; FIELDS]) -> Option<Value> {
    let [sha, short, parents, author, time, refs, subject] = fields;
    Some(json!({
        "sha": sha.trim_start_matches('\n'),
        "short": short,
        "parents": parents.split_whitespace().collect::<Vec<_>>(),
        "author": author,
        "time": time.parse::<i64>().ok()?,
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

    /// A record as `FORMAT` and `-z` write it.
    fn record(fields: [&str; FIELDS]) -> String {
        fields.join("\0")
    }

    #[test]
    fn a_commit_is_read_with_its_parents_and_refs() {
        let merge = record([
            "aaa",
            "a",
            "bbb ccc",
            "Ada",
            "1700",
            "HEAD -> main, origin/main, tag: v1",
            "Merge",
        ]);
        let first = record(["bbb", "b", "", "Ada", "1600", "", "First"]);
        let commits = parse(&format!("{merge}\0\n{first}"));
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0]["parents"], json!(["bbb", "ccc"]));
        assert_eq!(commits[0]["refs"], json!(["main", "origin/main", "v1"]));
        assert_eq!(commits[0]["time"], 1700);
        assert_eq!(commits[1]["sha"], "bbb");
        assert_eq!(commits[1]["parents"], json!([]));
        assert_eq!(commits[1]["refs"], json!([]));
    }

    #[test]
    fn text_in_a_commit_cannot_pose_as_another_field() {
        let name = "Ada\x1f1\x1ftag: forged\x1fFake subject";
        let text = record(["aaa", "a", "", name, "1700", "", "Real\x1fsubject"]);
        let commits = parse(&text);
        assert_eq!(commits[0]["author"], name);
        assert_eq!(commits[0]["time"], 1700);
        assert_eq!(commits[0]["refs"], json!([]));
        assert_eq!(commits[0]["subject"], "Real\x1fsubject");
    }

    #[test]
    fn nothing_readable_is_no_commits() {
        assert!(parse("").is_empty());
        assert!(parse("half\0a record").is_empty());
        let bad_time = record(["aaa", "a", "", "Ada", "soon", "", "Subject"]);
        assert!(parse(&bad_time).is_empty());
    }

    /// A repository can name a program for git to check signatures with. A
    /// read of the log must not run it, though `git log` itself does.
    #[cfg(unix)]
    #[test]
    fn a_repository_cannot_make_reading_its_log_run_a_program() {
        use std::{os::unix::fs::PermissionsExt, process::Command};

        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .unwrap();
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        };
        git(&["init", "-q", "-b", "main"]);
        let tree = git(&["hash-object", "-t", "tree", "-w", "/dev/null"]);
        // git checks only a commit with a signature, so one has to be there.
        let who = "Moonframe Test <test@moonframe.invalid> 1700000000 +0000";
        let signed = format!(
            "tree {tree}\nauthor {who}\ncommitter {who}\n\
             gpgsig -----BEGIN PGP SIGNATURE-----\n \n abcd\n -----END PGP SIGNATURE-----\n\n\
             first\n"
        );
        let object = dir.path().join("object");
        std::fs::write(&object, signed).unwrap();
        let file = object.to_str().unwrap();
        let commit = git(&["hash-object", "-t", "commit", "-w", file]);
        git(&["update-ref", "refs/heads/main", &commit]);

        let ran = dir.path().join("ran");
        let program = dir.path().join("gpg.sh");
        let script = format!("#!/bin/sh\ntouch '{}'\n", ran.display());
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        git(&["config", "log.showSignature", "true"]);
        git(&["config", "gpg.program", program.to_str().unwrap()]);

        git(&["log", "-1"]);
        assert!(ran.exists(), "this repository should run its program");
        std::fs::remove_file(&ran).unwrap();

        assert_eq!(commits(&Repo::at(dir.path()), 10).len(), 1);
        assert!(
            !ran.exists(),
            "reading the log ran the repository's program"
        );
    }
}
