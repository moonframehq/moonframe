use {
    crate::{
        Git, Repo,
        tests::{events, published, repository, writers},
    },
    moonframe_core::Data,
    moonframe_providers::{Event, Handle, Native},
    serde_json::Value,
    std::{collections::BTreeMap, path::Path, sync::Mutex},
};

/// The repository's top, as git names it (through the link macOS puts in
/// front of temporary folders).
fn top(path: &Path) -> String {
    Repo::at(path)
        .text(&["rev-parse", "--show-toplevel"], "find the repository")
        .unwrap()
}

/// Sends `{ open = folder, id = "open" }` as `from` and returns its answer.
fn open(git: &mut Git, host: &Handle, seen: &Mutex<Vec<Event>>, from: &str, folder: &str) -> Value {
    seen.lock().unwrap().clear();
    git.receive(
        host,
        from,
        Data::Map(BTreeMap::from([
            ("open".to_owned(), Data::from(folder)),
            ("id".to_owned(), Data::from("open")),
        ])),
    );
    let reply = seen.lock().unwrap().iter().find_map(|event| match event {
        Event::Sent { to, msg, .. } if to == from => Some(moonframe_providers::to_json(msg)),
        _ => None,
    });
    reply.expect("an answer, at once")
}

/// An app started outside any repository (from the Finder, in `/`) has
/// none until a mod opens one; from then on that one is watched.
#[test]
fn a_writer_opens_a_repository_when_there_is_none() {
    if std::env::var_os("LAYOUTS_REPO").is_some() {
        return;
    }
    let (plain, chosen) = (tempfile::tempdir().unwrap(), repository());
    std::fs::write(chosen.path().join("c.txt"), "new\n").unwrap();
    std::fs::create_dir(chosen.path().join("sub")).unwrap();
    let (events, seen) = events();
    let folder = plain.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&["opener"]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    assert_eq!(published(&seen)["repository"], false);

    // A folder inside the repository opens the repository.
    let inside = chosen.path().join("sub");
    let reply = open(&mut git, &host, &seen, "opener", inside.to_str().unwrap());
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(reply["action"], "open");
    assert_eq!(reply["reply"], "open");
    assert_eq!(reply["message"], top(chosen.path()));
    let state = published(&seen);
    assert_eq!(state["repository"], true);
    assert_eq!(state["root"], top(chosen.path()));
    assert_eq!(state["files"][0]["path"], "c.txt");

    // The next tick stays there: the project followed did not change.
    git.tick(&host);
    assert_eq!(published(&seen)["root"], top(chosen.path()));
}

/// A folder that is not in a repository, a relative path or a folder that
/// does not exist is refused with the reason, and nothing moves.
#[test]
fn what_is_not_a_repository_is_refused_with_the_reason() {
    let (dir, plain) = (repository(), tempfile::tempdir().unwrap());
    let (events, seen) = events();
    let folder = dir.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&["opener"]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    let root = published(&seen)["root"].clone();

    let missing = plain.path().join("gone");
    for (folder, reason) in [
        (plain.path().to_str().unwrap(), "is not a git repository"),
        ("relative/path", "is not a full path"),
        (missing.to_str().unwrap(), "is not a folder"),
        ("", "say which folder"),
    ] {
        let reply = open(&mut git, &host, &seen, "opener", folder);
        assert_eq!(reply["ok"], false, "{reply}");
        assert!(
            reply["error"].as_str().unwrap().contains(reason),
            "{folder}: {reply}"
        );
        git.refresh(&host, true);
        assert_eq!(published(&seen)["root"], root, "it stayed");
    }
}

/// Opening changes what every mod reads and where every write goes: a mod
/// outside `writers` is refused.
#[test]
fn only_writers_may_open_a_repository() {
    let (dir, other) = (repository(), repository());
    let (events, seen) = events();
    let folder = dir.path().join("providers").join("git");
    std::fs::create_dir_all(&folder).unwrap();
    let host = Handle::new("git", &folder, writers(&["opener"]), events);
    let mut git = Git::default();
    git.start(&host).unwrap();
    let root = published(&seen)["root"].clone();

    let reply = open(
        &mut git,
        &host,
        &seen,
        "stranger",
        other.path().to_str().unwrap(),
    );
    assert_eq!(reply["ok"], false);
    assert!(
        reply["error"].as_str().unwrap().contains("may not open"),
        "{reply}"
    );
    git.refresh(&host, true);
    assert_eq!(published(&seen)["root"], root, "it stayed");
}
