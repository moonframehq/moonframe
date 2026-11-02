use {super::*, moonframe_providers::Event, std::sync::Mutex};

fn handle() -> (Handle, Arc<Mutex<Vec<Event>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    let host = Handle::new(
        "github",
        std::path::Path::new("."),
        Data::Nil,
        Arc::new(move |event| seen.lock().unwrap().push(event)),
    );
    (host, events)
}

fn asker(attempt: u64) -> Asker {
    Asker {
        from: "forge".into(),
        id: Data::from("login"),
        action: "sign_in",
        attempt,
    }
}

fn signed_in() -> Forge {
    let mut forge = Forge::new(Kind::GitHub);
    forge.status = Status::SignedIn;
    forge.credential = Some(Credential::new(
        SecretString::from("fake-token"),
        Source::Saved,
    ));
    forge.checkout = Checkout {
        remote: Some(("github.com".into(), "owner/repo".into())),
        branch: "before".into(),
        branches: vec!["before".into()],
    };
    forge
}

#[test]
fn authentication_is_initialized_without_a_checkout() {
    let (host, _) = handle();
    let mut forge = Forge::new(Kind::GitHub);
    forge.look(&host);
    assert_eq!(forge.loaded.as_deref(), Some("github.com"));
    forge.take_in(&host, Done::Loaded {
        host: "github.com".into(),
        epoch: forge.epoch,
        credential: None,
        cli: true,
        error: None,
    });
    let data = forge.snapshot();
    assert_eq!(data["reason"], "not a git repository");
    assert_eq!(data["auth"]["status"], "signed_out");
    assert_eq!(data["auth"]["cli"], true);
}

#[test]
fn expiry_completes_the_attempt_even_if_the_worker_never_answers() {
    let (host, events) = handle();
    let mut forge = Forge::new(Kind::GitHub);
    let cancel = Arc::new(AtomicBool::new(false));
    forge.status = Status::Pending;
    forge.waiting = Some(asker(0));
    forge.cancel = Some(Arc::clone(&cancel));
    forge.code = Some(Shown {
        user: "CODE".into(),
        url: "https://github.com/login/device".into(),
        complete_url: None,
        expires: 100,
    });
    forge.expire_device(&host, 100);
    assert!(cancel.load(Ordering::Acquire));
    assert!(forge.waiting.is_none());
    assert!(forge.code.is_none());
    assert_eq!(forge.status, Status::SignedOut);
    assert_eq!(
        forge.error.as_deref(),
        Some("the code expired; start again")
    );
    assert!(events.lock().unwrap().iter().any(|event| matches!(event,
        Event::Sent { msg, .. } if msg.get("reply") == Some(&Data::from("login"))
            && msg.get("ok") == Some(&Data::Bool(false)))));

    // A new attempt is not cancelled by late events from the expired one.
    forge.status = Status::Pending;
    forge.waiting = Some(asker(forge.attempt));
    forge.take_in(&host, Done::Refused {
        asker: asker(0),
        error: "old failure".into(),
    });
    forge.take_in(&host, Done::Code {
        attempt: 0,
        user: "OLD".into(),
        url: "https://github.com/login/device".into(),
        complete_url: None,
        expires: 200,
    });
    assert_eq!(forge.status, Status::Pending);
    assert!(forge.waiting.is_some());
    assert!(forge.code.is_none());
}

#[test]
fn a_branch_change_discards_the_old_fetch_and_preserves_the_refresh() {
    let (host, _) = handle();
    let mut forge = signed_in();
    let old = forge.target().unwrap();
    forge.fetching = true;
    forge.checkout.branch = "after".into();
    forge.checkout.branches = vec!["after".into()];
    let due = Instant::now();
    forge.next = Some(due);
    forge.take_in(&host, Done::Fetched {
        target: old,
        epoch: forge.epoch,
        result: Ok(json!({ "current_pr": { "number": 1 } })),
        credential: None,
    });
    assert!(forge.data.is_null());
    assert!(!forge.fetching);
    assert_eq!(forge.next, Some(due));
}

#[test]
fn a_refresh_during_a_fetch_still_runs_after_it() {
    let (host, _) = handle();
    let mut forge = signed_in();
    let due = Instant::now();
    forge.next = Some(due);
    forge.take_in(&host, Done::Fetched {
        target: forge.target().unwrap(),
        epoch: forge.epoch,
        result: Ok(json!({ "viewer": { "login": "user" } })),
        credential: None,
    });
    assert_eq!(forge.login.as_deref(), Some("user"));
    assert_eq!(forge.next, Some(due));
}

#[test]
fn an_old_accounts_failure_cannot_sign_out_the_new_account() {
    let (host, _) = handle();
    let mut forge = signed_in();
    forge.epoch = 1;
    forge.take_in(&host, Done::Fetched {
        target: forge.target().unwrap(),
        epoch: 0,
        result: Err(Failure::Unauthorized),
        credential: None,
    });
    assert_eq!(forge.status, Status::SignedIn);
    assert!(forge.credential.is_some());
}

#[test]
fn a_self_hosted_remote_is_identified_before_signing_in() {
    let mut forge = Forge::new(Kind::GitLab);
    forge.settings = json!({ "hosts": ["git.company.example"] });
    forge.checkout.remote = Some(("git.company.example".into(), "group/repo".into()));
    let data = forge.snapshot();
    assert_eq!(data["matches_remote"], true);
    assert_eq!(data["available"], false);
    forge.checkout.remote = Some(("gitlab.attacker.example".into(), "group/repo".into()));
    assert_eq!(forge.snapshot()["matches_remote"], false);
}

#[test]
fn a_refresh_request_waits_out_a_rate_limit() {
    let (host, _) = handle();
    let mut forge = signed_in();
    let target = forge.target().unwrap();
    forge.fetching = true;
    forge.take_in(&host, Done::Fetched {
        target,
        epoch: forge.epoch,
        result: Err(Failure::RateLimited),
        credential: None,
    });
    // What a mod's refresh does.
    forge.next = Some(Instant::now());
    assert!(!forge.due(Instant::now()));
    assert!(forge.due(Instant::now() + RATE_LIMITED + Duration::from_secs(1)));
}

/// A provider whose sign-ins are kept in a private file under `dir`.
#[cfg(unix)]
fn with_store(dir: &std::path::Path) -> (Forge, Arc<Saved>) {
    let mut forge = Forge::new(Kind::GitHub);
    let saved = Arc::new(Saved::new(Kind::GitHub, dir.to_owned(), true));
    let (sender, receiver) = channel();
    forge.context = Some(Context {
        kind: Kind::GitHub,
        sender,
        agent: crate::http::agent(),
        saved: Arc::clone(&saved),
        path: std::ffi::OsString::new(),
        images: dir.join("avatars"),
    });
    forge.done = Some(receiver);
    forge.accounts = Mods::from_iter(["forge"]);
    forge.loaded = Some("github.com".into());
    forge.status = Status::Verifying;
    (forge, saved)
}

#[cfg(unix)]
#[test]
fn a_sign_out_during_the_check_is_not_undone() {
    let (host, _) = handle();
    let dir = tempfile::tempdir().unwrap();
    let (mut forge, saved) = with_store(dir.path());
    let waiting = Asker {
        action: "token",
        ..asker(forge.attempt)
    };
    forge.waiting = Some(waiting.clone());
    forge.act(
        &host,
        "forge",
        &Data::map([("sign_out", Data::from(true))]),
        "sign_out",
    );
    // The check finishes after the sign-out.
    forge.take_in(&host, Done::SignedIn {
        host: "github.com".into(),
        credential: Credential::new(SecretString::from("late-token"), Source::Saved),
        login: "late".into(),
        asker: waiting,
    });
    assert_eq!(forge.status, Status::SignedOut);
    assert!(forge.credential.is_none());
    assert!(
        matches!(saved.load("github.com"), Ok(None)),
        "the token was saved"
    );
}

#[cfg(unix)]
#[test]
fn a_finished_check_saves_the_sign_in() {
    use crate::credentials::Remembered;
    let (host, _) = handle();
    let dir = tempfile::tempdir().unwrap();
    let (mut forge, saved) = with_store(dir.path());
    let waiting = Asker {
        action: "token",
        ..asker(forge.attempt)
    };
    forge.waiting = Some(waiting.clone());
    forge.take_in(&host, Done::SignedIn {
        host: "github.com".into(),
        credential: Credential::new(SecretString::from("good-token"), Source::Saved),
        login: "penso".into(),
        asker: waiting,
    });
    assert_eq!(forge.status, Status::SignedIn);
    assert!(matches!(
        saved.load("github.com"),
        Ok(Some(Remembered::Token(_)))
    ));
}

#[test]
fn a_renewed_token_outlives_a_branch_change() {
    use secrecy::ExposeSecret;
    let (host, _) = handle();
    let mut forge = signed_in();
    let old = forge.target().unwrap();
    forge.fetching = true;
    forge.checkout.branch = "after".into();
    forge.checkout.branches = vec!["after".into()];
    forge.take_in(&host, Done::Fetched {
        target: old,
        epoch: forge.epoch,
        result: Ok(json!({ "viewer": { "login": "penso" } })),
        // No context to save into here: the token is still applied.
        credential: Some(Credential::new(
            SecretString::from("renewed-token"),
            Source::Saved,
        )),
    });
    let kept = forge.credential.as_ref().unwrap();
    assert_eq!(kept.token.expose_secret(), "renewed-token");
    // The old branch's data is not shown.
    assert!(forge.data.is_null());
}

#[test]
fn branch_changes_preserve_account_backoff() {
    for (failure, wait) in [
        (Failure::RateLimited, RATE_LIMITED),
        (Failure::Network("offline".into()), UNREACHABLE),
    ] {
        let (host, _) = handle();
        let mut forge = signed_in();
        let target = forge.target().unwrap();
        forge.fetching = true;
        forge.checkout.branch = "after".into();
        forge.checkout.branches = vec!["after".into()];
        forge.next = Some(Instant::now());
        forge.take_in(&host, Done::Fetched {
            target,
            epoch: forge.epoch,
            result: Err(failure),
            credential: None,
        });
        // Explicit refreshes and further branch changes must still wait.
        forge.next = Some(Instant::now());
        assert!(!forge.due(Instant::now()));
        assert!(forge.due(Instant::now() + wait + Duration::from_secs(1)));
        assert!(!forge.fetching);
        assert_eq!(forge.status, Status::SignedIn);
    }
}

#[test]
fn an_old_accounts_rate_limit_does_not_block_the_new_account() {
    let (host, _) = handle();
    let mut forge = signed_in();
    let old_epoch = forge.epoch;
    forge.epoch += 1;
    forge.next = Some(Instant::now());
    forge.take_in(&host, Done::Fetched {
        target: forge.target().unwrap(),
        epoch: old_epoch,
        result: Err(Failure::RateLimited),
        credential: None,
    });
    assert!(forge.due(Instant::now()));
    assert!(forge.blocked_until.is_none());
}

#[cfg(unix)]
#[test]
fn an_old_load_cannot_restore_a_signed_out_account_after_a_host_round_trip() {
    let (host, _) = handle();
    let dir = tempfile::tempdir().unwrap();
    let (mut forge, _) = with_store(dir.path());
    forge.status = Status::Checking;
    let old = Done::Loaded {
        host: forge.host.clone(),
        epoch: forge.epoch,
        credential: Some(Credential::new(
            SecretString::from("old-token"),
            Source::Cli,
        )),
        cli: true,
        error: Some("old error".into()),
    };
    forge.act(
        &host,
        "forge",
        &Data::map([("sign_out", Data::Bool(true))]),
        "sign_out",
    );
    // Two host changes start newer loads; the original one is still running.
    forge.epoch += 2;
    forge.status = Status::Checking;
    forge.take_in(&host, old);
    assert!(forge.credential.is_none());
    assert!(!forge.cli);
    assert!(forge.error.is_none());
    assert_eq!(forge.status, Status::Checking);
    forge.take_in(&host, Done::Loaded {
        host: forge.host.clone(),
        epoch: forge.epoch,
        credential: None,
        cli: false,
        error: None,
    });
    assert_eq!(forge.status, Status::SignedOut);
}

#[cfg(unix)]
#[test]
fn refresh_discovers_a_new_cli_login_without_selecting_it() {
    use std::os::unix::fs::PermissionsExt;
    let (host, _) = handle();
    let dir = tempfile::tempdir().unwrap();
    let (mut forge, _) = with_store(dir.path());
    forge.status = Status::SignedOut;
    forge.context.as_mut().unwrap().path = dir.path().as_os_str().to_owned();
    let cli = dir.path().join("gh");
    std::fs::write(&cli, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o700)).unwrap();
    for (script, available) in [
        ("#!/bin/sh\nexit 1\n", false),
        ("#!/bin/sh\nprintf new-cli-token\n", true),
        ("#!/bin/sh\nexit 1\n", false),
    ] {
        std::fs::write(&cli, script).unwrap();
        forge.asked_at = None;
        forge.receive(&host, "forge", Data::map([("refresh", Data::Bool(true))]));
        let done = forge
            .done
            .as_ref()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        forge.take_in(&host, done);
        assert_eq!(forge.snapshot()["auth"]["cli"], available);
        assert_eq!(forge.status, Status::SignedOut);
        assert!(forge.credential.is_none());
        assert!(forge.cli_probe.is_none());
    }
}

#[test]
fn an_obsolete_cli_probe_cannot_replace_the_current_one() {
    let (host, _) = handle();
    let mut forge = signed_in();
    let old_epoch = forge.epoch;
    forge.epoch += 1;
    forge.cli_probe = Some(forge.epoch);
    forge.take_in(&host, Done::Cli {
        host: forge.host.clone(),
        epoch: old_epoch,
        available: true,
    });
    assert!(!forge.cli);
    assert_eq!(forge.cli_probe, Some(forge.epoch));
    forge.take_in(&host, Done::Cli {
        host: forge.host.clone(),
        epoch: forge.epoch,
        available: true,
    });
    assert!(forge.cli);
    assert!(forge.cli_probe.is_none());
}
