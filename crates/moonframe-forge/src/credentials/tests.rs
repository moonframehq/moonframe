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
    let initial = cli_ready(Kind::GitHub, "github.com", folder.path().as_os_str()).unwrap();
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
    assert!(for_fetch(Kind::GitHub, "github.com", folder.path().as_os_str(), next).is_none());
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
            .token
            .expose_secret(),
        "expired-token"
    );
    assert_eq!(
        cli_ready(Kind::GitLab, "git.corp.example", path)
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
    assert!(run_for(sleeper, true, std::time::Duration::from_millis(300)).is_none());
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    let mut echo = Command::new("sh");
    echo.args(["-c", "printf token-value"]);
    let (succeeded, stdout) = run_for(echo, true, std::time::Duration::from_secs(5)).unwrap();
    assert!(succeeded);
    assert_eq!(stdout, b"token-value");
}

fn oauth(token: &str, refresh: &str) -> Credential {
    Credential {
        token: SecretString::from(token.to_owned()),
        refresh: Some(SecretString::from(refresh.to_owned())),
        expires: Some(1),
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
        Some(oauth("access-two", "refresh-two"))
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
        Some(oauth("access-two", "refresh-two"))
    }) else {
        panic!("not renewed");
    };
    // Next time the store still holds the spent pair; that is not another
    // app's newer sign-in, and the fresh pair is renewed.
    let next = saved.renew("gitlab.com", &fresh, |_| {
        Some(oauth("access-three", "refresh-three"))
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
        Some(oauth("access-four", "refresh-four"))
    });
    assert!(matches!(last, Renewal::Renewed(_)));
    let Ok(Some(Remembered::Token(kept))) = saved.load("gitlab.com") else {
        panic!("nothing saved");
    };
    assert_eq!(kept.token.expose_secret(), "access-four");
}
