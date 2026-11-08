//! Who may point `app.repo` at a checkout and change it, and what reaches git.

#![allow(clippy::unwrap_used)]

use {
    super::Repo,
    moonframe_core::Data,
    moonframe_providers::Handle,
    serde_json::json,
    std::{
        collections::BTreeMap,
        path::Path,
        process::Command,
        sync::{Arc, atomic::AtomicBool},
    },
};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A checkout of two commits, and the provider pointed at it by the workbench.
fn checkout() -> (tempfile::TempDir, Repo) {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    // The checkout's own identity, which the provider's commit uses too: a
    // machine with none, as CI's, would refuse it.
    git(dir.path(), &["config", "user.name", "t"]);
    git(dir.path(), &["config", "user.email", "t@t"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    for name in ["a", "b"] {
        std::fs::write(dir.path().join(name), name).unwrap();
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-qm", name]);
    }
    let settings = Data::Map(BTreeMap::from([(
        "writers".to_owned(),
        Data::List(vec![Data::from("workbench")]),
    )]));
    let host = Handle::new("repo", dir.path(), settings, Arc::new(|_| {}));
    let mut repo = Repo::new(host, Arc::new(AtomicBool::new(false)));
    let at = dir.path().to_string_lossy().into_owned();
    repo.handle("workbench", &json!({ "at": at }));
    (dir, repo)
}

fn commits(dir: &Path) -> usize {
    git(dir, &["rev-list", "--count", "HEAD"]).parse().unwrap()
}

#[test]
fn a_mod_not_listed_cannot_commit_and_a_writer_can() {
    let (dir, mut repo) = checkout();
    std::fs::write(dir.path().join("c"), "c").unwrap();
    repo.handle("intruder", &json!({ "commit": "sneaked in" }));
    assert_eq!(commits(dir.path()), 2, "the intruder committed");
    assert!(repo.error.as_deref().unwrap().contains("intruder"));
    repo.handle("workbench", &json!({ "commit": "meant" }));
    assert_eq!(
        commits(dir.path()),
        3,
        "the writer's commit: {:?}",
        repo.error
    );
}

#[test]
fn a_mod_not_listed_cannot_point_it_elsewhere() {
    let (dir, mut repo) = checkout();
    repo.handle("intruder", &json!({ "at": "/" }));
    assert_eq!(
        repo.at.as_deref(),
        Some(dir.path().to_string_lossy().as_ref())
    );
}

#[test]
fn a_revision_that_reads_as_an_option_is_refused() {
    let (dir, mut repo) = checkout();
    let written = dir.path().join("written-by-git");
    let option = format!("--output={}", written.display());
    repo.handle(
        "workbench",
        &json!({ "compare": "range", "from": option, "to": "HEAD" }),
    );
    assert!(repo.error.is_some());
    assert!(repo.compare.commits.is_none());
    assert!(!written.exists(), "git took the revision as an option");
}

#[test]
fn a_range_compares_the_commits_it_names() {
    let (dir, mut repo) = checkout();
    repo.handle(
        "workbench",
        &json!({ "compare": "range", "from": "HEAD~1", "to": "HEAD" }),
    );
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    let (_, to) = repo.compare.commits.clone().unwrap();
    assert_eq!(to, head);
    assert_eq!(repo.range(), vec![
        git(dir.path(), &["rev-parse", "HEAD~1"]),
        head
    ]);
}

#[test]
fn only_a_listed_worktree_is_removed() {
    let (dir, mut repo) = checkout();
    let elsewhere = dir.path().join("not-a-worktree");
    std::fs::create_dir(&elsewhere).unwrap();
    let path = elsewhere.to_string_lossy().into_owned();
    repo.handle("workbench", &json!({ "remove_worktree": path }));
    assert!(repo.error.as_deref().unwrap().contains("no worktree"));
    assert!(elsewhere.exists());
}
