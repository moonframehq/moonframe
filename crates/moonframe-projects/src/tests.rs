//! The provider on real repositories, driven as mods drive it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use {
    super::*,
    std::{fs, process::Command, thread, time::Duration},
    tempfile::{TempDir, tempdir},
};

fn sh(dir: &Path, args: &[&str]) {
    let done = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(done.status.success(), "git {args:?}: {done:?}");
}

/// `path` with links resolved, as git reports paths: macOS's temp folder is
/// one, and Windows gets forward slashes and no `\\\\?\\` in front.
fn real(path: &Path) -> PathBuf {
    let full = path.canonicalize().unwrap().to_string_lossy().into_owned();
    PathBuf::from(if cfg!(windows) {
        full.trim_start_matches(r"\\?\").replace('\\', "/")
    } else {
        full
    })
}

/// `rel` under `base` with a forward slash, as the provider joins paths:
/// `Path::join` puts a backslash there on Windows.
fn under(base: &Path, rel: &str) -> PathBuf {
    PathBuf::from(format!("{}/{rel}", base.to_string_lossy()))
}

/// A repository with one commit, in a project folder of its own.
struct Fixture {
    temp: TempDir,
    repo: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let root = real(temp.path());
        let repo = under(&root, "code/app");
        fs::create_dir_all(&repo).unwrap();
        sh(&repo, &["init", "-q", "-b", "main"]);
        sh(&repo, &["config", "user.name", "Test"]);
        sh(&repo, &["config", "user.email", "test@example.com"]);
        sh(&repo, &["config", "commit.gpgsign", "false"]);
        sh(&repo, &["config", "core.autocrlf", "false"]);
        fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
        fs::write(repo.join(".gitignore"), "build\n").unwrap();
        sh(&repo, &["add", "-A"]);
        sh(&repo, &["commit", "-q", "-m", "First"]);
        Self { temp, repo }
    }

    fn path(&self) -> String {
        self.repo.to_string_lossy().into_owned()
    }

    fn projects(&self) -> Projects {
        let root = self.repo.parent().unwrap();
        Projects::new(Config {
            home: real(self.temp.path()).to_string_lossy().into_owned(),
            roots: vec![root.to_string_lossy().into_owned()],
            writers: vec!["writer".to_owned()],
            kept: self.temp.path().join("cache/kept.json"),
        })
    }
}

fn published(projects: &mut Projects) -> Value {
    projects.last = None;
    projects.emit(true).unwrap()
}

/// Waits for the job under way, then publishes what it did.
fn settled(projects: &mut Projects) -> Value {
    for _ in 0..500 {
        if projects.collect() {
            return published(projects);
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("the job did not finish");
}

#[test]
fn it_finds_repositories_and_opens_the_newest() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    let out = published(&mut projects);
    assert_eq!(out["available"], true);
    let row = &out["projects"][0];
    assert_eq!(row["name"], "app");
    assert_eq!(row["branch"], "main");
    assert_eq!(row["subject"], "First");
    assert_eq!(row["home"], "~/code");
    assert_eq!(row["worktrees"][0]["primary"], true);
    assert_eq!(out["current"]["path"], fixture.path());
    assert_eq!(out["current"]["log"][0]["subject"], "First");
}

#[test]
fn a_repository_inside_another_is_not_listed_again() {
    let fixture = Fixture::new();
    let inner = fixture.repo.join("vendor");
    fs::create_dir_all(&inner).unwrap();
    sh(&inner, &["init", "-q"]);
    let roots = [fixture
        .repo
        .parent()
        .unwrap()
        .to_string_lossy()
        .into_owned()];
    assert_eq!(scan::find_projects(&roots).len(), 1);
}

#[test]
fn changes_list_edits_and_untracked_files() {
    let fixture = Fixture::new();
    fs::write(fixture.repo.join("a.txt"), "one\nchanged\nthree\n").unwrap();
    fs::write(fixture.repo.join("new.txt"), "x\ny").unwrap();
    let mut projects = fixture.projects();
    let out = published(&mut projects);
    let files = out["current"]["files"].as_array().unwrap();
    assert_eq!(files[0], "M\t2\t1\ta.txt");
    assert_eq!(files[1], "A\t2\t0\tnew.txt");
    assert_eq!(out["current"]["count"], 2);
    assert_eq!(out["current"]["added"], 4);
    assert_eq!(out["projects"][0]["files"], 2);
    // The last commit is its own scope.
    fs::write(fixture.repo.join("b.txt"), "b\n").unwrap();
    sh(&fixture.repo, &["add", "b.txt"]);
    sh(&fixture.repo, &["commit", "-q", "-m", "Second"]);
    projects.receive("any", &json!({ "scope": "last" }));
    let out = published(&mut projects);
    assert_eq!(out["current"]["scope"], "last");
    assert_eq!(out["current"]["files"], json!(["A\t1\t0\tb.txt"]));
}

#[test]
fn folders_list_with_what_git_ignores_marked() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.repo.join("build")).unwrap();
    fs::create_dir(fixture.repo.join("src")).unwrap();
    let mut projects = fixture.projects();
    let root = published(&mut projects)["dirs"][""].clone();
    assert_eq!(root, json!(["Dbuild", "dsrc", "f.gitignore", "fa.txt"]));
}

#[test]
fn completion_and_inspector_listings_are_independent() {
    let fixture = Fixture::new();
    for folder in ["src", "docs"] {
        fs::create_dir(fixture.repo.join(folder)).unwrap();
    }
    fs::write(fixture.repo.join("src/main.rs"), "fn main() {}").unwrap();
    let root = fixture.path();
    let mut projects = fixture.projects();
    projects.receive("any", &json!({ "select": root, "open": "docs" }));
    projects.receive("any", &json!({ "complete": "src", "cwd": root }));
    assert_eq!(projects.state.open, [json!("docs")]);
    assert_eq!(published(&mut projects)["dirs"]["src"], json!(["fmain.rs"]));
    projects.receive("any", &json!({ "open": "" }));
    let dirs = published(&mut projects)["dirs"].clone();
    assert!(dirs.get("src").is_some());
    assert!(dirs.get("docs").is_none());
    projects.receive(
        "any",
        &json!({ "complete": "other", "cwd": format!("{root}/different") }),
    );
    assert_eq!(projects.completion.as_deref(), Some("src"));
    let other = under(fixture.repo.parent().unwrap(), "other");
    fs::create_dir(&other).unwrap();
    sh(&other, &["init", "-q"]);
    projects.receive("any", &json!({ "select": other.to_string_lossy() }));
    assert!(projects.completion.is_none());
}

/// A repository outside the project folders.
fn outside(fixture: &Fixture) -> String {
    let elsewhere = under(&real(fixture.temp.path()), "elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    sh(&elsewhere, &["init", "-q"]);
    fs::write(elsewhere.join("secret.txt"), "private").unwrap();
    elsewhere.to_string_lossy().into_owned()
}

#[test]
fn a_mod_cannot_open_a_repository_outside_the_project_folders() {
    let fixture = Fixture::new();
    let elsewhere = outside(&fixture);
    let mut projects = fixture.projects();
    published(&mut projects);
    projects.receive("any", &json!({ "select": elsewhere, "read": "secret.txt" }));
    let out = published(&mut projects);
    assert_eq!(out["current"]["path"], fixture.path());
    assert_eq!(out["file"]["missing"], true);
}

#[test]
fn a_kept_repository_outside_the_project_folders_is_not_reopened() {
    let fixture = Fixture::new();
    let elsewhere = outside(&fixture);
    let mut projects = fixture.projects();
    projects.state.select = Some(elsewhere);
    assert_eq!(published(&mut projects)["current"]["path"], fixture.path());
}

#[test]
fn a_work_tree_goes_only_on_a_repository_found() {
    let fixture = Fixture::new();
    let elsewhere = outside(&fixture);
    let mut projects = fixture.projects();
    published(&mut projects);
    projects.receive("writer", &json!({ "worktree": "feat", "repo": elsewhere }));
    assert_eq!(published(&mut projects)["action"]["ok"], false);
}

#[test]
fn a_work_tree_just_added_can_be_opened() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    published(&mut projects);
    let tree = under(&real(fixture.temp.path()), "tree");
    sh(&fixture.repo, &[
        "worktree",
        "add",
        "-q",
        "-b",
        "side",
        tree.to_str().unwrap(),
    ]);
    projects.receive("any", &json!({ "select": tree.to_string_lossy() }));
    assert_eq!(
        published(&mut projects)["current"]["path"],
        tree.to_str().unwrap()
    );
}

#[test]
fn a_file_reads_only_from_inside_the_project() {
    let fixture = Fixture::new();
    let secret = fixture.temp.path().join("secret.txt");
    fs::write(&secret, "private").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&secret, fixture.repo.join("link")).unwrap();
    let mut projects = fixture.projects();
    published(&mut projects);
    projects.receive("any", &json!({ "read": "a.txt" }));
    assert_eq!(
        published(&mut projects)["file"]["lines"],
        json!(["one", "two"])
    );
    projects.receive("any", &json!({ "read": "../../secret.txt" }));
    assert!(published(&mut projects).get("file").is_none());
    #[cfg(unix)]
    {
        projects.receive("any", &json!({ "read": "link" }));
        let file = published(&mut projects)["file"].clone();
        assert_eq!(file["missing"], true);
        assert_eq!(file["lines"], json!([]));
    }
}

#[test]
fn only_writers_may_write() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    published(&mut projects);
    projects.receive("stranger", &json!({ "commit": "Sneaky", "id": 7 }));
    let action = published(&mut projects)["action"].clone();
    assert_eq!(action["ok"], false);
    assert_eq!(action["id"], 7);
    assert_eq!(action["action"], "commit");
    assert!(action["error"].as_str().unwrap().contains("`stranger`"));
}

#[test]
fn a_commit_stages_everything_and_shows_in_the_log() {
    let fixture = Fixture::new();
    fs::write(fixture.repo.join("new.txt"), "x\n").unwrap();
    let mut projects = fixture.projects();
    published(&mut projects);
    projects.receive("writer", &json!({ "commit": "Add a file" }));
    assert_eq!(published(&mut projects)["running"], "commit");
    let out = settled(&mut projects);
    assert_eq!(out["action"]["ok"], true);
    assert_eq!(out["action"]["id"], "commit");
    assert!(out.get("running").is_none());
    assert_eq!(out["current"]["log"][0]["subject"], "Add a file");
    assert_eq!(out["current"]["count"], 0);
}

#[test]
fn an_empty_commit_message_is_refused() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    published(&mut projects);
    projects.receive("writer", &json!({ "commit": "  " }));
    let action = published(&mut projects)["action"].clone();
    assert_eq!(action["error"], "Write a message to commit with");
}

#[test]
fn a_second_write_waits_its_turn() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    published(&mut projects);
    projects.running = Some("push".to_owned());
    projects.receive("writer", &json!({ "commit": "Too soon" }));
    let action = published(&mut projects)["action"].clone();
    assert_eq!(action["error"], "push is still running");
}

#[test]
fn a_work_tree_lands_in_the_moonframe_folder() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    published(&mut projects);
    let repo = fixture.path();
    projects.receive("writer", &json!({ "worktree": "feat/x", "repo": repo }));
    let out = settled(&mut projects);
    assert_eq!(out["action"]["ok"], true, "{}", out["action"]);
    let place = out["action"]["path"].as_str().unwrap();
    assert!(place.ends_with(".moonframe/worktrees/app/feat/x"));
    assert!(Path::new(place).is_dir());
    assert_eq!(out["projects"][0]["worktrees"][1]["branch"], "feat/x");
}

#[test]
fn a_bad_branch_name_is_refused() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    published(&mut projects);
    for name in ["../up", "-flag", "a..b", ""] {
        projects.receive(
            "writer",
            &json!({ "worktree": name, "repo": fixture.path() }),
        );
        assert_eq!(published(&mut projects)["action"]["ok"], false, "{name}");
    }
}

#[test]
fn what_the_app_asked_for_survives_a_restart() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    published(&mut projects);
    let open =
        json!({ "select": fixture.path(), "scope": "branch", "read": "a.txt", "open": "src" });
    projects.receive("any", &open);
    let mut again = fixture.projects();
    assert_eq!(again.state.select.as_deref(), Some(fixture.path().as_str()));
    assert_eq!(again.state.scope, "branch");
    assert_eq!(again.state.open, [json!("src")]);
    assert_eq!(published(&mut again)["file"]["path"], "a.txt");
}

#[test]
fn nothing_new_publishes_nothing() {
    let fixture = Fixture::new();
    let mut projects = fixture.projects();
    assert!(projects.emit(true).is_some());
    assert!(projects.emit(false).is_none());
}

#[test]
fn messages_read_loosely_as_lua_sends_them() {
    assert!(truthy(Some(&json!(1))));
    assert!(!truthy(Some(&json!(0))));
    assert!(!truthy(Some(&json!("0"))));
    assert!(!truthy(None));
    assert_eq!(text_of(Some(&json!(7.0))).as_deref(), Some("7"));
    assert_eq!(text_of(Some(&json!(true))), None);
    assert_eq!(tilde("/home/me/code", "/home/me"), "~/code");
    assert_eq!(tilde("/home/melody", "/home/me"), "/home/melody");
    assert_eq!(expand("~/code", "/home/me"), "/home/me/code");
    assert_eq!(last_name("/a/b"), Some("b"));
    assert_eq!(last_name("/a/"), None);
}
