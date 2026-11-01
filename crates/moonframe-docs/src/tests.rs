use {
    super::*,
    moonframe_providers::Event,
    std::sync::{Arc, Mutex},
};

/// A provider rooted in a fresh folder, whose `writers` may save, and what
/// it sends.
fn docs(writers: &[&str]) -> (Docs, Handle, Arc<Mutex<Vec<Event>>>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let events: moonframe_providers::Events = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event| seen.lock().unwrap().push(event))
    };
    let host = Handle::new("docs", dir.path(), Data::Nil, events);
    let docs = Docs {
        root: Some(dir.path().canonicalize().unwrap()),
        writers: writers.iter().map(|name| (*name).to_owned()).collect(),
        open: BTreeMap::new(),
        uses: 0,
    };
    (docs, host, seen, dir)
}

fn message(fields: &[(&'static str, Data)]) -> Data {
    map(fields.iter().cloned())
}

/// The messages sent to mods since the last call, as (mod, fields).
fn sent(seen: &Mutex<Vec<Event>>) -> Vec<(String, BTreeMap<String, Data>)> {
    std::mem::take(&mut *seen.lock().unwrap())
        .into_iter()
        .filter_map(|event| match event {
            Event::Sent {
                to,
                msg: Data::Map(fields),
                ..
            } => Some((to, fields)),
            _ => None,
        })
        .collect()
}

fn string(fields: &BTreeMap<String, Data>, key: &str) -> String {
    match fields.get(key) {
        Some(Data::String(text)) => text.clone(),
        other => panic!("`{key}` is {other:?}"),
    }
}

#[test]
fn reads_a_file_and_answers_the_mod_that_asked() {
    let (mut docs, host, seen, dir) = docs(&[]);
    fs::create_dir(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join("notes/plan.md"), "# Plan\n").unwrap();
    docs.receive(
        &host,
        "editor/markdown-editor#main-1",
        message(&[("read", "notes/plan.md".into()), ("id", "open".into())]),
    );
    let answers = sent(&seen);
    let [(to, answer)] = answers.as_slice() else {
        panic!("{answers:?}");
    };
    assert_eq!(to, "editor/markdown-editor#main-1");
    assert_eq!(answer.get("ok"), Some(&Data::Bool(true)));
    assert_eq!(string(answer, "reply"), "open");
    assert_eq!(string(answer, "action"), "read");
    assert_eq!(string(answer, "path"), "notes/plan.md");
    assert_eq!(string(answer, "text"), "# Plan\n");
    assert!(string(answer, "version").ends_with(":7"));
}

#[test]
fn refuses_paths_outside_the_project_and_files_that_are_not_text() {
    let (mut docs, host, seen, dir) = docs(&["editor"]);
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "no").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
    fs::write(dir.path().join("binary.md"), [0xff, 0xfe, 0x00]).unwrap();
    fs::write(dir.path().join("big.md"), "x".repeat(MAX_DOCUMENT + 1)).unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git/config"), "[core]").unwrap();
    let cases = [
        ("../secret.md", "not a path in the project"),
        ("/etc/hosts", "not a path in the project"),
        ("./binary.md", "not a path in the project"),
        ("a//b.md", "not a path in the project"),
        (".git/config", "not a path in the project"),
        (".GIT/config", "not a path in the project"),
        ("", "not a path in the project"),
        ("missing.md", "does not exist"),
        ("binary.md", "not a text file"),
        ("big.md", "larger than 1024 KiB"),
    ];
    for (path, expected) in cases {
        docs.receive(&host, "editor", message(&[("read", path.into())]));
        let answers = sent(&seen);
        let error = string(&answers[0].1, "error");
        assert!(error.contains(expected), "{path}: {error}");
        assert_eq!(answers[0].1.get("ok"), Some(&Data::Bool(false)));
    }
    #[cfg(unix)]
    for action in ["read", "write"] {
        docs.receive(
            &host,
            "editor",
            message(&[(action, "link/secret.md".into()), ("text", "x".into())]),
        );
        let error = string(&sent(&seen)[0].1, "error");
        assert!(
            error.contains("leads outside the project"),
            "{action}: {error}"
        );
    }
    // A link into `.git` is refused too: reading its files, saving over
    // one (a hook), or making a new one there.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join(".git"), dir.path().join("hooks")).unwrap();
        for (action, path) in [
            ("read", "hooks/config"),
            ("write", "hooks/config"),
            ("write", "hooks/post-checkout"),
        ] {
            docs.receive(
                &host,
                "editor",
                message(&[(action, path.into()), ("text", "x".into())]),
            );
            let error = string(&sent(&seen)[0].1, "error");
            assert!(
                error.contains("leads into `.git`"),
                "{action} {path}: {error}"
            );
        }
        assert_eq!(
            fs::read_to_string(dir.path().join(".git/config")).unwrap(),
            "[core]"
        );
        assert!(!dir.path().join(".git/post-checkout").exists());
    }
    assert_eq!(
        fs::read_to_string(outside.path().join("secret.md")).unwrap(),
        "no"
    );
    assert!(docs.open.is_empty());
}

#[test]
fn saves_only_for_writers_and_never_over_a_newer_file() {
    let (mut docs, host, seen, dir) = docs(&["editor"]);
    let file = dir.path().join("README.md");
    fs::write(&file, "old").unwrap();

    // Another mod may read but not save.
    docs.receive(
        &host,
        "intruder",
        message(&[("write", "README.md".into()), ("text", "pwned".into())]),
    );
    let error = string(&sent(&seen)[0].1, "error");
    assert!(error.contains("`intruder` may not save"), "{error}");
    // A writer named as a prefix of another mod's name is not that mod.
    docs.receive(
        &host,
        "editorial/markdown-editor#x",
        message(&[("write", "README.md".into()), ("text", "pwned".into())]),
    );
    assert!(string(&sent(&seen)[0].1, "error").contains("may not save"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "old");

    // Saving an existing file needs the version it was read at.
    let from = "editor/markdown-editor#main-1";
    docs.receive(
        &host,
        from,
        message(&[("write", "README.md".into()), ("text", "new".into())]),
    );
    let answer = &sent(&seen)[0].1;
    assert_eq!(answer.get("conflict"), Some(&Data::Bool(true)));
    assert!(string(answer, "error").contains("already exists"));

    docs.receive(&host, from, message(&[("read", "README.md".into())]));
    let version = string(&sent(&seen)[0].1, "version");
    docs.receive(
        &host,
        from,
        message(&[
            ("write", "README.md".into()),
            ("text", "new text".into()),
            ("version", Data::String(version.clone())),
            ("id", "save".into()),
        ]),
    );
    let answer = &sent(&seen)[0].1;
    assert_eq!(answer.get("ok"), Some(&Data::Bool(true)), "{answer:?}");
    assert_eq!(string(answer, "reply"), "save");
    assert_eq!(string(answer, "message"), "Saved README.md");
    let saved = string(answer, "version");
    assert_eq!(fs::read_to_string(&file).unwrap(), "new text");
    // No temporary file is left behind.
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    // Its own save is not reported back as a change.
    docs.tick(&host);
    assert!(sent(&seen).is_empty());

    // The old version is now stale: a save from it is a conflict...
    docs.receive(
        &host,
        from,
        message(&[
            ("write", "README.md".into()),
            ("text", "stale".into()),
            ("version", Data::String(version)),
        ]),
    );
    let answer = &sent(&seen)[0].1;
    assert_eq!(answer.get("conflict"), Some(&Data::Bool(true)));
    assert!(string(answer, "error").contains("changed on disk"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "new text");
    // ...unless the user chose to overwrite.
    docs.receive(
        &host,
        from,
        message(&[
            ("write", "README.md".into()),
            ("text", "forced".into()),
            ("force", Data::Bool(true)),
        ]),
    );
    assert_eq!(sent(&seen)[0].1.get("ok"), Some(&Data::Bool(true)));
    assert_eq!(fs::read_to_string(&file).unwrap(), "forced");
    assert_ne!(saved, "");

    // A new file in an existing folder is created.
    docs.receive(
        &host,
        "editor",
        message(&[("write", "NEW.md".into()), ("text", "hi".into())]),
    );
    assert_eq!(sent(&seen)[0].1.get("ok"), Some(&Data::Bool(true)));
    assert_eq!(fs::read_to_string(dir.path().join("NEW.md")).unwrap(), "hi");
    docs.receive(
        &host,
        "editor",
        message(&[("write", "nope/NEW.md".into()), ("text", "hi".into())]),
    );
    assert!(string(&sent(&seen)[0].1, "error").contains("does not exist"));
}

#[test]
#[cfg(unix)]
fn saving_keeps_the_file_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let (mut docs, host, seen, dir) = docs(&["editor"]);
    let file = dir.path().join("run.md");
    fs::write(&file, "a").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
    docs.receive(&host, "editor", message(&[("read", "run.md".into())]));
    let version = string(&sent(&seen)[0].1, "version");
    docs.receive(
        &host,
        "editor",
        message(&[
            ("write", "run.md".into()),
            ("text", "b".into()),
            ("version", Data::String(version)),
        ]),
    );
    assert_eq!(sent(&seen)[0].1.get("ok"), Some(&Data::Bool(true)));
    let mode = file.metadata().unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640);
}

#[test]
fn tells_readers_when_a_file_changes_or_goes() {
    let (mut docs, host, seen, dir) = docs(&[]);
    let file = dir.path().join("a.md");
    fs::write(&file, "one").unwrap();
    docs.receive(&host, "viewer", message(&[("read", "a.md".into())]));
    sent(&seen);
    docs.tick(&host);
    assert!(sent(&seen).is_empty());

    fs::write(&file, "three").unwrap();
    docs.tick(&host);
    let changes = sent(&seen);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, "viewer");
    assert_eq!(string(&changes[0].1, "changed"), "a.md");
    assert!(string(&changes[0].1, "version").ends_with(":5"));
    docs.tick(&host);
    assert!(sent(&seen).is_empty());

    fs::remove_file(&file).unwrap();
    docs.tick(&host);
    assert_eq!(sent(&seen)[0].1.get("missing"), Some(&Data::Bool(true)));

    // Closed, it is no longer watched.
    docs.receive(&host, "viewer", message(&[("close", "a.md".into())]));
    fs::write(&file, "back").unwrap();
    docs.tick(&host);
    assert!(sent(&seen).is_empty());
}

#[test]
fn the_root_is_the_repository_holding_the_app() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    let provider = dir.path().join("app/providers/docs");
    fs::create_dir_all(&provider).unwrap();
    // LAYOUTS_REPO, which git and the forges watch, doesn't move it: the
    // explorer lists this same root.
    if std::env::var_os("MOONFRAME_FILES").is_none() {
        assert_eq!(root(&provider), Some(dir.path().canonicalize().unwrap()));
    }
    assert_eq!(
        writers(&map([("writers", Data::List(vec!["editor".into()]))])),
        ["editor"]
    );
}

#[test]
fn a_save_made_elsewhere_meanwhile_is_refused_not_lost() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "theirs").unwrap();
    // Checked at one version; by the time it is put in place, another.
    let stale = Over::Version("0:1".to_owned());
    assert!(matches!(
        save(&file, "mine", &stale),
        Err(Saved::Conflict(CHANGED))
    ));
    // A new file, when one appeared meanwhile.
    assert!(matches!(
        save(&file, "mine", &Over::Nothing),
        Err(Saved::Conflict(EXISTS))
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "theirs");
    // Nothing is left beside it.
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);

    let current = Over::Version(version(&file.metadata().unwrap()));
    assert!(save(&file, "mine", &current).is_ok());
    assert_eq!(fs::read_to_string(&file).unwrap(), "mine");
    let new = dir.path().join("b.md");
    assert!(save(&new, "new", &Over::Nothing).is_ok());
    assert_eq!(fs::read_to_string(&new).unwrap(), "new");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn past_the_limit_the_oldest_watch_makes_room() {
    let (mut docs, host, seen, dir) = docs(&["editor"]);
    for index in 0..=MAX_OPEN {
        fs::write(dir.path().join(format!("{index}.md")), "x").unwrap();
    }
    for index in 0..MAX_OPEN {
        let path = format!("{index}.md");
        docs.receive(&host, "editor", message(&[("read", path.into())]));
    }
    // Read again, the first is the newest; the second is now the oldest.
    docs.receive(&host, "editor", message(&[("read", "0.md".into())]));
    let last = format!("{MAX_OPEN}.md");
    docs.receive(&host, "editor", message(&[("read", last.clone().into())]));
    sent(&seen);
    assert_eq!(docs.open.len(), MAX_OPEN);
    assert!(
        !docs
            .open
            .contains_key(&("editor".to_owned(), "1.md".to_owned()))
    );

    // The newest read is still followed.
    fs::write(dir.path().join(&last), "changed").unwrap();
    fs::write(dir.path().join("0.md"), "changed").unwrap();
    docs.tick(&host);
    let mut changed: Vec<String> = sent(&seen)
        .iter()
        .map(|(_, fields)| string(fields, "changed"))
        .collect();
    changed.sort();
    assert_eq!(changed, ["0.md".to_owned(), last]);
}

#[test]
fn a_save_that_lands_after_the_last_look_is_swapped_back() {
    // `replace` looks only once the new text is in place: here the file
    // has already moved on, as when a save elsewhere lands between the last
    // look and the swap.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.md");
    let temp = dir.path().join(".a.md.tmp");
    fs::write(&file, "theirs").unwrap();
    fs::write(&temp, "mine").unwrap();
    assert!(matches!(
        replace(&temp, &file, "0:1"),
        Err(Saved::Conflict(CHANGED))
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "theirs");

    let current = version(&file.metadata().unwrap());
    replace(&temp, &file, &current).unwrap_or_else(|_| panic!("not replaced"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "mine");
    assert!(!temp.exists());
}

#[test]
fn without_a_swap_the_file_is_moved_aside_and_only_ever_linked_over_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.md");
    let temp = dir.path().join(".a.md.tmp");
    fs::write(&file, "theirs").unwrap();
    fs::write(&temp, "mine").unwrap();
    assert!(matches!(
        replace_aside(&temp, &file, "0:1"),
        Err(Saved::Conflict(CHANGED))
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "theirs");
    // Gone meanwhile: nothing is written.
    let gone = dir.path().join("gone.md");
    assert!(matches!(
        replace_aside(&temp, &gone, "0:1"),
        Err(Saved::Conflict(CHANGED))
    ));
    assert!(!gone.exists());

    let current = version(&file.metadata().unwrap());
    replace_aside(&temp, &file, &current).unwrap_or_else(|_| panic!("not replaced"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "mine");
    // Only the file is left: nothing aside, no temporary one.
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn placing_never_replaces_a_file_and_the_original_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("from");
    let to = dir.path().join("to");
    fs::write(&from, "mine").unwrap();
    fs::write(&to, "theirs").unwrap();
    let error = place(&from, &to).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&to).unwrap(), "theirs");
    assert_eq!(fs::read_to_string(&from).unwrap(), "mine");

    // Put back where nothing is; when a newer one took the place, it stays.
    let aside = dir.path().join("aside");
    fs::write(&aside, "original").unwrap();
    let gone = dir.path().join("gone");
    put_back(&aside, &gone, false).unwrap();
    assert_eq!(fs::read_to_string(&gone).unwrap(), "original");
    assert!(!aside.exists());
    // The original this save was allowed to replace is dropped...
    fs::write(&aside, "older").unwrap();
    put_back(&aside, &to, true).unwrap();
    assert_eq!(fs::read_to_string(&to).unwrap(), "theirs");
    assert!(!aside.exists());
    // ...but a save from elsewhere never is: it is kept, and said where.
    fs::write(&aside, "another save").unwrap();
    let error = put_back(&aside, &to, false).unwrap_err();
    assert!(error.to_string().contains("kept at"), "{error}");
    assert_eq!(fs::read_to_string(&aside).unwrap(), "another save");
    assert_eq!(fs::read_to_string(&to).unwrap(), "theirs");
}
