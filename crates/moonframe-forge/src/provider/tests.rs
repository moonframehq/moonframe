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
