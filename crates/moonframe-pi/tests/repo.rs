//! The repo provider against a real repository: each step follows on from
//! the last, as a person's would.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use {
    serde_json::{Value, json},
    std::{
        fs,
        path::{Path, PathBuf},
        thread,
        time::Duration,
    },
    support::{App, count, fixture_project, git},
};

const MIB: usize = 1024 * 1024;

fn host_takes(view: &Value) -> bool {
    view.to_string().len() <= MIB && count(view) <= 10_000
}

fn files_len(value: &Value) -> usize {
    value["files"].as_array().map_or(0, Vec::len)
}

struct Rig {
    app: App,
    project: PathBuf,
}

impl Rig {
    fn file(&self, path: &str) -> PathBuf {
        self.project.join(path)
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.file(path)).unwrap()
    }

    fn write(&self, path: &str, text: &str) {
        fs::write(self.file(path), text).unwrap();
    }
}

fn a_change_shows_with_its_diff_an_untracked_file_too(rig: &mut Rig) {
    let price = rig.read("src/price.js");
    rig.write("src/price.js", &format!("{price}// more\n"));
    rig.write("notes.txt", "one\ntwo\n");
    rig.app.send(&json!({ "refresh": true }));
    rig.app
        .when("changes", |v| files_len(v) == 2 && v["diff"].is_object());
    rig.app.send(&json!({ "pick": "src/price.js" }));
    let diff = rig.app.until("diff", |v| {
        (v["diff"]["path"] == "src/price.js").then(|| v["diff"].clone())
    });
    let lines = diff["hunks"][0]["lines"].as_array().unwrap();
    assert!(
        lines
            .iter()
            .any(|line| line["kind"] == "add" && line["text"] == "// more")
    );
    rig.app.send(&json!({ "pick": "notes.txt" }));
    let added = rig.app.until("untracked diff", |v| {
        (v["diff"]["path"] == "notes.txt").then(|| v["diff"].clone())
    });
    assert_eq!(added["hunks"][0]["lines"].as_array().unwrap().len(), 2);
}

fn staged(value: &Value, path: &str) -> bool {
    let files = value["files"].as_array().unwrap();
    files
        .iter()
        .any(|file| file["path"] == path && file["staged"] == true)
}

fn staging_and_unstaging_a_file(rig: &mut Rig) {
    rig.app.send(&json!({ "stage": "notes.txt" }));
    rig.app.when("staged", |v| staged(v, "notes.txt"));
    rig.app.send(&json!({ "unstage": "notes.txt" }));
    rig.app.when("unstaged", |v| !staged(v, "notes.txt"));
}

fn files_listing_opening_saving_refusing_what_changed_underneath(rig: &mut Rig) {
    let tree = rig.app.value()["tree"][""].clone();
    let rows = tree.as_array().unwrap();
    assert!(
        rows.iter()
            .any(|row| row["name"] == "src" && row["dir"] == true)
    );
    rig.app.send(&json!({ "open_dir": "src" }));
    rig.app.when("folder", |v| {
        let rows = v["tree"]["src"].as_array();
        rows.is_some_and(|rows| rows.iter().any(|row| row["name"] == "price.js"))
    });
    rig.app.send(&json!({ "read": "notes.txt" }));
    let file = rig.app.until("read", |v| {
        v["file"]["text"].is_string().then(|| v["file"].clone())
    });
    rig.app
        .send(&json!({ "write": "notes.txt", "text": "three\n", "version": file["version"] }));
    rig.app.when("saved", |v| v["file"]["saved"] == true);
    assert_eq!(rig.read("notes.txt"), "three\n");
    rig.write("notes.txt", "changed elsewhere\n");
    let version = rig.app.value()["file"]["version"].clone();
    rig.app
        .send(&json!({ "write": "notes.txt", "text": "mine\n", "version": version }));
    rig.app.when("conflict", |v| v["file"]["conflict"] == true);
    assert_eq!(rig.read("notes.txt"), "changed elsewhere\n");
    rig.app.send(&json!({ "read": "../outside" }));
    rig.app
        .when("outside refused", |v| v["file"]["missing"] == true);
}

fn finding_a_file_by_a_few_letters(rig: &mut Rig) {
    rig.app.send(&json!({ "find": "pric" }));
    let found = rig.app.until("found", |v| {
        (v["found"]["query"] == "pric").then(|| v["found"]["results"].clone())
    });
    let first_two = &found.as_array().unwrap()[..2];
    assert!(first_two.contains(&json!("src/price.js")), "{found}");
}

fn a_commit_takes_everything_when_nothing_is_staged(rig: &mut Rig) {
    rig.app.send(&json!({ "commit": "Tidy up" }));
    rig.app.when("committed", |v| {
        v["message"] == "Committed" && files_len(v) == 0
    });
}

fn a_file_that_json_makes_too_long_is_refused_not_published(rig: &mut Rig) {
    rig.write("quotes.txt", &"\"\n".repeat(400_000));
    rig.app.send(&json!({ "read": "quotes.txt" }));
    rig.app
        .when("opened", |v| v["file"]["path"] == "quotes.txt");
    let value = rig.app.value();
    assert_eq!(value["file"]["tooLarge"], true);
    assert!(host_takes(&value));
    rig.app.send(&json!({ "close": true }));
    rig.app.when("closed", |v| v["file"].is_null());
}

fn many_changes_and_a_long_diff_stay_under_the_host_s_value_limit(rig: &mut Rig) {
    for index in 0..400 {
        rig.write(&format!("many-{index}.txt"), "x\n");
    }
    rig.write("long.txt", &"line\n".repeat(3000));
    rig.app.send(&json!({ "refresh": true }));
    rig.app
        .when("listed", |v| v["more_files"].as_u64().unwrap_or(0) > 0);
    rig.app.send(&json!({ "pick": "long.txt" }));
    rig.app.when("diff", |v| v["diff"]["path"] == "long.txt");
    let value = rig.app.value();
    assert!(host_takes(&value), "{} values", count(&value));
    rig.app.send(&json!({ "stage_all": true }));
    rig.app.when("staged", |v| {
        v["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file["staged"] == true)
    });
    rig.app.send(&json!({ "commit": "Lots of files" }));
    rig.app.when("committed", |v| {
        v["message"] == "Committed" && files_len(v) == 0
    });
}

fn in_subfolder(value: &Value) -> bool {
    value["at"].as_str().is_some_and(|at| at.ends_with("src")) && files_len(value) > 0
}

fn a_project_below_the_repository_s_top_uses_paths_from_its_own_folder(rig: &mut Rig) {
    rig.write("src/price.js", "// changed\n");
    rig.write("outside.txt", "not in this project\n");
    rig.app.send(&json!({ "at": rig.file("src") }));
    rig.app.when("subfolder", in_subfolder);
    let value = rig.app.value();
    let paths: Vec<&Value> = value["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| &f["path"])
        .collect();
    assert_eq!(paths, [&json!("price.js")]);
    assert!(
        !value["diff"]["hunks"].as_array().unwrap().is_empty(),
        "its diff is found"
    );
    rig.app.send(&json!({ "stage": "price.js" }));
    rig.app.when("staged", |v| v["files"][0]["staged"] == true);
    rig.app.send(&json!({ "commit": "Only the subfolder" }));
    rig.app.when("committed", |v| {
        v["message"] == "Committed" && files_len(v) == 0
    });
    let committed = git(&rig.project, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(committed.trim(), "src/price.js");
    rig.app.send(&json!({ "at": rig.project }));
    rig.app.when("the rest is still uncommitted", |v| {
        v["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "outside.txt")
    });
}

fn a_commit_refuses_what_is_staged_beyond_the_project(rig: &mut Rig) {
    git(&rig.project, &["add", "outside.txt"]);
    rig.write("src/price.js", "// changed again\n");
    rig.app.send(&json!({ "at": rig.file("src") }));
    rig.app.when("subfolder", in_subfolder);
    let before = git(&rig.project, &["rev-parse", "HEAD"]);
    rig.app.send(&json!({ "commit": "Should not happen" }));
    rig.app.when("refused", |v| {
        v["error"]
            .as_str()
            .is_some_and(|error| error.contains("Staged outside this project: outside.txt"))
    });
    assert_eq!(git(&rig.project, &["rev-parse", "HEAD"]), before);
    rig.app.send(&json!({ "refresh": true }));
    thread::sleep(Duration::from_millis(300));
    let error = rig.app.value()["error"].clone();
    assert!(
        error
            .as_str()
            .unwrap_or_default()
            .contains("Staged outside"),
        "a refresh leaves it"
    );
    git(&rig.project, &["reset", "-q", "HEAD", "--", "outside.txt"]);
    rig.app.send(&json!({ "at": rig.project }));
    let at = json!(rig.project);
    rig.app.when("back", |v| v["at"] == at && files_len(v) > 0);
}

fn a_conflicting_save_shows_what_is_on_disk_and_can_replace_it(rig: &mut Rig) {
    rig.write("shared.txt", "first\n");
    rig.app.send(&json!({ "read": "shared.txt" }));
    let version = rig.app.until("opened", |v| {
        (v["file"]["path"] == "shared.txt").then(|| v["file"]["version"].clone())
    });
    rig.write("shared.txt", "someone else\n");
    rig.app
        .send(&json!({ "write": "shared.txt", "text": "mine\n", "version": version }));
    let conflict = rig.app.until("conflict", |v| {
        (v["file"]["conflict"] == true).then(|| v["file"].clone())
    });
    assert_eq!(conflict["text"], "someone else\n");
    assert_eq!(rig.read("shared.txt"), "someone else\n");
    let version = conflict["version"].clone();
    rig.app
        .send(&json!({ "write": "shared.txt", "text": "mine\n", "version": version }));
    rig.app.when("saved", |v| v["file"]["saved"] == true);
    assert_eq!(rig.read("shared.txt"), "mine\n");
    rig.app.send(&json!({ "close": true }));
    rig.app.when("closed", |v| v["file"].is_null());
}

fn a_search_can_be_of_another_folder_than_the_open_one(rig: &mut Rig, other: &Path) {
    fs::create_dir(other.join("docs")).unwrap();
    fs::write(other.join("docs/only-here.md"), "hello").unwrap();
    rig.app.send(&json!({ "find": "onlyhere", "root": other }));
    rig.app.when("found", |v| v["found"]["query"] == "onlyhere");
    assert_eq!(
        rig.app.value()["found"]["results"],
        json!(["docs/only-here.md"])
    );
}

#[test]
fn a_checkout_from_change_to_commit() {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let other = root.join("other");
    fs::create_dir(&other).unwrap();
    let project = fixture_project(&root);
    let mut app = App::start("repo", moonframe_pi::repo());
    app.send(&json!({ "at": project }));
    app.when("repository", |v| v["git"] == true && v["branch"] == "main");
    let mut rig = Rig { app, project };
    a_change_shows_with_its_diff_an_untracked_file_too(&mut rig);
    staging_and_unstaging_a_file(&mut rig);
    files_listing_opening_saving_refusing_what_changed_underneath(&mut rig);
    finding_a_file_by_a_few_letters(&mut rig);
    a_commit_takes_everything_when_nothing_is_staged(&mut rig);
    a_file_that_json_makes_too_long_is_refused_not_published(&mut rig);
    many_changes_and_a_long_diff_stay_under_the_host_s_value_limit(&mut rig);
    a_project_below_the_repository_s_top_uses_paths_from_its_own_folder(&mut rig);
    a_commit_refuses_what_is_staged_beyond_the_project(&mut rig);
    a_conflicting_save_shows_what_is_on_disk_and_can_replace_it(&mut rig);
    a_search_can_be_of_another_folder_than_the_open_one(&mut rig, &other);
}

#[test]
fn a_folder_that_is_no_repository_still_lists() {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    fs::write(root.join("note.md"), "hi").unwrap();
    let mut app = App::start("repo", moonframe_pi::repo());
    app.send(&json!({ "at": root }));
    app.when("listed", |v| {
        v["git"] == false && v["tree"][""][0]["name"] == "note.md"
    });
    app.send(&json!({ "find": "note" }));
    app.when("walked", |v| v["found"]["results"] == json!(["note.md"]));
}

#[test]
fn a_provider_toml_for_pilot_loads_as_native() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/pilot/providers");
    for name in ["pi", "repo"] {
        let manifest = moonframe_providers::Manifest::load(name, &root.join(name)).unwrap();
        assert_eq!(manifest.kind, moonframe_providers::Kind::Native);
        assert_eq!(manifest.publish, moonframe_providers::Publish::Latest);
    }
    let pi = moonframe_providers::Manifest::load("pi", &root.join("pi")).unwrap();
    assert_eq!(pi.start, moonframe_providers::Start::Always);
}
