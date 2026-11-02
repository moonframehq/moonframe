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
