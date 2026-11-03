use {
    super::*,
    std::{path::Path, process::Command},
};

fn repository(root: &Path, remote: &str) -> PathBuf {
    std::fs::create_dir_all(root).unwrap();
    for args in [
        vec!["init", "-b", "feature"],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "initial",
        ],
        vec!["remote", "add", "origin", remote],
    ] {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    root.canonicalize().unwrap()
}

#[test]
fn a_packaged_app_follows_projects_and_rejects_old_repository_data() {
    let dir = tempfile::tempdir().unwrap();
    let first = repository(&dir.path().join("one"), "https://github.com/owner/one.git");
    let second = repository(&dir.path().join("two"), "https://github.com/owner/two.git");
    let (handle, _) = tests::handle();
    let mut forge = Forge::new(Kind::GitHub);
    forge.look_project(&handle, None);
    assert!(forge.root.is_none());
    forge.look_project(&handle, Some(first.clone()));
    assert_eq!(forge.root.as_ref().unwrap().canonicalize().unwrap(), first);
    assert_eq!(forge.target().unwrap().path, "owner/one");
    forge.status = Status::SignedIn;
    forge.credential = Some(Credential::new(
        SecretString::from("fake-token"),
        Source::Saved,
    ));
    let old = forge.target().unwrap();
    forge.data = json!({ "current_pr": { "number": 1 } });
    forge.fetching = true;
    forge.look_project(&handle, Some(second.clone()));
    assert_eq!(forge.root.as_ref().unwrap().canonicalize().unwrap(), second);
    assert!(forge.data.is_null());
    assert_eq!(forge.target().unwrap().path, "owner/two");
    forge.take_in(&handle, Done::Fetched {
        target: old,
        epoch: forge.epoch,
        result: Ok(json!({ "current_pr": { "number": 1 } })),
        credential: None,
    });
    assert!(forge.data.is_null());
    assert!(forge.due(Instant::now()));
    forge.look_project(&handle, None);
    assert_eq!(forge.root.as_ref().unwrap().canonicalize().unwrap(), second);
}

#[test]
fn a_project_on_another_host_cancels_the_old_sign_in() {
    let dir = tempfile::tempdir().unwrap();
    let first = repository(&dir.path().join("one"), "https://gitlab.com/group/one.git");
    let second = repository(
        &dir.path().join("two"),
        "https://git.company.example/group/two.git",
    );
    let (handle, _) = tests::handle();
    let mut forge = Forge::new(Kind::GitLab);
    forge.settings = json!({ "hosts": ["git.company.example"] });
    forge.look_project(&handle, Some(first));
    let cancel = Arc::new(AtomicBool::new(false));
    forge.cancel = Some(Arc::clone(&cancel));
    forge.waiting = Some(Asker {
        from: "forge".into(),
        id: Data::Nil,
        action: "sign_in",
        attempt: forge.attempt,
    });
    let epoch = forge.epoch;
    forge.look_project(&handle, Some(second));
    assert!(cancel.load(Ordering::Acquire));
    assert!(forge.waiting.is_none());
    assert!(forge.epoch > epoch);
    assert_eq!(forge.host, "git.company.example");
    assert_eq!(forge.status, Status::Checking);
}
