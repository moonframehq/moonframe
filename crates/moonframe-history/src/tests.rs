#![allow(clippy::unwrap_used)]

use {
    super::*,
    std::{fs, path::PathBuf},
    tempfile::TempDir,
};

struct Fixture {
    _dirs: TempDir,
    folder: PathBuf,
    history: History,
}

fn fixture() -> Fixture {
    let dirs = tempfile::tempdir().unwrap();
    let folder = dirs.path().join("folder");
    fs::create_dir_all(folder.join("mods/clock")).unwrap();
    fs::write(folder.join("main.luau"), "-- shell\n").unwrap();
    let history = History::open(&folder, &dirs.path().join("state/history.git")).unwrap();
    Fixture {
        folder: folder.canonicalize().unwrap(),
        _dirs: dirs,
        history,
    }
}

impl Fixture {
    fn write(&self, path: &str, text: &str) {
        fs::write(self.folder.join(path), text).unwrap();
    }

    fn read(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.folder.join(path)).ok()
    }
}

#[test]
fn a_capture_with_nothing_changed_takes_no_snapshot() {
    let fixture = fixture();
    let first = fixture.history.capture("launch").unwrap().unwrap();
    assert_eq!(first.label, "main.luau");
    assert_eq!(first.reason, "launch");
    assert!(first.current);
    assert_eq!(fixture.history.capture("changed").unwrap(), None);
}

#[test]
fn undo_and_redo_walk_the_snapshots_and_write_the_files() {
    let fixture = fixture();
    fixture.history.capture("launch").unwrap();
    fixture.write("mods/clock/init.luau", "return 1\n");
    let added = fixture.history.capture("changed").unwrap().unwrap();
    assert_eq!(added.label, "mods/clock/init.luau");
    fixture.write("main.luau", "-- edited\n");
    fixture.history.capture("changed").unwrap().unwrap();

    let undone = fixture.history.undo().unwrap().unwrap();
    assert_eq!(undone.label, "main.luau");
    assert_eq!(fixture.read("main.luau").as_deref(), Some("-- shell\n"));
    let undone = fixture.history.undo().unwrap().unwrap();
    assert_eq!(undone.id, added.id);
    assert_eq!(fixture.read("mods/clock/init.luau"), None);
    // Nothing before the first snapshot.
    assert_eq!(fixture.history.undo().unwrap(), None);

    let redone = fixture.history.redo().unwrap().unwrap();
    assert_eq!(redone.id, added.id);
    assert_eq!(
        fixture.read("mods/clock/init.luau").as_deref(),
        Some("return 1\n")
    );
    fixture.history.redo().unwrap().unwrap();
    assert_eq!(fixture.read("main.luau").as_deref(), Some("-- edited\n"));
    assert_eq!(fixture.history.redo().unwrap(), None);
}

#[test]
fn undo_takes_back_a_change_no_snapshot_saw_yet() {
    let fixture = fixture();
    fixture.history.capture("launch").unwrap();
    fixture.write("main.luau", "-- unseen\n");
    let undone = fixture.history.undo().unwrap().unwrap();
    assert_eq!(undone.reason, "before undo");
    assert_eq!(fixture.read("main.luau").as_deref(), Some("-- shell\n"));
    // And redo brings it back: nothing was lost.
    fixture.history.redo().unwrap().unwrap();
    assert_eq!(fixture.read("main.luau").as_deref(), Some("-- unseen\n"));
}

#[test]
fn a_change_after_an_undo_starts_a_new_line() {
    let fixture = fixture();
    fixture.history.capture("launch").unwrap();
    fixture.write("main.luau", "-- one\n");
    fixture.history.capture("changed").unwrap();
    fixture.history.undo().unwrap().unwrap();
    fixture.write("main.luau", "-- two\n");
    fixture.history.capture("changed").unwrap().unwrap();
    assert_eq!(fixture.history.redo().unwrap(), None);
    let listed = fixture.history.list(10).unwrap();
    let labels: Vec<&str> = listed.iter().map(|s| s.reason.as_str()).collect();
    assert_eq!(labels, ["changed", "launch"]);
    assert!(listed[0].current);
}

#[test]
fn what_a_window_keeps_is_left_as_it_is() {
    let fixture = fixture();
    fixture.write("settings.toml", "theme = \"one\"\n");
    fs::create_dir_all(fixture.folder.join("packages/clone")).unwrap();
    fixture.write("packages/clone/init.luau", "return 0\n");
    fixture.history.capture("launch").unwrap();
    fixture.write("main.luau", "-- edited\n");
    fixture.history.capture("changed").unwrap();
    fixture.write("settings.toml", "theme = \"two\"\n");
    fixture.history.undo().unwrap().unwrap();
    assert_eq!(fixture.read("main.luau").as_deref(), Some("-- shell\n"));
    assert_eq!(
        fixture.read("settings.toml").as_deref(),
        Some("theme = \"two\"\n")
    );
    assert!(fixture.folder.join("packages/clone/init.luau").is_file());
}

#[test]
fn restore_goes_to_any_snapshot_and_can_be_undone() {
    let fixture = fixture();
    let first = fixture.history.capture("launch").unwrap().unwrap();
    fixture.write("main.luau", "-- one\n");
    fixture.history.capture("changed").unwrap();
    fixture.write("main.luau", "-- two\n");
    fixture.history.capture("changed").unwrap();

    let restored = fixture.history.restore(first.short()).unwrap();
    assert_eq!(restored.id, first.id);
    assert!(restored.current);
    assert_eq!(fixture.read("main.luau").as_deref(), Some("-- shell\n"));
    assert!(fixture.history.restore("not-an-id").is_err());
    assert!(
        fixture
            .history
            .show(&first.id)
            .unwrap()
            .contains("-- shell")
    );
}

#[test]
fn a_repository_the_folder_is_in_is_not_touched() {
    let fixture = fixture();
    let status = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&fixture.folder)
        .status()
        .unwrap();
    assert!(status.success());
    fixture.history.capture("launch").unwrap();
    fixture.write("main.luau", "-- edited\n");
    fixture.history.capture("changed").unwrap();
    fixture.history.undo().unwrap().unwrap();
    // The folder's own repository has no commits, index, or refs of ours.
    let git = fixture.folder.join(".git");
    assert!(!git.join("index").exists());
    assert!(!git.join("refs/moonframe").exists());
    assert!(!git.join("refs/heads/history").exists());
}

#[test]
fn labels_name_a_few_files_and_count_the_rest() {
    assert_eq!(label(["a"].into_iter()), "a");
    assert_eq!(label(["a", "b"].into_iter()), "a, b");
    assert_eq!(label(["a", "b", "c", "d"].into_iter()), "a, b and 2 more");
    assert_eq!(label(std::iter::empty()), "no files");
}

#[test]
fn each_folder_keeps_a_store_of_its_own() {
    let dirs = tempfile::tempdir().unwrap();
    let (one, two) = (dirs.path().join("one"), dirs.path().join("two"));
    fs::create_dir_all(&one).unwrap();
    fs::create_dir_all(&two).unwrap();
    let state = dirs.path().join("state");
    assert_ne!(store_in(&state, &one), store_in(&state, &two));
    assert_eq!(store_in(&state, &one), store_in(&state, &one));
    assert!(store_in(&state, &one).starts_with(state.join("history")));
}

#[test]
fn a_store_is_never_used_for_another_folder() {
    let fixture = fixture();
    let other = fixture._dirs.path().join("other");
    fs::create_dir_all(&other).unwrap();
    let error = History::open(&other, &fixture.history.git.dir).unwrap_err();
    assert!(
        error.to_string().contains("keeps the history of"),
        "{error}"
    );
    // Its own folder opens it again.
    History::open(&fixture.folder, &fixture.history.git.dir).unwrap();
}

#[cfg(unix)]
#[test]
fn only_its_owner_can_read_the_store() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = fixture();
    let store = &fixture.history.git.dir;
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(store), 0o700);
    // Reopening puts it right again.
    fs::set_permissions(store, fs::Permissions::from_mode(0o755)).unwrap();
    History::open(&fixture.folder, store).unwrap();
    assert_eq!(mode(store), 0o700);
}

#[test]
fn another_process_waits_for_an_operation_to_finish() {
    let fixture = fixture();
    fixture.history.capture("launch").unwrap();
    // A second opening, as `moonframe history` beside the window has.
    let other = History::open(&fixture.folder, &fixture.history.git.dir).unwrap();
    let held = fixture.history.lock().unwrap();
    fixture.write("main.luau", "-- edited\n");
    let (sender, done) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| sender.send(other.capture("changed")).unwrap());
        assert!(
            done.recv_timeout(std::time::Duration::from_millis(300))
                .is_err()
        );
        drop(held);
        let taken = done
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        assert_eq!(taken.unwrap().unwrap().reason, "changed");
    });
}
