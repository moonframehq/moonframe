//! The `notes` native provider: a Commonplace vault for the app's mods,
//! shared as `app.notes`.
//!
//! Publishes a small summary: `{ available, reason?, vault = { path,
//! name }, status ("indexing" "ready"), counts = { all, notes, wiki, raw,
//! journal }, tags, today, today_path, import?, enhancing, version }`
//! (`enhancing`: the notes being enhanced). `version` goes
//! up each time the index changes.
//!
//! What a mod shows it asks for as a live query (see `views`):
//! `{ watch = { id, kind, ... } }` is answered `{ view = id, data }` now and
//! again whenever the answer changes; `{ unwatch = id }` stops it. Anyone
//! may watch.
//!
//! Changes, only from the mods in `[settings] writers`, each answered
//! `{ reply = id, action, ok, ... | error }` (see `actions`):
//! `save = { path, text, base? }` (written once typing stops, merged with
//! changes made elsewhere since `base`, the text the edit changed, else
//! since the text this editor last had; a write that fails is answered
//! `ok = false, kept = true` and tried again, and edits still waiting are
//! written when the provider stops; see `save`),
//! `create = { path, text? }`, `rename = { from, to }`, `trash = path`,
//! `capture = { text, to? = "inbox" | "today" }`, `inbox = { op = "keep" |
//! "task" | "note" | "drop", line }`, `today = true`, `import = { source =
//! "apple-notes", export?, into?, dry_run? }` (`export`: the folder of
//! Notes.app's Markdown export, joined with what Notes.app says), `enhance
//! = { path }` (titles for bare web links, favicons for their hosts; see
//! `enhance`), `open_vault = path`.
//!
//! The vault is `[settings] vault` (`~` allowed), else the one last opened
//! with `open_vault`, else `~/Commonplace`, made with a starting layout the
//! first time. Its index is in the cache folder, or at `[settings] index`.

// keep-sorted start
mod actions;
mod enhance;
mod save;
#[cfg(test)]
mod tests;
mod views;
mod watch;
// keep-sorted end

use {
    crate::Vault,
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, profile, to_json},
    save::Pending,
    serde_json::{Value, json},
    std::{
        collections::{BTreeMap, HashMap},
        env, fs,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
    views::View,
    watch::Watch,
};

/// How long without a keystroke before an edit is written.
const QUIET: Duration = Duration::from_millis(500);

/// A whole scan now and then, in case the watcher missed something.
const RESCAN: Duration = Duration::from_secs(300);

pub fn native() -> Box<dyn Native> {
    Box::new(Provider::default())
}

/// An import running on a thread of its own.
#[derive(Clone, Debug, Default)]
pub(crate) struct ImportState {
    pub running: bool,
    pub phase: String,
    pub done: usize,
    pub total: usize,
    pub report: Option<Value>,
    pub error: Option<String>,
    pub owner: String,
    pub reply: Value,
    pub finished: bool,
}

#[derive(Default)]
pub(crate) struct Provider {
    vault: Option<Vault>,
    watch: Option<Watch>,
    reason: Option<String>,
    indexing: bool,
    /// Bumped each time the index changes.
    version: u64,
    views: BTreeMap<(String, String), View>,
    /// Edits waiting to be written, by editor and note.
    pending: BTreeMap<save::Key, Pending>,
    /// The hash of what this provider last wrote to each note and the
    /// editor it wrote for, so that write coming back from the watcher is
    /// not sent to that editor (which would undo keystrokes made since);
    /// every other editor showing the note gets the new text.
    written: HashMap<String, (String, String)>,
    /// The text each editor was last known to have of each note, by
    /// editor and note: the base its next edit is merged from when it
    /// sends none.
    known: HashMap<save::Key, String>,
    /// The day views were last run for, so date-bound views (today's
    /// journal) move on at midnight.
    day: String,
    /// The day in tests, in place of the clock.
    #[cfg(test)]
    clock: Option<String>,
    import: Arc<Mutex<ImportState>>,
    enhance: Arc<Mutex<enhance::Enhancing>>,
    last_import: Option<String>,
    scanned: Option<Instant>,
    published: Option<String>,
}

impl Provider {
    fn writers(host: &Handle) -> Vec<String> {
        to_json(host.settings())
            .get("writers")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Opens (and the first time, makes) the vault and starts watching it.
    /// `keep` remembers it as the vault to open next time (one the user chose).
    fn open(&mut self, host: &Handle, root: PathBuf, keep: bool) {
        self.vault = None;
        self.watch = None;
        self.views.values_mut().for_each(|view| view.sent = None);
        // Paths are the old vault's: none of it applies to another.
        self.pending.clear();
        self.written.clear();
        self.known.clear();
        if !root.exists()
            && let Err(problem) = actions::starter(&root)
        {
            self.reason = Some(format!("making {}: {problem}", root.display()));
            return;
        }
        let index = to_json(host.settings())
            .get("index")
            .and_then(Value::as_str)
            .map(expand);
        let opened = match &index {
            Some(index) => Vault::open_with_index(&root, index),
            None => Vault::open(&root),
        };
        match opened {
            Ok(vault) => {
                self.watch = Watch::start(vault.root()).ok();
                self.vault = Some(vault);
                self.reason = None;
                self.indexing = true;
                if keep {
                    remember(host, &root);
                }
            },
            Err(problem) => self.reason = Some(format!("opening {}: {problem}", root.display())),
        }
    }

    fn summary(&self) -> Value {
        let Some(vault) = &self.vault else {
            return json!({ "available": false, "reason": self.reason.clone().unwrap_or_else(|| "no vault".to_string()) });
        };
        let index = vault.index();
        let count = |folder: &str| index.count(folder).unwrap_or(0);
        let today = self.today();
        let import = self
            .import
            .lock()
            .ok()
            .map(|state| state.clone())
            .unwrap_or_default();
        json!({
            "available": true,
            "vault": { "path": vault.root().display().to_string(), "name": vault.root().file_name().map(|name| name.to_string_lossy().into_owned()) },
            "status": if self.indexing { "indexing" } else { "ready" },
            "counts": { "all": count(""), "notes": count("notes"), "wiki": count("wiki"), "raw": count("raw"), "journal": count("journal") },
            "tags": index.tags().unwrap_or_default().into_iter().take(40).collect::<Vec<_>>(),
            "today": today,
            "today_path": format!("journal/{today}.md"),
            "import": if import.running || import.report.is_some() || import.error.is_some() {
                json!({ "running": import.running, "phase": import.phase, "done": import.done, "total": import.total, "report": import.report, "error": import.error })
            } else { Value::Null },
            "enhancing": self.enhancing(),
            "version": self.version,
        })
    }

    fn publish(&mut self, host: &Handle) {
        let summary = self.summary();
        let line = summary.to_string();
        if self.published.as_ref() != Some(&line) {
            host.publish_json(&summary);
            self.published = Some(line);
        }
    }

    /// Runs the views again and sends those whose answer changed.
    fn refresh_views(&mut self, host: &Handle, only: Option<&(String, String)>) {
        let today = self.today();
        if only.is_none() {
            self.day.clone_from(&today);
        }
        let Some(vault) = &self.vault else {
            return;
        };
        for (key, view) in self.views.iter_mut() {
            if only.is_some_and(|only| only != key) {
                continue;
            }
            let started = Instant::now();
            let answer = match views::run(vault, &view.query, &today) {
                Ok(data) => data,
                Err(problem) => json!({ "error": problem.to_string() }),
            };
            let kind = view.query.get("kind").and_then(Value::as_str).unwrap_or("");
            slow(
                "view",
                &format!("{}/{} {kind}", view.owner, view.id),
                started,
            );
            let print = views::fingerprint(&answer);
            if view.sent.as_ref() == Some(&print) {
                continue;
            }
            let first = view.sent.is_none();
            view.sent = Some(print);
            // A note whose new text is what this provider just wrote for
            // this same editor: it has it (or newer), so leave its text
            // alone. Any other editor showing the note takes the new text.
            let path = answer
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let hash = answer.get("hash").and_then(Value::as_str);
            let own_write = !first
                && self.written.get(&path).is_some_and(|(written, by)| {
                    Some(written.as_str()) == hash && *by == view.owner
                });
            let mut answer = answer;
            if own_write && let Some(object) = answer.as_object_mut() {
                object.remove("text");
                object.insert("own".to_string(), Value::Bool(true));
            } else if let Some(text) = answer.get("text").and_then(Value::as_str) {
                self.known
                    .insert((view.owner.clone(), path.clone()), text.to_string());
            }
            let message = json!({ "view": view.id, "data": answer });
            tracing::debug!(target: "commonplace", view = %view.id, owner = %view.owner, bytes = message.to_string().len(), "sent");
            send(host, &view.owner, message);
        }
    }

    fn changed(&mut self, host: &Handle) {
        self.version += 1;
        self.refresh_views(host, None);
        self.publish(host);
    }

    /// Today, `2026-11-10`: the clock's, or in tests the one set.
    fn today(&self) -> String {
        #[cfg(test)]
        if let Some(day) = &self.clock {
            return day.clone();
        }
        today()
    }
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let root = vault_folder(host);
        self.open(host, root, false);
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        let started = Instant::now();
        self.tick_inner(host);
        slow("tick", "", started);
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let started = Instant::now();
        let message = to_json(&message);
        let what = message
            .as_object()
            .and_then(|object| object.keys().find(|key| *key != "id" && *key != "again"))
            .cloned()
            .unwrap_or_default();
        let kind = message
            .get("watch")
            .and_then(|watch| watch.get("kind"))
            .and_then(Value::as_str)
            .map(|kind| format!(" {kind}"))
            .unwrap_or_default();
        self.receive_inner(host, from, message);
        slow("receive", &format!("{from}: {what}{kind}"), started);
    }
}

impl Provider {
    fn tick_inner(&mut self, host: &Handle) {
        let mut changed = false;
        if self.indexing {
            self.indexing = false;
            if let Some(vault) = &mut self.vault {
                if let Err(problem) = vault.scan() {
                    host.fail(format!("indexing: {problem}"));
                }
                self.scanned = Some(Instant::now());
            }
            changed = true;
        }
        changed |= self.flush(host, false);
        // Past midnight: today's views (the journal, Today) move on.
        if self.day != self.today() {
            changed = true;
        }
        let mut rescan = self.scanned.is_some_and(|at| at.elapsed() >= RESCAN);
        if let (Some(watch), Some(vault)) = (&self.watch, &mut self.vault) {
            let seen = watch.drain();
            rescan |= seen.rescan;
            for path in &seen.notes {
                if self.pending.keys().any(|(_, pending)| pending == path) {
                    continue;
                }
                if vault.refresh(path).is_ok() {
                    changed = true;
                }
            }
        }
        if rescan && let Some(vault) = &mut self.vault {
            if vault
                .scan()
                .is_ok_and(|report| report.parsed + report.removed > 0)
            {
                changed = true;
            }
            self.scanned = Some(Instant::now());
        }
        if self.import_finished(host) {
            changed = true;
        }
        changed |= self.enhance_finished(host);
        if changed {
            self.changed(host);
        } else {
            self.publish(host);
        }
    }

    fn receive_inner(&mut self, host: &Handle, from: &str, message: Value) {
        if let Some(watch) = message.get("watch") {
            let id = watch
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("view")
                .to_string();
            let key = (from.to_string(), id.clone());
            let unchanged = self
                .views
                .get(&key)
                .is_some_and(|view| view.query == *watch);
            if !unchanged {
                self.views.insert(key.clone(), View {
                    owner: from.to_string(),
                    id,
                    query: watch.clone(),
                    sent: None,
                });
            } else if message
                .get("again")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && let Some(view) = self.views.get_mut(&key)
            {
                // A mod that lost what it had (reloaded): answer again. Asked
                // the same with no `again`, it already has the answer.
                view.sent = None;
            }
            self.refresh_views(host, Some(&key));
            return;
        }
        if let Some(id) = message.get("unwatch").and_then(Value::as_str) {
            self.views.remove(&(from.to_string(), id.to_string()));
            return;
        }
        let reply = message.get("id").cloned().unwrap_or(Value::Null);
        if !Self::writers(host).iter().any(|writer| writer == from) {
            send(
                host,
                from,
                json!({ "reply": reply, "ok": false, "error": format!("{from} may not change the vault (providers/notes [settings] writers)") }),
            );
            return;
        }
        if let Some(save) = message.get("save") {
            let path = save
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let text = save
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let base = save.get("base").and_then(Value::as_str).map(str::to_string);
            self.hold(from, path, text, base, reply);
            return;
        }
        if let Some(path) = message.get("open_vault").and_then(Value::as_str) {
            self.flush(host, true);
            self.open(host, expand(path), true);
            send(
                host,
                from,
                json!({ "reply": reply, "action": "open_vault", "ok": self.vault.is_some(), "error": self.reason }),
            );
            self.publish(host);
            return;
        }
        if let Some(import) = message.get("import") {
            self.start_import(host, from, reply, import);
            return;
        }
        if let Some(enhance) = message.get("enhance") {
            self.start_enhance(host, from, reply, enhance);
            return;
        }
        // Everything else changes files now: write waiting edits first.
        self.flush(host, true);
        let today = self.today();
        let Some(vault) = &mut self.vault else {
            send(
                host,
                from,
                json!({ "reply": reply, "ok": false, "error": "no vault" }),
            );
            return;
        };
        let answer = actions::act(vault, &message, &today);
        let mut answer = match answer {
            Ok(answer) => answer,
            Err(problem) => json!({ "ok": false, "error": problem.to_string() }),
        };
        if let Some(object) = answer.as_object_mut() {
            object.insert("reply".to_string(), reply);
        }
        send(host, from, answer);
        self.changed(host);
    }
}

impl Provider {
    fn start_import(&mut self, host: &Handle, from: &str, reply: Value, import: &Value) {
        let source = import.get("source").and_then(Value::as_str).unwrap_or("");
        if source != "apple-notes" {
            send(
                host,
                from,
                json!({ "reply": reply, "action": "import", "ok": false, "error": format!("no importer for {source:?}") }),
            );
            return;
        }
        let Some(vault) = &self.vault else {
            return;
        };
        if self.import.lock().is_ok_and(|state| state.running) {
            send(
                host,
                from,
                json!({ "reply": reply, "action": "import", "ok": false, "error": "an import is already running" }),
            );
            return;
        }
        let into = import
            .get("into")
            .and_then(Value::as_str)
            .unwrap_or("notes")
            .to_string();
        let export = import
            .get("export")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .map(expand);
        let dry_run = import
            .get("dry_run")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let root = vault.root().to_path_buf();
        let state = Arc::clone(&self.import);
        if let Ok(mut state) = state.lock() {
            *state = ImportState {
                running: true,
                phase: "Reading Notes.app".to_string(),
                owner: from.to_string(),
                reply,
                ..ImportState::default()
            };
        }
        let worker = Arc::clone(&state);
        std::thread::spawn(move || {
            let result =
                actions::import_apple_notes(&root, &into, export.as_deref(), dry_run, &worker);
            if let Ok(mut state) = worker.lock() {
                state.running = false;
                state.finished = true;
                match result {
                    Ok(report) => state.report = Some(report),
                    Err(problem) => state.error = Some(problem.to_string()),
                }
            }
        });
        self.publish(host);
    }

    /// Answers the mod that asked once an import ends; true when it wrote notes.
    fn import_finished(&mut self, host: &Handle) -> bool {
        let Ok(mut state) = self.import.lock() else {
            return false;
        };
        let progress = format!("{}/{}/{}", state.phase, state.done, state.total);
        let moved = self.last_import.as_ref() != Some(&progress);
        self.last_import = Some(progress);
        if !state.finished {
            drop(state);
            if moved {
                self.publish(host);
            }
            return false;
        }
        state.finished = false;
        let answer = json!({ "reply": state.reply, "action": "import", "ok": state.error.is_none(), "report": state.report, "error": state.error });
        let owner = state.owner.clone();
        drop(state);
        send(host, &owner, answer);
        if let Some(vault) = &mut self.vault {
            let _ = vault.scan();
        }
        true
    }
}

/// Logs how long `what` took: at debug always, at info past 30 ms, so a
/// slow step shows without asking (`RUST_LOG=commonplace=debug` for all).
fn slow(what: &str, detail: &str, started: Instant) {
    let took = started.elapsed();
    let ms = took.as_secs_f64() * 1000.0;
    if took >= Duration::from_millis(30) {
        tracing::info!(target: "commonplace", ms = format!("{ms:.1}"), "{what} {detail}");
    } else {
        tracing::debug!(target: "commonplace", ms = format!("{ms:.1}"), "{what} {detail}");
    }
}

fn send(host: &Handle, to: &str, value: Value) {
    match moonframe_providers::from_json(&value) {
        Ok(data) => host.send(to, data),
        Err(problem) => host.send(
            to,
            moonframe_providers::from_json(&json!({ "error": problem.to_string() }))
                .unwrap_or_default(),
        ),
    }
}

/// Today in the local time zone, `2026-11-10`.
fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn home() -> PathBuf {
    PathBuf::from(
        env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .unwrap_or_default(),
    )
}

fn expand(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None if path == "~" => home(),
        None => PathBuf::from(path),
    }
}

/// The file remembering the vault last opened, per profile.
fn memory(host: &Handle) -> PathBuf {
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home().join(".local/share"));
    let name: String = profile()
        .unwrap_or(host.name())
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    data.join("moonframe").join(name).join("notes-vault")
}

fn remember(host: &Handle, root: &Path) {
    let file = memory(host);
    if let Some(folder) = file.parent() {
        let _ = fs::create_dir_all(folder);
    }
    let _ = fs::write(file, root.display().to_string());
}

fn vault_folder(host: &Handle) -> PathBuf {
    if let Some(path) = to_json(host.settings())
        .get("vault")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
    {
        return expand(path);
    }
    if let Ok(remembered) = fs::read_to_string(memory(host)) {
        let remembered = PathBuf::from(remembered.trim());
        if remembered.is_dir() {
            return remembered;
        }
    }
    home().join("Commonplace")
}
