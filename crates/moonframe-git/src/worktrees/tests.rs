use {
    super::*,
    std::{collections::BTreeMap, process::Command},
};

/// A repository in a folder of its own, so `<name>.worktrees` lands in the
/// test's folder too.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

fn run(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let created = dir.path().join("project");
    std::fs::create_dir(&created).unwrap();
    run(&created, &["init", "-q", "-b", "main"]);
    // As git names it, as the provider has it: not as the temporary folder
    // was named, through a link on macOS, with other separators on Windows.
    let root = PathBuf::from(run(&created, &["rev-parse", "--show-toplevel"]));
    for (key, value) in [
        ("user.name", "Moonframe Test"),
        ("user.email", "test@moonframe.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "/dev/null"),
    ] {
        run(&root, &["config", key, value]);
    }
    std::fs::write(root.join("a.txt"), "one\n").unwrap();
    run(&root, &["add", "-A"]);
    run(&root, &["commit", "-q", "-m", "first"]);
    Fixture { _dir: dir, root }
}

fn add(fixture: &Fixture, branch: &str, base: Option<&str>) -> Result<String, String> {
    Change::Add {
        branch: branch.to_owned(),
        base: base.map(str::to_owned),
    }
    .apply(&fixture.root)
}

fn remove(fixture: &Fixture, path: &str) -> Result<String, String> {
    Change::Remove {
        path: path.to_owned(),
    }
    .apply(&fixture.root)
}

fn branches(root: &Path) -> String {
    run(root, &["branch", "--format=%(refname:short)"])
}

#[test]
fn a_new_branch_gets_a_folder_beside_the_repository() {
    let fixture = fixture();
    let path = add(&fixture, "feat/login", None).unwrap();
    let expected = fixture
        .root
        .with_file_name("project.worktrees")
        .join("feat-login");
    assert_eq!(Path::new(&path), expected);
    assert!(expected.join("a.txt").is_file());
    assert_eq!(run(&expected, &["branch", "--show-current"]), "feat/login");
}

#[test]
fn a_branch_that_exists_is_checked_out_and_a_base_is_followed() {
    let fixture = fixture();
    let first = run(&fixture.root, &["rev-parse", "HEAD"]);
    run(&fixture.root, &["branch", "kept"]);
    std::fs::write(fixture.root.join("a.txt"), "two\n").unwrap();
    run(&fixture.root, &["commit", "-qam", "second"]);

    let path = add(&fixture, "kept", None).unwrap();
    assert_eq!(run(Path::new(&path), &["rev-parse", "HEAD"]), first);

    let path = add(&fixture, "from-first", Some(&first)).unwrap();
    assert_eq!(run(Path::new(&path), &["rev-parse", "HEAD"]), first);
}

#[test]
fn nothing_a_mod_sends_is_read_as_an_option() {
    let fixture = fixture();
    let before = branches(&fixture.root);
    for branch in ["--force", "-b", "a b", "a..b", "@{-1}", "", "bad~1"] {
        let refused = add(&fixture, branch, None).unwrap_err();
        assert!(refused.contains("not a branch name"), "{branch}: {refused}");
    }
    for base in ["--detach", "no-such-branch", "-"] {
        let refused = add(&fixture, "fine", Some(base)).unwrap_err();
        assert!(refused.contains("no commit"), "{base}: {refused}");
    }
    assert_eq!(branches(&fixture.root), before);
    assert!(!fixture.root.with_file_name("project.worktrees").exists());
}

#[test]
fn a_folder_in_the_way_is_left_alone() {
    let fixture = fixture();
    let folder = fixture
        .root
        .with_file_name("project.worktrees")
        .join("taken");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("mine.txt"), "keep\n").unwrap();
    assert!(add(&fixture, "taken", None).unwrap_err().contains("exists"));
    assert!(folder.join("mine.txt").is_file());
}

#[test]
fn removing_a_worktree_keeps_its_branch() {
    let fixture = fixture();
    let path = add(&fixture, "done", None).unwrap();
    let said = remove(&fixture, &path).unwrap();
    assert!(said.contains("branch done is kept"), "{said}");
    assert!(!Path::new(&path).exists());
    assert!(branches(&fixture.root).contains("done"));
}

#[test]
fn work_in_a_worktree_is_never_thrown_away() {
    let fixture = fixture();
    let path = add(&fixture, "busy", None).unwrap();
    let file = Path::new(&path).join("new.txt");
    std::fs::write(&file, "unsaved\n").unwrap();
    let refused = remove(&fixture, &path).unwrap_err();
    assert!(refused.contains("uncommitted work"), "{refused}");
    assert!(!refused.contains("--force"), "{refused}");
    assert!(file.is_file());
}

#[test]
fn only_a_linked_worktree_elsewhere_can_be_removed() {
    let fixture = fixture();
    let main = fixture.root.to_string_lossy().into_owned();
    assert!(
        remove(&fixture, &main)
            .unwrap_err()
            .contains("main checkout")
    );
    assert!(fixture.root.join("a.txt").is_file());

    let stranger = fixture.root.with_file_name("stranger");
    std::fs::create_dir(&stranger).unwrap();
    let path = stranger.to_string_lossy().into_owned();
    assert!(
        remove(&fixture, &path)
            .unwrap_err()
            .contains("not a worktree")
    );
    assert!(stranger.is_dir());

    // The checkout the provider watches, when that is a linked one.
    let linked = add(&fixture, "here", None).unwrap();
    let refused = Change::Remove {
        path: linked.clone(),
    }
    .apply(Path::new(&linked))
    .unwrap_err();
    assert!(refused.contains("in use"), "{refused}");
    assert!(Path::new(&linked).is_dir());
}

#[test]
fn an_add_is_read_from_a_message() {
    let map = |pairs: &[(&str, &str)]| {
        Data::Map(
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), Data::from(*value)))
                .collect::<BTreeMap<_, _>>(),
        )
    };
    assert_eq!(
        Change::add(&map(&[("branch", "a"), ("base", "main")])),
        Ok(Change::Add {
            branch: "a".into(),
            base: Some("main".into())
        })
    );
    assert!(Change::add(&map(&[("base", "main")])).is_err());
    assert!(Change::add(&Data::from("a")).is_err());
}
