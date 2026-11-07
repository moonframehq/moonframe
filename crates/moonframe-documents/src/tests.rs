//! The library on its own files, each run as the app would make one.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use {
    super::*,
    std::path::Path,
    tempfile::{TempDir, tempdir},
};

struct Fixture {
    _temp: TempDir,
    folder: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let folder = temp.path().join("documents");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("Draft.md"), "Original draft").unwrap();
        Self {
            _temp: temp,
            folder,
        }
    }

    fn run(&self) -> Library {
        Library::open(self.folder.clone(), &[]).unwrap()
    }

    fn file(&self, name: &str) -> PathBuf {
        self.folder.join(format!("{name}.md"))
    }

    fn text(&self, name: &str) -> String {
        fs::read_to_string(self.file(name)).unwrap()
    }

    fn write(&self, name: &str, body: &str) {
        fs::write(self.file(name), body).unwrap();
    }

    fn trash(&self) -> PathBuf {
        let trash = self.folder.join(".trash");
        fs::create_dir_all(&trash).unwrap();
        trash
    }

    fn matching(&self, folder: &str, extension: &str) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(self.folder.join(folder)) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == extension))
            .collect()
    }

    fn journals(&self) -> Vec<PathBuf> {
        self.matching(".journals", "json")
    }
}

fn out(library: &mut Library) -> Value {
    library.published().0
}

fn send(library: &mut Library, message: Value) -> Value {
    library.receive(&message).unwrap();
    out(library)
}

fn opened(published: &Value) -> String {
    published["current"]["opened"].as_str().unwrap().to_owned()
}

fn ids(published: &Value) -> Vec<String> {
    let mut ids: Vec<String> = published["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|doc| doc["id"].as_str().unwrap().to_owned())
        .collect();
    ids.sort();
    ids
}

#[test]
fn a_killed_run_recovers_its_latest_edit_before_the_debounce() {
    let fixture = Fixture::new();
    let mut run = fixture.run();
    out(&mut run);
    send(
        &mut run,
        json!({ "save": { "id": "Draft", "text": "First edit" } }),
    );
    send(
        &mut run,
        json!({ "save": { "id": "Draft", "text": "Latest edit — café" } }),
    );
    // Publication acknowledges the edit, but the markdown is still old.
    assert_eq!(fixture.text("Draft"), "Original draft");
    run.kill();
    let restored = out(&mut fixture.run());
    assert_eq!(restored["current"]["text"], "Latest edit — café");
    assert_eq!(fixture.text("Draft"), "Latest edit — café");
    assert!(fixture.journals().is_empty());
}

#[test]
fn opening_tokens_change_on_reopen_and_restart() {
    let fixture = Fixture::new();
    let mut run = fixture.run();
    let first = opened(&out(&mut run));
    let second = opened(&send(&mut run, json!({ "open": "Draft" })));
    assert_ne!(first, second);
    run.kill();
    fixture.write("Draft", "Edited outside Folio");
    let current = out(&mut fixture.run());
    assert!(![first, second].contains(&opened(&current)));
    assert_eq!(current["current"]["text"], "Edited outside Folio");
}

#[test]
fn switching_documents_flushes_the_previous_edit() {
    let fixture = Fixture::new();
    let mut run = fixture.run();
    out(&mut run);
    send(
        &mut run,
        json!({ "save": { "id": "Draft", "text": "Saved before switching" } }),
    );
    let new = send(&mut run, json!({ "new": true }));
    let new = new["current"]["id"].as_str().unwrap().to_owned();
    assert_ne!(new, "Draft");
    assert_eq!(fixture.text("Draft"), "Saved before switching");
    send(
        &mut run,
        json!({ "save": { "id": new, "text": "New document edit" } }),
    );
    run.kill();
    drop(fixture.run());
    assert_eq!(fixture.text(&new), "New document edit");
    assert_eq!(fixture.text("Draft"), "Saved before switching");
}

#[test]
fn purging_a_pending_document_does_not_recover_it() {
    let fixture = Fixture::new();
    let mut run = fixture.run();
    out(&mut run);
    send(&mut run, json!({ "trash": "Draft" }));
    send(
        &mut run,
        json!({ "save": { "id": "Draft", "text": "Edited in trash" } }),
    );
    assert!(send(&mut run, json!({ "purge": "Draft" }))["current"].is_null());
    run.kill();
    assert!(ids(&out(&mut fixture.run())).is_empty());
    assert!(fixture.journals().is_empty());
}

#[test]
fn trashing_never_replaces_a_trashed_document_of_the_same_name() {
    let fixture = Fixture::new();
    let trash = fixture.trash();
    fs::write(trash.join("Draft.md"), "Trashed earlier").unwrap();
    let mut run = fixture.run();
    out(&mut run);
    let published = send(&mut run, json!({ "trash": "Draft" }));
    let trashed = |name: &str| fs::read_to_string(trash.join(name)).unwrap();
    assert_eq!(trashed("Draft.md"), "Trashed earlier");
    assert_eq!(trashed("Draft (2).md"), "Original draft");
    assert_eq!(ids(&published), ["Draft", "Draft (2)"]);
    // Putting it back is just as careful.
    fixture.write("Draft", "Written meanwhile");
    send(&mut run, json!({ "restore": "Draft" }));
    assert_eq!(fixture.text("Draft"), "Written meanwhile");
    assert_eq!(fixture.text("Draft (2)"), "Trashed earlier");
    // Deleting for good takes the trashed one, never the live one.
    fs::write(trash.join("Draft.md"), "Trashed again").unwrap();
    send(&mut run, json!({ "purge": "Draft" }));
    assert!(!trash.join("Draft.md").exists());
    assert_eq!(fixture.text("Draft"), "Written meanwhile");
}

#[test]
fn trashing_the_open_one_from_the_trash_leaves_a_live_namesake() {
    let fixture = Fixture::new();
    let mut run = fixture.run();
    out(&mut run);
    // The only document goes to the trash, and opens from there.
    let published = send(&mut run, json!({ "trash": "Draft" }));
    assert_eq!(published["current"]["id"], "Draft");
    fixture.write("Draft", "Live");
    send(&mut run, json!({ "trash_current": true }));
    assert_eq!(fixture.text("Draft"), "Live");
    let trashed = fs::read_to_string(fixture.trash().join("Draft.md")).unwrap();
    assert_eq!(trashed, "Original draft");
}

#[test]
fn a_document_too_large_to_send_opens_without_its_text() {
    let fixture = Fixture::new();
    fixture.write("Draft", &"é\n".repeat(200_000));
    let published = out(&mut fixture.run());
    assert_eq!(published["current"]["too_large"], true);
    assert!(published["current"].get("text").is_none());
}

#[test]
fn a_text_that_escapes_past_the_limit_is_left_out() {
    let fixture = Fixture::new();
    // Under MAX_TEXT as a file; a quote escapes to two bytes, a control
    // character to six.
    fixture.write("Draft", &"\u{1}".repeat(200_000));
    let published = out(&mut fixture.run());
    assert_eq!(published["current"]["too_large"], true);
}

#[test]
fn a_large_library_lists_the_newest_and_counts_the_rest() {
    let fixture = Fixture::new();
    for n in 0..510 {
        fixture.write(&format!("Note {n}"), "A note");
    }
    let published = out(&mut fixture.run());
    assert_eq!(published["documents"].as_array().unwrap().len(), 500);
    assert_eq!(published["hidden"], 11);
}

#[test]
fn a_document_put_back_under_a_new_name_keeps_its_edits_to_itself() {
    let fixture = Fixture::new();
    let mut run = fixture.run();
    out(&mut run);
    send(&mut run, json!({ "trash": "Draft" }));
    let token = opened(&send(&mut run, json!({ "open": "Draft" })));
    fixture.write("Draft", "Another document");
    let current = send(&mut run, json!({ "restore": "Draft" }));
    assert_eq!(current["current"]["id"], "Draft (2)");
    assert_eq!(opened(&current), token);
    // The editor still names the opening it was given, and its old name.
    let edit =
        json!({ "save": { "id": "Draft", "opened": token, "text": "Edited after restoring" } });
    send(&mut run, edit);
    run.kill();
    drop(fixture.run());
    assert_eq!(fixture.text("Draft"), "Another document");
    assert_eq!(fixture.text("Draft (2)"), "Edited after restoring");
}

#[test]
fn runs_of_one_profile_keep_their_own_recovery() {
    let fixture = Fixture::new();
    let mut first = fixture.run();
    let token = opened(&out(&mut first));
    send(
        &mut first,
        json!({ "save": { "id": "Draft", "opened": token, "text": "First run's edit" } }),
    );
    let second = fixture.run(); // while the first still goes
    assert_eq!(fixture.text("Draft"), "Original draft");
    assert_eq!(fixture.journals().len(), 1);
    second.kill();
    // The first run is still alive and writes its edit itself.
    first.flush().unwrap();
    assert_eq!(fixture.text("Draft"), "First run's edit");
    assert!(fixture.journals().is_empty());
}

#[test]
fn a_long_name_stays_valid_when_it_must_change() {
    let fixture = Fixture::new();
    let name = "L".repeat(81);
    fixture.write(&name, "Long");
    fs::write(fixture.trash().join(format!("{name}.md")), "Older long").unwrap();
    let mut run = fixture.run();
    out(&mut run);
    let ids = ids(&send(&mut run, json!({ "trash": name })));
    assert_eq!(ids.len(), 3);
    assert!(ids.iter().all(|id| id.len() <= 81));
    assert!(ids.contains(&format!("{} (2)", "L".repeat(77))));
}

#[test]
fn a_stale_recovery_never_overwrites_newer_writing() {
    let fixture = Fixture::new();
    let mut run = fixture.run();
    let token = opened(&out(&mut run));
    send(
        &mut run,
        json!({ "save": { "id": "Draft", "opened": token, "text": "Run A, never written" } }),
    );
    run.kill(); // A stops before writing; its journal stays
    assert_eq!(fixture.journals().len(), 1);
    // Meanwhile the file changes elsewhere.
    fixture.write("Draft", "Newer, saved by run B");
    drop(fixture.run());
    assert_eq!(fixture.text("Draft"), "Newer, saved by run B");
    assert_eq!(fixture.text("Draft (recovered)"), "Run A, never written");
    assert!(fixture.journals().is_empty());
}

#[test]
fn only_locked_files_are_published_as_locks() {
    let fixture = Fixture::new();
    let first = fixture.run();
    let locks = fixture.matching(".journals", "lock");
    assert_eq!(locks.len(), 1);
    assert!(fixture.matching(".journals", "locking").is_empty());
    // A second run's scan leaves a live run's lock in place.
    let second = fixture.run();
    assert!(locks[0].exists());
    drop((first, second));
}

#[test]
fn recovered_copies_never_replace_one_another() {
    let fixture = Fixture::new();
    fixture.write("Draft (recovered)", "Recovered earlier");
    let mut run = fixture.run();
    let token = opened(&send(&mut run, json!({ "open": "Draft" })));
    send(
        &mut run,
        json!({ "save": { "id": "Draft", "opened": token, "text": "Recovered now" } }),
    );
    run.kill();
    fixture.write("Draft", "Changed elsewhere");
    drop(fixture.run());
    assert_eq!(fixture.text("Draft (recovered)"), "Recovered earlier");
    assert_eq!(fixture.text("Draft (recovered) (2)"), "Recovered now");
    let names: Vec<_> = fs::read_dir(&fixture.folder)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !names.iter().any(|name| name.starts_with(".tmp-")),
        "{names:?}"
    );
}

#[test]
fn a_journal_written_by_the_python_provider_is_still_recovered() {
    let fixture = Fixture::new();
    let journals = fixture.folder.join(".journals");
    fs::create_dir_all(&journals).unwrap();
    let path = fixture.file("Draft");
    let base = digest(&path);
    let entry = json!({ "path": path.to_string_lossy(), "text": "From Python", "base": base });
    fs::write(journals.join("abc.json"), entry.to_string()).unwrap();
    fs::write(journals.join("abc.lock"), "").unwrap();
    drop(fixture.run());
    assert_eq!(fixture.text("Draft"), "From Python");
}

#[test]
fn a_new_library_starts_with_its_samples_the_first_newest() {
    let temp = tempdir().unwrap();
    let folder = temp.path().join("documents");
    let samples = [
        ("Newest".to_owned(), "# Newest".to_owned()),
        ("Older".to_owned(), "# Older".to_owned()),
    ];
    let mut run = Library::open(folder.clone(), &samples).unwrap();
    let published = out(&mut run);
    assert_eq!(published["documents"][0]["id"], "Newest");
    assert_eq!(published["documents"][1]["id"], "Older");
    assert_eq!(published["current"]["id"], "Newest");
    // Emptied by the user, it stays empty.
    send(&mut run, json!({ "trash": "Newest" }));
    send(&mut run, json!({ "trash": "Older" }));
    drop(run);
    let mut again = Library::open(folder, &samples).unwrap();
    assert!(
        out(&mut again)["documents"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["trashed"] == true)
    );
}

#[test]
fn names_that_could_leave_the_folder_are_refused() {
    for name in ["../secret", "a/b", ".hidden", "", "é"] {
        assert!(!valid(name), "{name}");
    }
    assert!(valid("Notes (2), v1.0 - it's"));
    let fixture = Fixture::new();
    let outside = fixture.folder.parent().unwrap().join("secret.md");
    fs::write(&outside, "Keep me").unwrap();
    let mut run = fixture.run();
    out(&mut run);
    send(&mut run, json!({ "purge": "../../secret" }));
    send(&mut run, json!({ "trash": "../secret" }));
    assert!(Path::new(&outside).exists());
}
