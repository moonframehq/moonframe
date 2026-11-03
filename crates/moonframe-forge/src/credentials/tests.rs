use {super::*, std::os::unix::fs::PermissionsExt};

fn script(folder: &Path, name: &str, body: &str) {
    let path = folder.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn each_fetch_resolves_the_current_cli_account() {
    let folder = tempfile::tempdir().unwrap();
    script(folder.path(), "gh", "printf 'first-token'");
    let initial = cli_ready(Kind::GitHub, "github.com", folder.path().as_os_str())
        .unwrap()
        .unwrap();
    script(folder.path(), "gh", "printf 'second-token'");
    let next = for_fetch(
        Kind::GitHub,
        "github.com",
        folder.path().as_os_str(),
        initial,
    )
    .unwrap();
    assert_eq!(next.token.expose_secret(), "second-token");
    script(folder.path(), "gh", "exit 1");
    assert!(matches!(
        for_fetch(Kind::GitHub, "github.com", folder.path().as_os_str(), next),
        Err(Failure::Network(_))
    ));
}

#[test]
fn gitlab_refreshes_through_the_cli_before_reading_its_token() {
    let folder = tempfile::tempdir().unwrap();
    script(
        folder.path(),
        "glab",
        r#"
case "$*" in
  'api user --hostname git.corp.example')
    printf 'refreshed-token' > "${0%/*}/token"
    ;;
  'config get token --host git.corp.example')
    if test -f "${0%/*}/token"; then
      read -r token < "${0%/*}/token"
      printf '%s' "$token"
    else
      printf 'expired-token'
    fi
    ;;
  *) exit 1 ;;
esac
"#,
    );
    let path = folder.path().as_os_str();
    assert_eq!(
        cli(Kind::GitLab, "git.corp.example", path)
            .unwrap()
            .unwrap()
            .token
            .expose_secret(),
        "expired-token"
    );
    assert_eq!(
        cli_ready(Kind::GitLab, "git.corp.example", path)
            .unwrap()
            .unwrap()
            .token
            .expose_secret(),
        "refreshed-token"
    );
}

#[test]
fn a_stalled_cli_is_stopped_at_its_deadline() {
    let started = std::time::Instant::now();
    let mut sleeper = Command::new("sh");
    sleeper.args(["-c", "sleep 30"]);
    assert!(matches!(
        run_for(sleeper, true, std::time::Duration::from_millis(300)),
        Err(Failure::Network(_))
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    let mut echo = Command::new("sh");
    echo.args(["-c", "printf token-value"]);
    let (succeeded, stdout) = run_for(echo, true, std::time::Duration::from_secs(5)).unwrap();
    assert!(succeeded);
    assert_eq!(stdout, b"token-value");
}

#[test]
fn a_cli_that_exited_does_not_wait_forever_for_inherited_stdout() {
    let started = std::time::Instant::now();
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30 & printf token-value; exit 0"]);
    assert!(matches!(
        run_for(command, true, std::time::Duration::from_millis(300)),
        Err(Failure::Network(_))
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn cli_read_errors_are_retryable_and_can_recover() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().as_os_str();
    assert!(matches!(
        cli(Kind::GitHub, "github.com", path),
        Err(Failure::Network(_))
    ));
    script(dir.path(), "gh", "exit 1");
    assert!(matches!(
        cli(Kind::GitHub, "github.com", path),
        Err(Failure::Network(_))
    ));
    script(dir.path(), "gh", "exit 0");
    assert!(cli(Kind::GitHub, "github.com", path).unwrap().is_none());
    script(dir.path(), "gh", "printf recovered-token");
    assert_eq!(
        cli(Kind::GitHub, "github.com", path)
            .unwrap()
            .unwrap()
            .token
            .expose_secret(),
        "recovered-token"
    );
    script(
        dir.path(),
        "glab",
        "case \"$1\" in api) exit 1;; *) printf expired-token;; esac",
    );
    assert!(matches!(
        cli_ready(Kind::GitLab, "gitlab.com", path),
        Err(Failure::Network(_))
    ));
}

#[test]
fn a_transient_renewal_failure_preserves_the_saved_pair() {
    let dir = tempfile::tempdir().unwrap();
    let saved = Saved::new(Kind::GitLab, dir.path().to_owned(), true);
    let held = oauth("access-token", "refresh-token");
    saved.save("gitlab.com", &held).unwrap();
    let result = saved.renew("gitlab.com", &held, |_| Err(Failure::RateLimited));
    assert!(matches!(result, Renewal::Failed(Failure::RateLimited)));
    let Remembered::Token(kept) = saved.load("gitlab.com").unwrap().unwrap() else {
        panic!("lost sign-in")
    };
    assert_eq!(kept.refresh.unwrap().expose_secret(), "refresh-token");
    assert!(matches!(
        saved.renew("gitlab.com", &held, |_| Ok(oauth(
            "fresh-token",
            "fresh-refresh"
        ))),
        Renewal::Renewed(_)
    ));
}

fn oauth(token: &str, refresh: &str) -> Credential {
    Credential {
        token: SecretString::from(token.to_owned()),
        refresh: Some(SecretString::from(refresh.to_owned())),
        expires: Some(now() + 7200),
        source: Source::Saved,
    }
}

#[test]
fn apps_sharing_a_sign_in_renew_it_once() {
    let dir = tempfile::tempdir().unwrap();
    let first = Saved::new(Kind::GitLab, dir.path().to_owned(), true);
    let second = Saved::new(Kind::GitLab, dir.path().to_owned(), true);
    let held = oauth("access-one", "refresh-one");
    first.save("gitlab.com", &held).unwrap();
    // The first app renews it, and saves the new pair.
    let renewed = first.renew("gitlab.com", &held, |_| {
        Ok(oauth("access-two", "refresh-two"))
    });
    assert!(matches!(renewed, Renewal::Renewed(_)));
    // The second still holds the spent pair: it takes the saved one
    // instead of spending `refresh-one` again.
    let renewal = second.renew("gitlab.com", &held, |_| panic!("renewed a spent token"));
    let Renewal::Newer(newer) = renewal else {
        panic!("did not take the newer sign-in");
    };
    assert_eq!(newer.token.expose_secret(), "access-two");
    // Signed out meanwhile: nothing is renewed, nothing comes back.
    first.forget("gitlab.com").unwrap();
    let after = second.renew("gitlab.com", &newer, |_| panic!("renewed after a sign-out"));
    assert!(matches!(after, Renewal::Gone));
    assert!(matches!(first.load("gitlab.com"), Ok(None)));
}

#[test]
fn an_unsaved_renewal_is_still_ours_next_time() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("state");
    let saved = Saved::new(Kind::GitLab, store.clone(), true);
    let held = oauth("access-one", "refresh-one");
    saved.save("gitlab.com", &held).unwrap();
    // The store goes read-only: the renewal cannot be saved.
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o500)).unwrap();
    let Renewal::Renewed(fresh) = saved.renew("gitlab.com", &held, |_| {
        Ok(oauth("access-two", "refresh-two"))
    }) else {
        panic!("not renewed");
    };
    // Next time the store still holds the spent pair; that is not another
    // app's newer sign-in, and the fresh pair is renewed.
    let next = saved.renew("gitlab.com", &fresh, |_| {
        Ok(oauth("access-three", "refresh-three"))
    });
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        matches!(next, Renewal::Renewed(_)),
        "took the spent pair back"
    );
    // Once it can, it saves.
    let Renewal::Renewed(third) = next else {
        unreachable!()
    };
    let last = saved.renew("gitlab.com", &third, |_| {
        Ok(oauth("access-four", "refresh-four"))
    });
    assert!(matches!(last, Renewal::Renewed(_)));
    let Ok(Some(Remembered::Token(kept))) = saved.load("gitlab.com") else {
        panic!("nothing saved");
    };
    assert_eq!(kept.token.expose_secret(), "access-four");
}

#[test]
fn an_expired_newer_pair_is_renewed_under_the_same_lock() {
    let dir = tempfile::tempdir().unwrap();
    let saved = Saved::new(Kind::GitLab, dir.path().to_owned(), true);
    let stale = oauth("access-one", "refresh-one");
    let mut newer = oauth("access-two", "refresh-two");
    newer.expires = Some(1);
    saved.save("gitlab.com", &newer).unwrap();
    let result = saved.renew("gitlab.com", &stale, |current| {
        assert_eq!(
            current.refresh.as_ref().unwrap().expose_secret(),
            "refresh-two"
        );
        Ok(oauth("access-three", "refresh-three"))
    });
    let Renewal::Renewed(fresh) = result else {
        panic!("adopted an expired pair")
    };
    assert!(!fresh.expiring());
    let Ok(Some(Remembered::Token(kept))) = saved.load("gitlab.com") else {
        panic!("not saved")
    };
    assert_eq!(kept.token.expose_secret(), "access-three");
}

#[test]
fn unreadable_storage_preserves_multiple_unsaved_rotations() {
    let dir = tempfile::tempdir().unwrap();
    let saved = Saved::new(Kind::GitLab, dir.path().to_owned(), true);
    let held = oauth("access-one", "refresh-one");
    saved.save("gitlab.com", &held).unwrap();
    let record = dir.path().join("gitlab-sign-in.json");
    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o000)).unwrap();
    assert!(saved.load("gitlab.com").is_err());
    let Renewal::Renewed(second) = saved.renew("gitlab.com", &held, |_| {
        Ok(oauth("access-two", "refresh-two"))
    }) else {
        panic!("not renewed")
    };
    let Renewal::Renewed(third) = saved.renew("gitlab.com", &second, |current| {
        assert_eq!(current.token.expose_secret(), "access-two");
        Ok(oauth("access-three", "refresh-three"))
    }) else {
        panic!("not renewed")
    };
    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o600)).unwrap();
    let next = saved.renew("gitlab.com", &third, |current| {
        assert_eq!(current.token.expose_secret(), "access-three");
        Ok(oauth("access-four", "refresh-four"))
    });
    assert!(
        matches!(next, Renewal::Renewed(_)),
        "re-adopted a spent pair after storage recovered"
    );
    let Ok(Some(Remembered::Token(kept))) = saved.load("gitlab.com") else {
        panic!("not saved")
    };
    assert_eq!(kept.token.expose_secret(), "access-four");
}

#[test]
fn a_rotation_is_saved_if_storage_recovers_during_the_request() {
    let dir = tempfile::tempdir().unwrap();
    let saved = Saved::new(Kind::GitLab, dir.path().to_owned(), true);
    let held = oauth("access-one", "refresh-one");
    saved.save("gitlab.com", &held).unwrap();
    let record = dir.path().join("gitlab-sign-in.json");
    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = saved.renew("gitlab.com", &held, |_| {
        std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o600)).unwrap();
        Ok(oauth("access-two", "refresh-two"))
    });
    assert!(matches!(result, Renewal::Renewed(_)));
    let Ok(Some(Remembered::Token(kept))) = saved.load("gitlab.com") else {
        panic!("not saved")
    };
    assert_eq!(kept.token.expose_secret(), "access-two");
}

#[test]
fn retrying_a_save_updates_the_persisted_predecessor() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("state");
    let saved = Saved::new(Kind::GitLab, store.clone(), true);
    let held = oauth("access-one", "refresh-one");
    saved.save("gitlab.com", &held).unwrap();
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o500)).unwrap();
    let Renewal::Renewed(second) = saved.renew("gitlab.com", &held, |_| {
        Ok(oauth("access-two", "refresh-two"))
    }) else {
        panic!("not renewed")
    };
    // Failed account changes must also retain the unsaved pair.
    assert!(
        saved
            .save("gitlab.com", &oauth("other-access", "other-refresh"))
            .is_err()
    );
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
    let Renewal::Renewed(third) = saved.renew("gitlab.com", &second, |_| {
        // The retry persisted B; writing its rotation C now fails.
        let Ok(Some(Remembered::Token(kept))) = saved.load("gitlab.com") else {
            panic!("not saved")
        };
        assert_eq!(kept.token.expose_secret(), "access-two");
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o500)).unwrap();
        Ok(oauth("access-three", "refresh-three"))
    }) else {
        panic!("not renewed")
    };
    std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
    let next = saved.renew("gitlab.com", &third, |current| {
        assert_eq!(current.token.expose_secret(), "access-three");
        Ok(oauth("access-four", "refresh-four"))
    });
    assert!(
        matches!(next, Renewal::Renewed(_)),
        "re-adopted B after spending its refresh token"
    );
}
