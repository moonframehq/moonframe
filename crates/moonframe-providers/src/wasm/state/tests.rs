//! A real component in each test: text the runtime compiles, so no toolchain
//! is needed. It counts in one byte of its memory, as a digit, and publishes
//! that digit each tick and message.

use {
    crate::{Event, Providers},
    moonframe_core::Data,
    std::{
        fs,
        path::Path,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::Duration,
    },
};

const OK: &str = "(i32.const 32)";
const COPY: &str = "(i32.store8 (i32.const 0) (i32.load8_u (local.get 0))) (i32.const 32)";
const REFUSE: &str = "(i32.store8 (i32.const 40) (i32.const 1))
    (i32.store (i32.const 44) (i32.const 64))
    (i32.store (i32.const 48) (i32.const 2))
    (i32.const 40)";
const TRAP: &str = "unreachable";
const PUBLISH: &str = "(call $count)";
const STATE_EXPORTS: &str = r#"
  (func (export "save") (result (option string))
    (canon lift (core func $i "save") (memory $memory) (realloc $realloc)))
  (func (export "restore") (param "state" string) (result (result (error string)))
    (canon lift (core func $i "restore") (memory $memory) (realloc $realloc)))"#;
/// `save` of the wrong type: nothing comes back.
const WRONG_EXPORTS: &str = r#"
  (func (export "save") (canon lift (core func $i "nothing")))
  (func (export "restore") (param "state" string) (result (result (error string)))
    (canon lift (core func $i "restore") (memory $memory) (realloc $realloc)))"#;

/// In place of `$spin`, which does nothing: a loop long enough that a new
/// build is ready well before the call ends, and well within a call's fuel.
const SPIN: &str = "(func $spin (local $left i32)
      (local.set $left (i32.const 100000000))
      (loop $again
        (local.set $left (i32.sub (local.get $left) (i32.const 1)))
        (br_if $again (local.get $left))))";

/// A component whose `restore` and `tick` run `restore` and `tick`, with
/// `exports` for the state exports. `build` tells builds apart.
fn component(build: u32, restore: &str, tick: &str, exports: &str) -> String {
    format!(
        r#";; build {build}
(component
  (core module $m
    (memory (export "memory") 1)
    (global $heap (mut i32) (i32.const 4096))
    (data (i32.const 0) "0")
    (data (i32.const 64) "no")
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $at i32)
      (local.set $at (global.get $heap))
      (global.set $heap (i32.add (local.get $at) (local.get 3)))
      (local.get $at))
    (func $spin)
    (func $count (result i32)
      (i32.store8 (i32.const 16) (i32.const 1))
      (i32.store (i32.const 20) (i32.const 0))
      (i32.store (i32.const 24) (i32.const 1))
      (i32.const 16))
    (func (export "start") (param i32 i32) (result i32) (i32.const 32))
    (func (export "tick") (result i32) {tick})
    (func (export "receive") (param i32 i32) (result i32)
      (i32.store8 (i32.const 0) (i32.add (i32.load8_u (i32.const 0)) (i32.const 1)))
      (call $spin)
      (call $count))
    (func (export "save") (result i32) (call $count))
    (func (export "nothing"))
    (func (export "restore") (param i32 i32) (result i32) {restore}))
  (core instance $i (instantiate $m))
  (alias core export $i "memory" (core memory $memory))
  (alias core export $i "cabi_realloc" (core func $realloc))
  (func (export "start") (param "settings" string) (result (result (error string)))
    (canon lift (core func $i "start") (memory $memory) (realloc $realloc)))
  (func (export "tick") (result (option string))
    (canon lift (core func $i "tick") (memory $memory) (realloc $realloc)))
  (func (export "receive") (param "message" string) (result (option string))
    (canon lift (core func $i "receive") (memory $memory) (realloc $realloc)))
  {exports})"#
    )
}

fn good(build: u32) -> String {
    component(build, COPY, PUBLISH, STATE_EXPORTS)
}

/// One provider in a temporary folder, and what it reports.
struct Harness {
    dir: tempfile::TempDir,
    providers: Providers,
    received: mpsc::Receiver<Event>,
    statuses: mpsc::Receiver<moonframe_core::entries::EntryStatus>,
}

impl Harness {
    fn start(wasm: &str) -> Self {
        Self::start_with(wasm, |_| {})
    }

    fn start_with(wasm: &str, before_event: impl Fn(&Event) + Send + Sync + 'static) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("providers/count");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("provider.toml"),
            "kind = 'wasm'\ninterval = 60\nstart = 'always'\n",
        )
        .unwrap();
        fs::write(folder.join("provider.wasm"), wasm).unwrap();
        let (sender, received) = mpsc::channel();
        let sender = Arc::new(Mutex::new(sender));
        let (status, statuses) = mpsc::channel();
        let providers = Providers::start_configured(
            dir.path().into(),
            dir.path().join("build"),
            Arc::new(move |event| {
                before_event(&event);
                let _ = sender.lock().unwrap().send(event);
            }),
            Default::default(),
            None,
            crate::ProviderOptions {
                on_entry_status: Some(Arc::new(move |s| {
                    let _ = status.send(s);
                })),
                ..Default::default()
            },
        );
        Self {
            dir,
            providers,
            received,
            statuses,
        }
    }

    fn file(&self) -> std::path::PathBuf {
        Path::new(self.dir.path()).join("providers/count/provider.wasm")
    }

    /// Installs a new build, as a rebuild would.
    fn install(&mut self, wasm: &str) {
        fs::write(self.file(), wasm).unwrap();
        self.providers.reload();
    }

    fn next(&self) -> Event {
        self.received.recv_timeout(Duration::from_secs(20)).unwrap()
    }

    /// The digit it publishes next.
    fn count(&self) -> i64 {
        match self.next() {
            Event::Published {
                data: Data::Integer(count),
                ..
            } => count,
            other => panic!("{other:?}"),
        }
    }

    fn failed(&self) -> String {
        loop {
            let status = self.statuses.recv_timeout(Duration::from_secs(20)).unwrap();
            if let Some(error) = status.error {
                if status.state == moonframe_core::entries::EntryState::Failed {
                    assert!(matches!(self.next(), Event::Failed { .. }));
                }
                return error;
            }
        }
    }

    /// Counts up `by`, and checks where it got to.
    fn add(&mut self, by: i64) -> i64 {
        let mut count = 0;
        for _ in 0..by {
            self.providers
                .send("count", "main", Data::from("add"))
                .unwrap();
            count = self.count();
        }
        count
    }

    /// Stops inside publication of the first message, before the old build
    /// can save. The caller releases it after installing replacements.
    fn blocked() -> (Self, mpsc::Sender<()>) {
        let (entered, waiting) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let released = Mutex::new(released);
        let blocked = AtomicBool::new(false);
        let mut harness = Self::start_with(&good(1), move |event| {
            if matches!(event, Event::Published {
                data: Data::Integer(1),
                ..
            }) && !blocked.swap(true, Ordering::Relaxed)
            {
                entered.send(()).unwrap();
                released
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(20))
                    .unwrap();
            }
        });
        assert_eq!(harness.count(), 0);
        harness
            .providers
            .send("count", "main", Data::from("add"))
            .unwrap();
        waiting.recv_timeout(Duration::from_secs(20)).unwrap();
        (harness, release)
    }

    fn quiet(&self) {
        assert!(matches!(
            self.received.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
    }
}

#[test]
fn a_new_build_keeps_the_state() {
    let mut harness = Harness::start(&good(1));
    assert_eq!(harness.count(), 0);
    assert_eq!(harness.add(3), 3);
    harness.install(&good(2));
    assert_eq!(harness.count(), 3);
    assert_eq!(harness.add(1), 4);
    harness.install(&good(3));
    assert_eq!(harness.count(), 4);
}

#[test]
fn a_new_build_waits_for_the_old_one_to_save() {
    let mut harness = Harness::start(&good(1).replace("(func $spin)", SPIN));
    assert_eq!(harness.count(), 0);
    harness
        .providers
        .send("count", "main", Data::from("add"))
        .unwrap();
    // The old build is in that call, and saves only once it returns.
    harness.install(&good(2));
    assert_eq!(harness.count(), 1);
}

#[test]
fn a_failed_replacement_keeps_waiting_for_the_original_build() {
    let (mut harness, release) = Harness::blocked();
    harness.install("not a component");
    harness.failed();
    harness.install(&good(3));
    harness.quiet();
    release.send(()).unwrap();
    assert_eq!(harness.count(), 1); // The old publication, then the restored one.
    assert_eq!(harness.count(), 1);
}

#[test]
fn a_cancelled_replacement_keeps_waiting_for_the_original_build() {
    let (mut harness, release) = Harness::blocked();
    harness.install(&good(2));
    harness.quiet();
    harness.install(&good(3));
    harness.quiet();
    release.send(()).unwrap();
    assert_eq!(harness.count(), 1);
    assert_eq!(harness.count(), 1);
}

#[test]
fn a_provider_without_the_exports_starts_over() {
    let plain = |build| component(build, OK, PUBLISH, "");
    let mut harness = Harness::start(&plain(1));
    assert_eq!(harness.count(), 0);
    assert_eq!(harness.add(3), 3);
    harness.install(&plain(2));
    assert_eq!(harness.count(), 0);
}

#[test]
fn a_build_that_cannot_load_leaves_the_state_for_the_next() {
    let mut harness = Harness::start(&good(1));
    assert_eq!(harness.count(), 0);
    assert_eq!(harness.add(3), 3);
    harness.install("not a component");
    harness.failed();
    assert_eq!(harness.add(1), 4); // The last good revision is still answering.
    harness.install(&good(2));
    assert_eq!(harness.count(), 4);
}

#[test]
fn a_build_that_panics_on_its_first_tick_leaves_the_state_for_the_next() {
    let mut harness = Harness::start(&good(1));
    assert_eq!(harness.count(), 0);
    assert_eq!(harness.add(3), 3);
    harness.install(&component(2, COPY, TRAP, STATE_EXPORTS));
    harness.failed();
    harness.install(&good(3));
    assert_eq!(harness.count(), 3);
}

#[test]
fn a_build_that_panics_in_restore_leaves_the_state_for_the_next() {
    let mut harness = Harness::start(&good(1));
    assert_eq!(harness.count(), 0);
    assert_eq!(harness.add(3), 3);
    harness.install(&component(2, TRAP, PUBLISH, STATE_EXPORTS));
    let problem = harness.failed();
    assert!(problem.contains("restore"), "{problem}");
    harness.install(&good(3));
    assert_eq!(harness.count(), 3);
}

#[test]
fn a_refused_state_starts_fresh_and_is_gone() {
    let mut harness = Harness::start(&good(1));
    assert_eq!(harness.count(), 0);
    assert_eq!(harness.add(3), 3);
    harness.install(&component(2, REFUSE, PUBLISH, STATE_EXPORTS));
    assert_eq!(harness.count(), 0);
    assert_eq!(harness.add(1), 1);
    harness.install(&good(3));
    assert_eq!(harness.count(), 1);
}

#[test]
fn exports_of_the_wrong_type_are_a_problem() {
    let harness = Harness::start(&component(1, COPY, PUBLISH, WRONG_EXPORTS));
    let problem = harness.failed();
    assert!(problem.contains("provider.wit"), "{problem}");
}

#[test]
fn old_components_importing_only_the_original_host_interface_still_load() {
    let legacy = good(1).replacen(
        "(component",
        r#"(component
      (import "moonframe:provider/host" (instance
        (export "log" (func (param "message" string)))
        (export "system-load" (func (result (result string (error string)))))))"#,
        1,
    );
    let harness = Harness::start(&legacy);
    assert_eq!(harness.count(), 0);
}

#[test]
fn shutdown_waits_for_the_last_good_worker_under_a_failed_candidate() {
    let mut harness = Harness::start(&good(1));
    assert_eq!(harness.count(), 0);
    harness.add(2);
    harness.install("not a component");
    harness.failed();
    harness.providers.shutdown();
    let folder = harness.dir.path().join("providers/count");
    assert_eq!(harness.providers.kept.get(&folder), Some("2".into()));
}

#[test]
fn failed_build_and_failed_instantiation_keep_the_old_worker_answering() {
    let mut harness = Harness::start(&good(1));
    assert_eq!(harness.count(), 0);
    let dir = harness.file().parent().unwrap().to_owned();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = 'broken'\nversion = '0.1.0'",
    )
    .unwrap();
    fs::write(dir.join("build.rs"), "fn main() {}").unwrap();
    harness.providers.reload();
    assert!(harness.failed().contains("build.rs"));
    assert_eq!(harness.add(1), 1);
    fs::remove_file(dir.join("Cargo.toml")).unwrap();
    fs::remove_file(dir.join("build.rs")).unwrap();
    let missing_import = good(2).replacen(
        "(component",
        r#"(component (import "not-provided" (func))"#,
        1,
    );
    harness.install(&missing_import);
    assert!(harness.failed().contains("not-provided"));
    assert_eq!(harness.add(1), 2);
    harness.install(&good(3));
    assert_eq!(harness.count(), 2);
}

#[test]
fn namespaced_state_survives_shutdown_restart_and_path_changes() {
    use moonframe_core::entries::{Entries, RootPolicy};
    let root = tempfile::tempdir().unwrap();
    let entries = Entries::default();
    entries
        .add_root("user", root.path(), RootPolicy::default())
        .unwrap();
    let (send, recv) = mpsc::channel();
    let events: crate::Events = Arc::new(move |event| {
        let _ = send.send(event);
    });
    let options = crate::ProviderOptions {
        state_dir: Some(root.path().join("state")),
        ..Default::default()
    };
    let count = || match recv.recv_timeout(Duration::from_secs(20)).unwrap() {
        Event::Published {
            data: Data::Integer(n),
            ..
        } => n,
        other => panic!("{other:?}"),
    };
    for (path, expected) in [("first", 0), ("moved", 2)] {
        let dir = root.path().join(path).join("provider");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("provider.toml"), "kind = 'wasm'\ninterval = 60").unwrap();
        fs::write(dir.join("provider.wasm"), good(1)).unwrap();
        entries
            .add_entry("user", "count", dir.parent().unwrap())
            .unwrap();
        let mut providers = Providers::start_configured(
            entries.apply(root.path().into()),
            root.path().join("build"),
            events.clone(),
            Default::default(),
            None,
            options.clone(),
        );
        assert_eq!(count(), expected);
        for n in 1..=2 {
            providers.send("user:count", "user:mod", Data::Nil).unwrap();
            assert_eq!(count(), expected + n);
        }
        fs::write(dir.join("provider.wasm"), good(2)).unwrap();
        providers.reload();
        assert_eq!(count(), expected + 2);
        providers.shutdown();
        assert_eq!(
            fs::read_to_string(root.path().join("state/user/count.json")).unwrap(),
            (expected + 2).to_string()
        );
    }
}
