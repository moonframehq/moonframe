use {
    super::*,
    moonframe_core::{Roots, Workspace},
    std::process::Command,
};

const SHELL: &str = r#"return mod { view = function() return ui.region "right" end }"#;

fn yes(_: &Contents) -> Result<(), String> {
    Ok(())
}

fn no(_: &Contents) -> Result<(), String> {
    Err("declined".to_owned())
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Runs git in `dir` as a test author.
fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

/// A repository named `name` in `parent`, with `files` committed.
fn repo(parent: &Path, name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = parent.join(name);
    fs::create_dir_all(&dir).unwrap();
    run_git(&dir, &["init", "--quiet"]);
    commit(&dir, files, "First");
    dir
}

fn commit(dir: &Path, files: &[(&str, &str)], message: &str) {
    for (relative, text) in files {
        write(dir, relative, text);
    }
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "--quiet", "-m", message]);
}

fn counter(text: &str) -> String {
    format!(r#"return mod {{ region = "right", view = function() return ui.text "{text}" end }}"#)
}

/// The app with the user's enabled packages, as the window loads it.
fn open(builtin: &Path, user: &Path) -> Workspace {
    let roots = Roots::with_packages(
        builtin.to_owned(),
        enabled_folders(user),
        Some(user.to_owned()),
    );
    Workspace::open(roots, false)
}

fn shown(workspace: &Workspace, name: &str) -> String {
    match workspace.slot(name).and_then(|slot| slot.view()) {
        Some(moonframe_core::Node::Text(text)) => text.text.clone(),
        other => panic!("{name}: {other:?} {:?}", workspace.problems()),
    }
}

#[test]
fn sources_name_a_repository_and_a_ref() {
    let parse = |text: &str| Source::parse(text).unwrap();
    assert_eq!(parse("github.com/penso/my-super-mod"), Source {
        id: "github.com/penso/my-super-mod".into(),
        url: "https://github.com/penso/my-super-mod".into(),
        reference: None,
    });
    assert_eq!(
        parse("penso/my-super-mod@v1.2").url,
        "https://github.com/penso/my-super-mod"
    );
    assert_eq!(
        parse("penso/my-super-mod@v1.2").reference.as_deref(),
        Some("v1.2")
    );
    let ssh = parse("git@github.com:penso/my-super-mod.git");
    assert_eq!(
        (ssh.id.as_str(), ssh.url.as_str(), ssh.reference),
        (
            "github.com/penso/my-super-mod",
            "git@github.com:penso/my-super-mod.git",
            None
        )
    );
    let https = parse("https://user@gitlab.com:443/group/sub/mod.git@main");
    assert_eq!(https.id, "gitlab.com/group/sub/mod");
    assert_eq!(https.reference.as_deref(), Some("main"));
    assert!(parse("/tmp/some/my-mod").id == "local/my-mod");

    for bad in [
        "github.com/../etc",
        "nothing",
        "ftp://host/a/b",
        "github.com/a/b@",
        "github.com/a b/c",
    ] {
        assert!(Source::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn an_installed_mod_is_laid_between_the_app_and_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let (builtin, user) = (dir.path().join("app"), dir.path().join("user"));
    write(&builtin, "main.luau", SHELL);
    write(&builtin, "mods/clock/main.luau", &counter("built-in clock"));
    let remote = repo(dir.path(), "My.Clock", &[(
        "main.luau",
        &counter("package"),
    )]);
    let package = repo(dir.path(), "extras", &[
        ("mods/clock/main.luau", &counter("package clock")),
        (
            "components/pill.luau",
            "return component { build = function() return ui.text 'pill' end }",
        ),
    ]);

    let (id, contents) = install(&user, remote.to_str().unwrap(), &yes).unwrap();
    assert_eq!(id, "local/My.Clock");
    assert_eq!(contents.mods, ["my-clock"]);
    let (_, contents) = install(&user, package.to_str().unwrap(), &yes).unwrap();
    assert_eq!(contents.to_string(), "mods: clock; components: pill");
    assert!(
        install(&user, package.to_str().unwrap(), &yes)
            .unwrap_err()
            .contains("already installed")
    );

    let workspace = open(&builtin, &user);
    assert_eq!(shown(&workspace, "my-clock"), "package");
    assert_eq!(shown(&workspace, "clock"), "package clock");
    assert_eq!(workspace.components().len(), 1);

    // The user's own mod goes over the package's.
    write(&user, "mods/clock/main.luau", &counter("mine"));
    assert_eq!(shown(&open(&builtin, &user), "clock"), "mine");
    fs::remove_dir_all(user.join("mods")).unwrap();

    set_enabled(&user, "EXTRAS", false).unwrap();
    assert_eq!(shown(&open(&builtin, &user), "clock"), "built-in clock");
    set_enabled(&user, "local/extras", true).unwrap();

    assert_eq!(remove(&user, "My.Clock").unwrap(), "local/My.Clock");
    assert_eq!(remove(&user, "extras").unwrap(), "local/extras");
    assert!(
        !user.join("packages/local").exists(),
        "empty folders stay behind"
    );
    assert!(open(&builtin, &user).slot("my-clock").is_none());
    assert!(remove(&user, "extras").is_err());
}

#[test]
fn a_ref_pins_a_package_and_update_follows_it() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let remote = repo(dir.path(), "clock", &[("main.luau", &counter("one"))]);
    run_git(&remote, &["tag", "v1"]);
    commit(&remote, &[("main.luau", &counter("two"))], "Two");

    install(&user, &format!("{}@v1", remote.display()), &yes).unwrap();
    let folder = user.join("packages/local/clock");
    let main = || fs::read_to_string(folder.join("main.luau")).unwrap();
    assert!(main().contains("one"));
    let updated = update(&user, None, &yes).unwrap();
    assert!(
        matches!(updated[0].1, Ok(Updated::Current(_))),
        "{updated:?}"
    );

    // Unpinned, it follows the default branch.
    remove(&user, "clock").unwrap();
    install(&user, remote.to_str().unwrap(), &yes).unwrap();
    assert!(main().contains("two"));
    commit(&remote, &[("main.luau", &counter("three"))], "Three");
    let updated = update(&user, Some("clock"), &yes).unwrap();
    assert!(
        matches!(updated[0].1, Ok(Updated::Moved { .. })),
        "{updated:?}"
    );
    assert!(main().contains("three"));
}

#[test]
fn providers_that_run_as_the_user_need_approval() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let remote = repo(dir.path(), "weather", &[
        ("mods/weather/main.luau", &counter("sunny")),
        (
            "providers/forecast/provider.toml",
            "kind = \"process\"\ncommand = [\"sh\", \"fetch.sh\"]\n",
        ),
    ]);
    let problem = install(&user, remote.to_str().unwrap(), &no).unwrap_err();
    assert_eq!(problem, "declined");
    assert!(!user.join("packages/local/weather").exists());
    assert_eq!(
        fs::read_dir(user.join("packages")).unwrap().count(),
        0,
        "a clone was left behind"
    );

    let (_, contents) = install(&user, remote.to_str().unwrap(), &yes).unwrap();
    let trusted: Vec<String> = contents
        .trusted()
        .map(|(name, how)| format!("{name}: {how}"))
        .collect();
    assert_eq!(trusted, [
        r#"forecast: runs ["sh" "fetch.sh"] with your access"#
    ]);

    // An update that is declined stays where it was.
    commit(
        &remote,
        &[("mods/weather/main.luau", &counter("rain"))],
        "Rain",
    );
    let live = user.join("packages/local/weather/mods/weather/main.luau");
    let unchanged = |_: &Contents| {
        let main = fs::read_to_string(&live).unwrap();
        assert!(
            main.contains("sunny"),
            "unapproved code reached the live folder: {main}"
        );
        Err("declined".to_owned())
    };
    let updated = update(&user, None, &unchanged).unwrap();
    assert_eq!(updated[0].1, Err("declined".to_owned()));
    let leftovers: Vec<_> = fs::read_dir(user.join("packages/local"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        leftovers,
        ["weather"],
        "the inspected checkout was left behind"
    );
    let main =
        fs::read_to_string(user.join("packages/local/weather/mods/weather/main.luau")).unwrap();
    assert!(main.contains("sunny"), "{main}");
}

#[test]
fn a_repository_without_a_mod_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let remote = repo(dir.path(), "notes", &[("README.md", "nothing here")]);
    let problem = install(&user, remote.to_str().unwrap(), &yes).unwrap_err();
    assert!(problem.contains("not a Moonframe package"), "{problem}");
    assert!(List::read(&user).unwrap().package.is_empty());
}

#[test]
fn a_new_package_links_in_place_and_loads() {
    let dir = tempfile::tempdir().unwrap();
    let (builtin, user) = (dir.path().join("app"), dir.path().join("user"));
    write(&builtin, "main.luau", SHELL);
    let folder = dir.path().join("Weather Mod");
    assert_eq!(create(&folder).unwrap(), "weather-mod");
    assert!(folder.join(".git").is_dir());
    // Its README links to the app's own repository, from Cargo.toml.
    let readme = fs::read_to_string(folder.join("README.md")).unwrap();
    assert!(
        readme.contains(&format!("[Moonframe]({})", env!("CARGO_PKG_REPOSITORY"))),
        "{readme}"
    );
    assert!(create(&folder).is_err(), "it overwrote a package");

    let (id, _) = link(&user, folder.to_str().unwrap(), &yes).unwrap();
    assert_eq!(id, "local/Weather Mod");
    let workspace = open(&builtin, &user);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.region("right"), ["weather-mod"]);
    let updated = update(&user, None, &yes).unwrap();
    assert_eq!(updated[0].1, Ok(Updated::Linked));

    // Removing a link keeps the folder.
    remove(&user, "Weather Mod").unwrap();
    assert!(folder.join("main.luau").is_file());
}

#[test]
fn a_broken_list_loads_no_packages() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), FILE, "package = 3");
    assert!(enabled_folders(dir.path()).is_empty());
    assert!(installed(dir.path()).is_err());
}

#[test]
fn a_package_ships_source_only() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let provider = "kind = \"wasm\"\n";
    let remote = repo(dir.path(), "meter", &[
        ("mods/meter/main.luau", &counter("load")),
        ("providers/load/provider.toml", provider),
        ("providers/load/provider.wasm", "\0asm"),
    ]);
    let problem = install(&user, remote.to_str().unwrap(), &yes).unwrap_err();
    assert!(
        problem.contains("providers/load/provider.wasm"),
        "{problem}"
    );

    // Nor an executable a process could run, under any name.
    fs::remove_file(remote.join("providers/load/provider.wasm")).unwrap();
    commit(
        &remote,
        &[
            (
                "providers/load/provider.toml",
                "kind = \"process\"\ncommand = [\"./tool\"]\n",
            ),
            ("providers/load/tool", "\u{7f}ELF\0\0\0"),
        ],
        "Tool",
    );
    let problem = install(&user, remote.to_str().unwrap(), &yes).unwrap_err();
    assert!(problem.contains("providers/load/tool"), "{problem}");
    assert!(!user.join("packages/local/meter").exists());

    // Nor one named like an image.
    fs::rename(remote.join("providers/load/tool"), remote.join("icon.png")).unwrap();
    commit(&remote, &[], "Disguised");
    let problem = install(&user, remote.to_str().unwrap(), &yes).unwrap_err();
    assert!(problem.contains("icon.png"), "{problem}");

    // Source, and images, are fine; a wasm crate is built, as the user.
    fs::write(remote.join("icon.png"), b"\x89PNG\r\n\x1a\n\0\0").unwrap();
    commit(
        &remote,
        &[
            ("providers/load/provider.toml", provider),
            ("providers/load/Cargo.toml", "[package]\nname = \"load\"\n"),
            ("providers/load/src/lib.rs", "// a provider\n"),
        ],
        "Source",
    );
    let (_, contents) = install(&user, remote.to_str().unwrap(), &yes).unwrap();
    let trusted: Vec<String> = contents
        .trusted()
        .map(|(name, how)| format!("{name}: {how}"))
        .collect();
    assert_eq!(trusted, ["load: built with Cargo, with your access"]);

    // An update that adds a binary is refused, and the package stays put.
    commit(
        &remote,
        &[("providers/load/provider.wasm", "\0asm")],
        "Prebuilt",
    );
    let updated = update(&user, None, &yes).unwrap();
    assert!(
        matches!(&updated[0].1, Err(problem) if problem.contains("provider.wasm")),
        "{updated:?}"
    );
    assert!(
        !user
            .join("packages/local/meter/providers/load/provider.wasm")
            .exists()
    );
}

#[test]
fn a_linked_folder_skips_what_git_ignores() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let folder = repo(dir.path(), "dev", &[
        ("main.luau", &counter("dev")),
        (".gitignore", "target/\n"),
    ]);
    write(&folder, "target/debug/dev", "\u{7f}ELF\0");
    link(&user, folder.to_str().unwrap(), &yes).unwrap();
    remove(&user, "dev").unwrap();
    write(&folder, "notes.bin", "\0");
    let problem = link(&user, folder.to_str().unwrap(), &yes).unwrap_err();
    assert!(problem.contains("notes.bin"), "{problem}");
}

#[test]
fn package_text_cannot_drive_the_terminal() {
    let provider = Provider::Process(vec!["sh".into(), "-c".into(), "x\x1b[2K\rsafe".into()]);
    let shown = format!("{provider}");
    assert!(!shown.contains('\x1b'), "{shown}");
    assert_eq!(printable("a\x1b[2Kb"), "a\\u{1b}[2Kb");
}

/// A package's native provider runs no code of its own, but it replaces the
/// app's built-in one with its settings (git's `writers`), so it is asked
/// about, and the prompt says what it changes.
#[test]
fn a_native_provider_in_a_package_needs_approval() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let remote = repo(dir.path(), "writer", &[
        ("mods/writer/main.luau", &counter("w")),
        (
            "providers/git/provider.toml",
            "kind = \"native\"\n[settings]\nwriters = [\"writer\"]\n",
        ),
    ]);
    assert_eq!(
        install(&user, remote.to_str().unwrap(), &no).unwrap_err(),
        "declined"
    );
    let (_, contents) = install(&user, remote.to_str().unwrap(), &yes).unwrap();
    let trusted: Vec<String> = contents
        .trusted()
        .map(|(name, how)| format!("{name}: {how}"))
        .collect();
    assert_eq!(trusted.len(), 1);
    assert!(
        trusted[0].starts_with("git: replaces the app's built-in"),
        "{trusted:?}"
    );
    assert!(trusted[0].contains("writer"), "{trusted:?}");
}

/// A package's mod named like one git lets commit and push (`status`)
/// would take that right over, so the prompt names it. The writers of
/// every layer count: built-in, the user's, and installed packages'.
#[test]
fn a_mod_that_shadows_a_git_writer_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("user");
    let remote = repo(dir.path(), "bar", &[
        ("mods/status/main.luau", &counter("s")),
        ("mods/other/main.luau", &counter("o")),
    ]);
    let (_, contents) = install(&user, remote.to_str().unwrap(), &yes).unwrap();
    let builtin = git_writers(&user);
    assert!(builtin.contains(&"status".to_owned()), "{builtin:?}");
    assert_eq!(contents.writers(&builtin).collect::<Vec<_>>(), ["status"]);

    let git = user.join("providers/git");
    fs::create_dir_all(&git).unwrap();
    fs::write(
        git.join("provider.toml"),
        "kind = \"native\"\n[settings]\nwriters = [\"other\"]\n",
    )
    .unwrap();
    let writers = git_writers(&user);
    assert_eq!(writers, ["other", "status"]);
    assert_eq!(contents.writers(&writers).collect::<Vec<_>>(), [
        "other", "status"
    ]);

    // A package installed earlier that widened `writers` counts too.
    let widening = repo(dir.path(), "widen", &[(
        "providers/git/provider.toml",
        "kind = \"native\"\n[settings]\nwriters = [\"later\"]\n",
    )]);
    install(&user, widening.to_str().unwrap(), &yes).unwrap();
    assert!(git_writers(&user).contains(&"later".to_owned()));
}
