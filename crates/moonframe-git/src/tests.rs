use {
    super::*,
    moonframe_providers::Event,
    std::{process::Command, sync::Mutex},
};

#[test]
fn status_reads_branch_files_and_lines() {
    let state = b"# branch.oid 1234\0# branch.head main\0# branch.upstream origin/main\0\
# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 aaa bbb src/a file.rs\0\
1 A. N... 000000 100644 100644 000 ccc new.rs\0\
2 R. N... 100644 100644 100644 ddd ddd R100 moved.rs\0old.rs\0\
u UU N... 100644 100644 100644 100644 e f g both.rs\0\
? notes.txt\0";
    let numstat =
        Numstat::parse(b"3\t1\tsrc/a file.rs\x005\t0\tnew.rs\0-\t-\t\0old.rs\0moved.rs\0");
    let status = Status::parse(state, &numstat);
    assert_eq!(status.branch, "main");
    assert_eq!(status.upstream.as_deref(), Some("origin/main"));
    assert_eq!((status.ahead, status.behind, status.changed), (2, 1, 5));
    assert_eq!((status.added, status.deleted), (8, 1));
    let kinds: Vec<_> = status
        .files
        .iter()
        .map(|file| file["status"].clone())
        .collect();
    assert_eq!(kinds, [
        json!("M"),
        json!("A"),
        json!("R"),
        json!("U"),
        json!("?")
    ]);
    assert_eq!(status.files[0]["path"], "src/a file.rs");
    assert_eq!(status.files[0]["staged"], false);
    assert_eq!(status.files[0]["unstaged"], true);
    assert_eq!(status.files[0]["added"], 3);
    assert_eq!(status.files[1]["staged"], true);
    assert_eq!(status.files[2]["old_path"], "old.rs");
    assert_eq!(status.files[4]["added"], 0);
}

#[test]
fn a_detached_head_is_named_head() {
    let status = Status::parse(
        b"# branch.oid (initial)\0# branch.head (detached)\0",
        &Numstat::default(),
    );
    assert!(status.detached);
    assert_eq!(status.branch, "HEAD");
    assert_eq!(status.oid, None);
}

#[test]
fn remotes_name_their_host_owner_and_repository() {
    let ssh = remote_json("origin", "git@github.com:moonframehq/moonframe.git");
    assert_eq!(ssh["kind"], "github");
    assert_eq!(
        (ssh["owner"].clone(), ssh["repo"].clone()),
        (json!("moonframehq"), json!("moonframe"))
    );
    assert_eq!(ssh["web_url"], "https://github.com/moonframehq/moonframe");
    let https = remote_json("up", "https://user@gitlab.example.com:8443/group/sub/app/");
    assert_eq!(https["kind"], "gitlab");
    assert_eq!(https["host"], "gitlab.example.com");
    assert_eq!(https["owner"], "group/sub");
    assert_eq!(
        https["url"],
        "https://gitlab.example.com:8443/group/sub/app/"
    );
    let token = remote_json("t", "https://x-access-token:ghp_secret@github.com/a/b.git");
    assert_eq!(token["url"], "https://github.com/a/b.git");
    assert!(!token.to_string().contains("ghp_secret"));
    let local = remote_json("disk", "/srv/repo");
    assert_eq!(local["kind"], "other");
    assert!(local.get("web_url").is_none());
}

#[test]
fn tracking_counts_ahead_behind_and_gone() {
    assert_eq!(refs::track("ahead 2, behind 1"), (2, 1, false));
    assert_eq!(refs::track("behind 3"), (0, 3, false));
    assert_eq!(refs::track("gone"), (0, 0, true));
    assert_eq!(refs::track(""), (0, 0, false));
}

#[test]
fn worktrees_are_read_from_porcelain() {
    let text = "worktree /a\0HEAD 111\0branch refs/heads/main\0\0worktree /b\0HEAD 222\0detached\0locked\0\0";
    let worktrees = refs::parse_worktrees(text);
    assert_eq!(worktrees.len(), 2);
    assert_eq!(worktrees[0].branch, "main");
    assert!(worktrees[1].detached && worktrees[1].locked);
}

#[test]
fn a_diff_becomes_numbered_hunks() {
    let text = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -3,2 +3,3 @@ fn\n keep\n-old\n+new\n+more\n\\ No newline at end of file\n";
    let diff = diff::parse("x", text);
    assert_eq!(diff["lines"], 4);
    let lines = &diff["hunks"][0]["lines"];
    assert_eq!(
        lines[0],
        json!({ "kind": "ctx", "old": 3, "new": 3, "text": "keep" })
    );
    assert_eq!(lines[1], json!({ "kind": "del", "old": 4, "text": "old" }));
    assert_eq!(lines[2], json!({ "kind": "add", "new": 4, "text": "new" }));
    assert_eq!(lines[3], json!({ "kind": "add", "new": 5, "text": "more" }));
}

/// provider.toml's settings with these `writers`.
fn writers(names: &[&str]) -> Data {
    Data::Map(BTreeMap::from([(
        "writers".to_owned(),
        Data::List(names.iter().map(|name| Data::from(*name)).collect()),
    )]))
}

/// A repository of the test's own, with an identity and no signing or
/// hooks, so a commit never waits on the user's key or config.
fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Moonframe Test"),
        ("user.email", "test@moonframe.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "/dev/null"),
    ] {
        git(&["config", key, value]);
    }
    std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "first"]);
    dir
}

fn events() -> (moonframe_providers::Events, Arc<Mutex<Vec<Event>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    (
        Arc::new(move |event| sink.lock().unwrap().push(event)),
        seen,
    )
}

fn published(seen: &Mutex<Vec<Event>>) -> Value {
    seen.lock()
        .unwrap()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Published { data, .. } => Some(moonframe_providers::to_json(data)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn a_repository_is_published_and_a_commit_answers_its_mod() {
    let dir = repository();
    std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "new\n").unwrap();
    let (events, seen) = events();
    // The provider's folder is two below the repository, as in an app.
    let folder = dir.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&["editor", "status"]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();

    let state = published(&seen);
    assert_eq!(state["branch"], "main");
    assert_eq!(state["changed"], 2);
    assert_eq!(
        (state["added"].clone(), state["deleted"].clone()),
        (json!(1), json!(0))
    );
    assert_eq!(state["head"]["subject"], "first");
    assert_eq!(state["worktrees"][0]["current"], true);
    let root = Data::from(state["root"].as_str().unwrap());

    git.receive(
        &host,
        "editor",
        Data::Map(BTreeMap::from([("diff".to_owned(), Data::from("a.txt"))])),
    );
    assert_eq!(
        published(&seen)["diff"]["hunks"][0]["lines"][1]["text"],
        "two"
    );
    // An untracked file diffs against nothing.
    git.receive(
        &host,
        "editor",
        Data::Map(BTreeMap::from([("diff".to_owned(), Data::from("b.txt"))])),
    );
    assert_eq!(
        published(&seen)["diff"]["hunks"][0]["lines"][0],
        json!({ "kind": "add", "new": 1, "text": "new" })
    );

    git.receive(
        &host,
        "editor",
        Data::Map(BTreeMap::from([
            ("commit".to_owned(), Data::from("second")),
            ("root".to_owned(), root.clone()),
            ("id".to_owned(), Data::from("c1")),
        ])),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let reply = loop {
        let found = seen.lock().unwrap().iter().find_map(|event| match event {
            Event::Sent { to, msg, .. } => Some((to.clone(), moonframe_providers::to_json(msg))),
            _ => None,
        });
        if let Some(found) = found {
            break found;
        }
        assert!(Instant::now() < deadline, "no reply to the commit");
        thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(reply.0, "editor");
    assert_eq!(reply.1["reply"], "c1");
    assert_eq!(reply.1["ok"], true, "{}", reply.1);

    // The next tick looks again, and finds the tree clean.
    git.tick(&host);
    let state = published(&seen);
    assert_eq!(state["changed"], 0);
    assert_eq!(state["head"]["subject"], "second");

    // Nothing left to commit is an answer too, not a silence.
    seen.lock().unwrap().clear();
    git.receive(
        &host,
        "editor",
        Data::Map(BTreeMap::from([
            ("commit".to_owned(), Data::from("again")),
            ("root".to_owned(), root),
        ])),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let reply = seen.lock().unwrap().iter().find_map(|event| match event {
            Event::Sent { msg, .. } => Some(moonframe_providers::to_json(msg)),
            _ => None,
        });
        if let Some(reply) = reply {
            assert_eq!(reply["ok"], false);
            assert_eq!(reply["reply"], "commit");
            assert!(
                reply["error"]
                    .as_str()
                    .unwrap()
                    .contains("Nothing to commit"),
                "{reply}"
            );
            break;
        }
        assert!(Instant::now() < deadline, "no reply to the empty commit");
        thread::sleep(Duration::from_millis(50));
    }
}

/// A push that git refuses (here, no `origin`) answers with git's reason.
#[test]
fn a_failed_push_answers_with_the_reason() {
    let dir = repository();
    let (events, seen) = events();
    let folder = dir.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&["editor", "status"]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    let root = Data::from(published(&seen)["root"].as_str().unwrap());
    git.receive(
        &host,
        "status",
        Data::Map(BTreeMap::from([
            ("push".to_owned(), Data::Bool(true)),
            ("root".to_owned(), root),
            ("id".to_owned(), Data::from("p1")),
        ])),
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let reply = seen.lock().unwrap().iter().find_map(|event| match event {
            Event::Sent { to, msg, .. } => Some((to.clone(), moonframe_providers::to_json(msg))),
            _ => None,
        });
        if let Some((to, reply)) = reply {
            assert_eq!(to, "status");
            assert_eq!(reply["reply"], "p1");
            assert_eq!(reply["action"], "push");
            assert_eq!(reply["ok"], false);
            assert!(reply["error"].as_str().unwrap().contains("push"), "{reply}");
            break;
        }
        assert!(Instant::now() < deadline, "no reply to the push");
        thread::sleep(Duration::from_millis(50));
    }
}

/// A mod not named in `writers` is refused, and nothing is committed.
#[test]
fn only_writers_may_commit() {
    let dir = repository();
    std::fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
    let (events, seen) = events();
    let folder = dir.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&["status"]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    git.receive(
        &host,
        "stranger",
        Data::Map(BTreeMap::from([(
            "commit".to_owned(),
            Data::from("sneaky"),
        )])),
    );
    let reply = seen.lock().unwrap().iter().find_map(|event| match event {
        Event::Sent { to, msg, .. } => Some((to.clone(), moonframe_providers::to_json(msg))),
        _ => None,
    });
    let (to, reply) = reply.expect("a refusal, at once");
    assert_eq!(to, "stranger");
    assert_eq!(reply["ok"], false);
    assert!(
        reply["error"].as_str().unwrap().contains("may not commit"),
        "{reply}"
    );
    assert_eq!(published(&seen)["changed"], 1, "nothing was committed");
}

/// A mod asking for the log each frame gets one read, not a `git log` a
/// frame: only a new request skips the pace of the refs.
#[test]
fn asking_for_the_same_log_again_reads_nothing() {
    let dir = repository();
    let (events, seen) = events();
    let folder = dir.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&[]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    let ask = |git: &mut Git, count: i64| {
        let message = BTreeMap::from([("log".to_owned(), Data::Integer(count))]);
        git.receive(&host, "history", Data::Map(message));
    };

    ask(&mut git, 10);
    assert_eq!(published(&seen)["log"][0]["subject"], "first");
    // Made after the read: only a look at the refs may show it.
    let status = Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args(["commit", "-q", "--allow-empty", "-m", "second"])
        .status()
        .unwrap();
    assert!(status.success());
    git.log.clear();
    ask(&mut git, 10);
    assert!(git.log.is_empty(), "the same request read the log again");
    ask(&mut git, 20);
    assert_eq!(git.log[0]["subject"], "second");
}

/// A diff only covers files git lists as changed, never a path from
/// outside the repository.
#[test]
fn a_diff_never_reads_outside_the_repository() {
    let dir = repository();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "do not publish\n").unwrap();
    let (events, seen) = events();
    let folder = dir.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, Data::Nil, events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    for path in [
        secret.to_string_lossy().into_owned(),
        "../../../etc/hosts".to_owned(),
    ] {
        git.receive(
            &host,
            "editor",
            Data::Map(BTreeMap::from([(
                "diff".to_owned(),
                Data::from(path.as_str()),
            )])),
        );
        let state = published(&seen);
        assert_eq!(state["diff"]["lines"], 0, "{path}: {}", state["diff"]);
        assert!(!state.to_string().contains("do not publish"));
    }
}

#[test]
fn messages_inside_the_interval_read_the_status_alone() {
    let dir = repository();
    let (events, _seen) = events();
    let folder = dir.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&[]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    let message = || Data::Map(BTreeMap::from([("diff".to_owned(), Data::Bool(false))]));

    // The first message may read everything; the next twenty, sent within
    // the interval, must not, even as files change between them. The last
    // full read is held at now, however slow the machine running this is.
    git.receive(&host, "editor", message());
    git.forced_at = Some(Instant::now() + Duration::from_secs(3600));
    let reads = git.ref_reads;
    for n in 0..20 {
        std::fs::write(dir.path().join("a.txt"), format!("{n}\n")).unwrap();
        git.receive(&host, "editor", message());
    }
    assert_eq!(git.ref_reads, reads);
    // What moved meanwhile is read at the next look.
    for _ in 0..LOOK_EVERY {
        git.tick(&host);
    }
    assert_eq!(git.ref_reads, reads + 1);
}

#[test]
fn it_moves_to_the_project_the_user_works_in() {
    if std::env::var_os("LAYOUTS_REPO").is_some() {
        return;
    }
    let (first, second) = (repository(), repository());
    std::fs::write(second.path().join("c.txt"), "new\n").unwrap();
    let (events, seen) = events();
    let folder = first.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&["status"]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    assert_eq!(published(&seen)["changed"], 0);

    assert!(git.follow(Some(second.path().to_owned())));
    git.refresh(&host, true);
    let state = published(&seen);
    assert_eq!(state["changed"], 1);
    assert_eq!(state["files"][0]["path"], "c.txt");
    // Asked again, it stays; a folder outside any repository moves nothing.
    assert!(!git.follow(Some(second.path().to_owned())));
    let plain = tempfile::tempdir().unwrap();
    assert!(!git.follow(Some(plain.path().to_owned())));

    // A commit asked for the first repository, or for none, is refused
    // with an answer, and nothing is committed in the second.
    let first_root = Repo::at(first.path())
        .text(&["rev-parse", "--show-toplevel"], "find the repository")
        .unwrap();
    for (id, root) in [("old", Some(first_root)), ("none", None)] {
        seen.lock().unwrap().clear();
        let mut fields = BTreeMap::from([
            ("commit".to_owned(), Data::from("wrong place")),
            ("id".to_owned(), Data::from(id)),
        ]);
        if let Some(root) = root {
            fields.insert("root".to_owned(), Data::String(root));
        }
        git.receive(&host, "status", Data::Map(fields));
        let reply = seen.lock().unwrap().iter().find_map(|event| match event {
            Event::Sent { msg, .. } => Some(moonframe_providers::to_json(msg)),
            _ => None,
        });
        let reply = reply.expect("a refusal, at once");
        assert_eq!(reply["reply"], id);
        assert_eq!(reply["ok"], false, "{reply}");
        assert_eq!(published(&seen)["changed"], 1, "nothing was committed");
    }
}
