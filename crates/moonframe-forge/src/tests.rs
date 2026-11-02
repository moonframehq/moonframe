use {
    super::*,
    crate::{
        credentials::{Credential, Remembered, Saved, Source},
        shape::{Avatars, parse_time},
    },
    moonframe_core::Data,
    moonframe_providers::{Event, Handle},
    secrecy::{ExposeSecret, SecretString},
    serde_json::{Value, json},
    std::sync::{Arc, Mutex},
};

fn target(branches: &[&str]) -> Target {
    Target {
        host: "github.com".into(),
        path: "moonframehq/moonframe".into(),
        branch: branches.first().copied().unwrap_or_default().to_owned(),
        branches: branches.iter().map(|branch| (*branch).to_owned()).collect(),
    }
}

#[test]
fn times_read_as_unix_seconds() {
    assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_time("2026-11-01T09:30:00Z"), Some(1_793_525_400));
    assert_eq!(
        parse_time("2026-11-01T11:30:00.123+02:00"),
        Some(1_793_525_400)
    );
    assert_eq!(parse_time("2026-11-01T04:30:00-05:00"), Some(1_793_525_400));
    assert_eq!(parse_time("yesterday"), None);
}

#[test]
fn worktree_branches_are_read_from_porcelain() {
    let list = "worktree /a\nHEAD 1\nbranch refs/heads/main\n\nworktree /b\nHEAD 2\ndetached\n\n\
worktree /c\nHEAD 3\nbranch refs/heads/feature/x\n";
    assert_eq!(repo::worktree_branches(list), ["main", "feature/x"]);
}

#[test]
fn github_answers_become_the_forge_shape() {
    let pr = |number: u64, state: &str, merge: &str, check: Value| {
        json!({
            "number": number, "title": "Add\nforges", "state": state, "isDraft": false,
            "url": format!("https://github.com/o/r/pull/{number}"), "headRefName": "forge",
            "baseRefName": "main", "updatedAt": "2026-11-01T09:30:00Z", "additions": 120,
            "deletions": 8, "changedFiles": 4, "reviewDecision": "REVIEW_REQUIRED",
            "mergeable": "MERGEABLE", "mergeStateStatus": merge,
            "author": { "__typename": "User", "login": "penso", "avatarUrl": "https://a/x" },
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "contexts": { "nodes": [check] } } } }] },
        })
    };
    let passing = json!({ "__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS", "detailsUrl": "https://ci" });
    let running = json!({ "__typename": "CheckRun", "name": "ci", "status": "IN_PROGRESS", "conclusion": null });
    let data = json!({
        "viewer": { "login": "penso", "name": "Fabien", "avatarUrl": "https://a/x" },
        "repository": {
            "nameWithOwner": "o/r", "url": "https://github.com/o/r", "defaultBranchRef": { "name": "main" },
            "pullRequests": { "nodes": [pr(7, "OPEN", "BLOCKED", running.clone())] },
            "issues": { "nodes": [{ "number": 3, "title": "Bug", "url": "https://github.com/o/r/issues/3",
                "updatedAt": "2026-11-01T09:30:00Z", "author": { "__typename": "Bot", "login": "dependabot" },
                "labels": { "nodes": [{ "name": "bug", "color": "d73a4a" }] },
                "assignees": { "nodes": [{ "login": "penso" }] } }] },
            "b0": { "nodes": [pr(9, "OPEN", "CLEAN", passing)] },
            "b1": { "nodes": [] },
        },
    });
    let folder = tempfile::tempdir().unwrap();
    let avatars = Avatars {
        folder: folder.path(),
        prefix: String::new(),
    };
    let out = github::shape(&data, &target(&["forge", "other"]), &avatars);
    assert_eq!(out["repo"]["default_branch"], "main");
    assert_eq!(out["viewer"]["login"], "penso");
    assert_eq!(out["prs"][0]["tone"], "waiting");
    assert_eq!(out["prs"][0]["title"], "Add forges");
    assert_eq!(out["prs"][0]["checks"]["pending"], 1);
    let current = &out["current_pr"];
    assert_eq!(current["number"], 9);
    assert_eq!(current["state"], "open");
    assert_eq!(
        (current["additions"].clone(), current["deletions"].clone()),
        (json!(120), json!(8))
    );
    assert_eq!(current["check_list"][0]["outcome"], "passed");
    assert_eq!(out["branches"]["forge"]["number"], 9);
    assert!(out["branches"].get("other").is_none());
    assert_eq!(out["issues"][0]["author"]["bot"], true);
    assert_eq!(out["issues"][0]["labels"][0]["name"], "bug");
    assert_eq!(out["issues"][0]["updated"], 1_793_525_400);
}

#[test]
fn gitlab_answers_become_the_forge_shape() {
    let mr = json!({
        "iid": 3, "title": "Forges", "state": "opened", "draft": false,
        "web_url": "https://gitlab.com/g/s/r/-/merge_requests/3", "source_branch": "forge",
        "target_branch": "main", "updated_at": "2026-11-01T09:30:00Z",
        "detailed_merge_status": "mergeable", "has_conflicts": false,
        "author": { "username": "penso", "name": "Fabien", "avatar_url": "https://gitlab.com/a.png" },
    });
    let mut full = mr.clone();
    full["head_pipeline"] = json!({ "id": 5, "status": "failed" });
    let details = gitlab::Details {
        mr: Some(full),
        jobs: vec![
            json!({ "name": "test", "status": "failed", "web_url": "https://gitlab.com/j" }),
        ],
        lines: Some((10, 2)),
    };
    let folder = tempfile::tempdir().unwrap();
    let avatars = Avatars {
        folder: folder.path(),
        prefix: "gitlab.com-".into(),
    };
    let target = Target {
        host: "gitlab.com".into(),
        path: "g/s/r".into(),
        branch: "forge".into(),
        branches: vec!["forge".into()],
    };
    let out = gitlab::shape(
        &json!({ "username": "penso" }),
        &json!({ "path_with_namespace": "g/s/r", "web_url": "https://gitlab.com/g/s/r", "default_branch": "main" }),
        &json!([mr]),
        &json!([]),
        &[("forge".into(), mr.clone(), details)],
        &target,
        &avatars,
    );
    assert_eq!(out["prs"][0]["tone"], "ready");
    assert_eq!(out["prs"][0]["state"], "open");
    let current = &out["current_pr"];
    assert_eq!(current["tone"], "attention");
    assert_eq!(current["checks"]["failed"], 1);
    assert_eq!(current["additions"], 10);
    assert_eq!(current["check_list"][0]["name"], "test");
    assert_eq!(gitlab::encode("g/s r"), "g%2Fs%20r");
}

#[test]
fn avatars_only_name_safe_files() {
    let folder = tempfile::tempdir().unwrap();
    let avatars = Avatars {
        folder: folder.path(),
        prefix: String::new(),
    };
    assert!(avatars.file("penso").is_some());
    assert!(avatars.file("../etc").is_none());
    assert!(avatars.file("a/b").is_none());
    assert_eq!(avatars.path("penso"), Value::Null);
    std::fs::write(folder.path().join("penso.png"), b"png").unwrap();
    assert!(
        avatars
            .path("penso")
            .as_str()
            .unwrap()
            .ends_with("penso.png")
    );
}

#[cfg(unix)]
#[test]
fn saved_sign_ins_round_trip_per_host() {
    let folder = tempfile::tempdir().unwrap();
    // The private file, whatever this build's keyring.
    let saved = Saved::new(kind::Kind::GitLab, folder.path().join("state"), true);
    if !matches!(saved.load("gitlab.com"), Ok(None)) {
        // A keyring this test may not touch.
        return;
    }
    let credential = Credential {
        token: SecretString::from("glpat-secret-token"),
        refresh: Some(SecretString::from("refresh-token")),
        expires: Some(42),
        source: Source::Saved,
    };
    saved.save("gitlab.com", &credential).unwrap();
    saved
        .save(
            "git.example.com",
            &Credential::new(SecretString::from("cli-token-x"), Source::Cli),
        )
        .unwrap();
    let Ok(Some(Remembered::Token(back))) = saved.load("gitlab.com") else {
        panic!("not saved");
    };
    assert_eq!(back.token.expose_secret(), "glpat-secret-token");
    assert_eq!(back.expires, Some(42));
    assert!(back.expiring());
    // The CLI's token is never saved, only the choice.
    assert!(matches!(
        saved.load("git.example.com"),
        Ok(Some(Remembered::Cli))
    ));
    saved.forget("gitlab.com").unwrap();
    assert!(matches!(saved.load("gitlab.com"), Ok(None)));
}

#[test]
fn only_account_mods_may_sign_in() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let events: moonframe_providers::Events = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event| seen.lock().unwrap().push(event))
    };
    let folder = tempfile::tempdir().unwrap();
    let settings = Data::map([("accounts", Data::List(vec![Data::from("forge")]))]);
    let host = Handle::new("github", folder.path(), settings, events);
    let mut forge = github();
    forge.start(&host).unwrap();
    forge.receive(
        &host,
        "intruder",
        Data::map([("sign_out", Data::from(true))]),
    );
    // No client id: the sign-in is refused with the way to set one.
    forge.receive(
        &host,
        "forge",
        Data::map([("sign_in", Data::from(true)), ("id", Data::from("me"))]),
    );
    let seen = seen.lock().unwrap();
    let answers: Vec<&Data> = seen
        .iter()
        .filter_map(|event| match event {
            Event::Sent { msg, .. } => Some(msg),
            _ => None,
        })
        .collect();
    assert_eq!(answers.len(), 2, "{answers:?}");
    assert_eq!(answers[0].get("ok"), Some(&Data::from(false)));
    assert!(
        answers[0]
            .get("error")
            .and_then(Data::as_str)
            .unwrap()
            .contains("accounts")
    );
    assert_eq!(answers[1].get("reply"), Some(&Data::from("me")));
    assert!(
        answers[1]
            .get("error")
            .and_then(Data::as_str)
            .unwrap()
            .contains("client id")
    );
    let published = seen.iter().find_map(|event| match event {
        Event::Published { data, .. } => Some(data),
        _ => None,
    });
    let auth = published.and_then(|data| data.get("auth")).expect("auth");
    assert_eq!(auth.get("device"), Some(&Data::from(false)));
}

#[test]
fn gitlab_hosts_are_named_never_guessed() {
    let gitlab = kind::Kind::GitLab;
    assert!(gitlab.serves("gitlab.com", &[]));
    assert!(gitlab.serves("GitLab.com", &[]));
    // A look-alike gets no sign-in, nor the environment's token.
    assert!(!gitlab.serves("gitlab.attacker.example", &[]));
    assert!(!gitlab.env_serves("gitlab.attacker.example"));
    assert!(gitlab.serves("git.example.com", &["git.example.com".to_owned()]));
    assert!(gitlab.env_serves("gitlab.com"));
    assert!(kind::Kind::GitHub.env_serves("github.com"));
}
