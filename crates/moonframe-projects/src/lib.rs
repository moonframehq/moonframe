//! The `projects` native provider, for apps that work across many
//! repositories: every git repository under the project folders, with its
//! branch and changes, and for the one an app has open, its changed files,
//! its folders and one file's text. It runs git with the user's access, and
//! reads only inside the open project.
//!
//! Publishes `app.projects`:
//!
//! - `now`: the time, in Unix seconds, to the minute (views have no clock)
//! - `roots`: the project folders looked in, as `~/...`
//! - `projects`: up to [`MAX_PROJECTS`], most recently active first: `{ path,
//!   name, home (its project folder, as ~/...), branch, files, added,
//!   deleted, ahead, behind, time, subject, worktrees }`; `worktrees` are the
//!   repository's work trees, the main one first: `{ path, branch, primary }`
//! - `current`: the open project (the most recently active one until a
//!   message selects another), or absent: `{ path, name, home, branch, scope,
//!   added, deleted, count, truncated, files = { "<status>\t<added>\t
//!   <deleted>\t<path>" }, log = { { sha, subject, author, time } } }`; status
//!   is M, A, D or R, and untracked files are A
//! - `dirs`: the open project's open folders, by path from its root ("" is
//!   the root): `{ "<kind><name>" }`, kind d (a folder), f (a file), and D or
//!   F when git ignores it
//! - `file`: the file being read: `{ path, lines, total, truncated, binary,
//!   missing }`
//! - `forge`: what `gh` says of the open project, once asked: `{ path, prs =
//!   { { number, title, branch, author, draft, updated } }, runs = { { id,
//!   title, workflow, branch, status, conclusion, created } }, error? }`
//! - `action`: the last write's outcome: `{ id, action, ok, message | error }`
//! - `running`: the write under way, by its action's name, or absent
//!
//! Messages, any combination of keys in one table: `select = "<path>"` opens
//! that project; `scope = "uncommitted" | "branch" | "last"` says which
//! changes `current` lists; `open = "a\na/b"` the open folders, one per line;
//! `complete = "a/b", cwd = "<project>"` also lists this folder for a
//! composer, independent of `open`, until the project changes; `read =
//! "<path>"` reads that file of the open project, false stops; `refresh =
//! true` looks again now; `forge = true` asks `gh` for pull requests and
//! workflow runs, again each minute while asked, false stops.
//!
//! Writes, only from the mods named in `[settings] writers` (or the
//! comma-separated `MOONFRAME_PROJECTS_WRITERS`), one at a time and in the
//! background, so a signing key waiting for a tap holds nothing up: `commit =
//! "<message>", id = ...` stages everything in the open project and commits
//! it; `pull = true`, `push = true`; `worktree = "<name>", repo = "<path>"`
//! adds a work tree on a new branch `<name>`, in
//! `~/.moonframe/worktrees/<repo>/<name>`.
//!
//! The project folders are `MOONFRAME_PROJECTS` (colon-separated), else those
//! of `~/code ~/dev ~/src ~/Projects ~/projects ~/work ~/.herdr/worktrees`
//! that exist. Each is searched two levels deep.

mod git;
mod jobs;
mod kept;
mod open;
mod scan;

use {
    jobs::{Done, Finished},
    kept::Kept,
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, profile, to_json},
    open::Reader,
    scan::{Found, Summaries},
    serde_json::{Map, Value, json},
    std::{
        env,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    },
};

/// Projects listed.
const MAX_PROJECTS: usize = 40;
/// Open folders listed.
pub(crate) const MAX_DIRS: usize = 60;
/// Work trees listed per project.
pub(crate) const MAX_WORKTREES: usize = 12;
/// Seconds between asks to `gh`.
const FORGE_EVERY: u64 = 60;
/// Seconds between looks for projects.
const SCAN_EVERY: u64 = 15;
/// How often the open project is looked at.
const LOOK_EVERY: Duration = Duration::from_secs(3);

pub fn native() -> Box<dyn Native> {
    Box::new(Provider::default())
}

/// `path` with the home folder as `~`.
pub(crate) fn tilde(path: &str, home: &str) -> String {
    match path.strip_prefix(home) {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
            format!("~{rest}")
        },
        _ => path.to_owned(),
    }
}

/// `path` with a leading `~` as the home folder.
fn expand(path: &str, home: &str) -> String {
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{home}{rest}"),
        _ => path.to_owned(),
    }
}

/// The last part of a path, or `None` when it ends in a slash.
pub(crate) fn last_name(path: &str) -> Option<&str> {
    path.rsplit('/').next().filter(|name| !name.is_empty())
}

/// True as the app meant it: not false, nil, 0 or an empty string.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null | Value::Bool(false)) => false,
        Some(Value::Number(number)) => number.as_f64() != Some(0.0),
        Some(Value::String(text)) => !matches!(text.as_str(), "" | "0"),
        Some(_) => true,
    }
}

/// A message's string (or number, as text); `None` for anything else.
fn text_of(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(match number.as_f64() {
            Some(float) if float.fract() == 0.0 && float.abs() < 1e15 => format!("{float:.0}"),
            _ => number.to_string(),
        }),
        _ => None,
    }
}

fn scope_known(scope: &str) -> bool {
    matches!(scope, "uncommitted" | "branch" | "last")
}

/// A branch name a work tree may take.
fn branch_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
        && !name.contains("..")
}

/// Where this provider looks and whom it obeys.
struct Config {
    home: String,
    /// The project folders, those that exist.
    roots: Vec<String>,
    /// The mods that may write.
    writers: Vec<String>,
    /// The file what the app asked for is kept in.
    kept: PathBuf,
}

impl Config {
    fn of(host: &Handle) -> Self {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_default();
        let wanted = env::var("MOONFRAME_PROJECTS").map_or_else(
            |_| {
                [
                    "code",
                    "dev",
                    "src",
                    "Projects",
                    "projects",
                    "work",
                    ".herdr/worktrees",
                ]
                .map(|folder| format!("{home}/{folder}"))
                .to_vec()
            },
            |wanted| wanted.split(':').map(str::to_owned).collect(),
        );
        let roots = wanted
            .iter()
            .map(|root| expand(root, &home))
            .filter(|root| Path::new(root).is_dir())
            .collect();
        let settings = to_json(host.settings());
        let writers = env::var("MOONFRAME_PROJECTS_WRITERS").map_or_else(
            |_| {
                let listed = settings.get("writers").and_then(Value::as_array);
                let listed = listed.into_iter().flatten().filter_map(Value::as_str);
                listed.map(str::to_owned).collect()
            },
            |writers| {
                let writers = writers.split(',').map(str::trim);
                writers
                    .filter(|w| !w.is_empty())
                    .map(str::to_owned)
                    .collect()
            },
        );
        let kept = kept::file(&home, profile());
        Self {
            home,
            roots,
            writers,
            kept,
        }
    }
}

/// Whether `gh` is wanted, and when it was last asked.
#[derive(Default)]
struct ForgeAsk {
    want: bool,
    at: u64,
    path: String,
    busy: bool,
}

struct Projects {
    config: Config,
    summaries: Summaries,
    state: Kept,
    /// A composer's folder, independent of the inspector's open ones.
    completion: Option<String>,
    projects: Vec<Value>,
    last_scan: u64,
    last: Option<String>,
    action: Option<Value>,
    running: Option<String>,
    rescan: bool,
    forge: Option<Value>,
    ask: ForgeAsk,
    reader: Reader,
    finished: Finished,
}

impl Projects {
    fn new(config: Config) -> Self {
        let state = Kept::restore(&config.kept);
        Self {
            config,
            summaries: Summaries::default(),
            state,
            completion: None,
            projects: Vec::new(),
            last_scan: 0,
            last: None,
            action: None,
            running: None,
            rescan: false,
            forge: None,
            ask: ForgeAsk::default(),
            reader: Reader::default(),
            finished: Finished::default(),
        }
    }

    fn finish(&mut self, name: &str, id: Option<Value>, result: Value) {
        self.running = None;
        let mut action = match result {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        let ok = truthy(action.get("ok"));
        action.insert("ok".to_owned(), json!(ok));
        action.insert("action".to_owned(), json!(name));
        action.insert("id".to_owned(), id.unwrap_or_else(|| json!(name)));
        self.action = Some(Value::Object(action));
        // Look again: a commit or a work tree changes what shows.
        self.rescan = true;
    }

    /// Picks up what finished; whether anything did.
    fn collect(&mut self) -> bool {
        let finished = self.finished.take();
        let any = !finished.is_empty();
        for done in finished {
            match done {
                Done::Write { name, id, result } => self.finish(&name, id, result),
                Done::Forge(result) => {
                    self.ask.busy = false;
                    self.forge = Some(result);
                },
            }
        }
        any
    }

    /// Runs `work` as the write `name`, if `from` may and none is under way.
    fn write(
        &mut self,
        from: &str,
        name: &str,
        id: Option<Value>,
        work: impl FnOnce() -> Value + Send + 'static,
    ) {
        if !self.config.writers.iter().any(|writer| writer == from) {
            let error = format!("`{from}` may not write; see MOONFRAME_PROJECTS_WRITERS");
            self.finish(name, id, json!({ "ok": false, "error": error }));
        } else if let Some(running) = &self.running {
            let error = format!("{running} is still running");
            self.finish(name, id, json!({ "ok": false, "error": error }));
        } else {
            self.running = Some(name.to_owned());
            let name = name.to_owned();
            self.finished.start(move || Done::Write {
                result: work(),
                name,
                id,
            });
        }
    }

    /// The open project, when it is one.
    fn open_tree(&self) -> Option<String> {
        let dir = self.state.select.clone()?;
        Path::new(&dir).join(".git").exists().then_some(dir)
    }

    fn write_or_refuse(
        &mut self,
        from: &str,
        name: &str,
        id: Option<Value>,
        work: impl FnOnce(&str) -> Value + Send + 'static,
    ) {
        match self.open_tree() {
            Some(dir) => self.write(from, name, id, move || work(&dir)),
            None => self.finish(
                name,
                id,
                json!({ "ok": false, "error": "No work tree open" }),
            ),
        }
    }

    fn receive(&mut self, from: &str, msg: &Value) {
        if !msg.is_object() {
            return;
        }
        if let Some(path) = text_of(msg.get("select")) {
            let path = expand(&path, &self.config.home);
            if self.state.select.as_ref() != Some(&path) {
                self.state.select = Some(path);
                self.completion = None;
                self.state.open.clear();
                self.state.read = None;
            }
        }
        if let Some(scope) = text_of(msg.get("scope")).filter(|scope| scope_known(scope)) {
            self.state.scope = scope;
        }
        if let Some(open) = msg
            .get("open")
            .filter(|open| !open.is_array() && !open.is_object() && !open.is_boolean())
        {
            let open = text_of(Some(open)).unwrap_or_default();
            let open = open.split('\n').filter(|path| !path.is_empty());
            self.state.open = open.map(|path| json!(path)).collect();
        }
        if msg.get("complete").is_some() && text_of(msg.get("cwd")) == self.state.select {
            self.completion = text_of(msg.get("complete"));
        }
        if msg.get("read").is_some() {
            self.state.read = text_of(msg.get("read"));
        }
        if msg.get("forge").is_some() {
            self.ask.want = truthy(msg.get("forge"));
            if truthy(msg.get("refresh")) {
                self.ask.at = 0;
            }
        }
        let id = msg
            .get("id")
            .filter(|id| text_of(Some(id)).is_some())
            .cloned();
        if let Some(message) = text_of(msg.get("commit")) {
            if self.open_tree().is_some() && message.trim().is_empty() {
                let error = "Write a message to commit with";
                self.finish("commit", id.clone(), json!({ "ok": false, "error": error }));
            } else {
                self.write_or_refuse(from, "commit", id.clone(), move |dir| {
                    jobs::commit(dir, &message)
                });
            }
        }
        for what in ["pull", "push"] {
            if truthy(msg.get(what)) {
                self.write_or_refuse(from, what, id.clone(), move |dir| jobs::sync(dir, what));
            }
        }
        if let Some(name) = text_of(msg.get("worktree")) {
            let repo = text_of(msg.get("repo")).unwrap_or_default();
            let repo = expand(&repo, &self.config.home);
            if branch_name(&name) && Path::new(&repo).join(".git").exists() {
                let home = self.config.home.clone();
                self.write(from, "worktree", id, move || {
                    jobs::add_worktree(&repo, &name, &home)
                });
            } else {
                let error = "A branch name (letters, digits, . _ / -) in a repository";
                self.finish("worktree", id, json!({ "ok": false, "error": error }));
            }
        }
        self.state.keep(&self.config.kept);
    }

    fn ask_forge(&mut self, dir: &str, now: u64) {
        let recent = self.ask.path == dir && now.saturating_sub(self.ask.at) < FORGE_EVERY;
        if !self.ask.want || self.ask.busy || recent {
            return;
        }
        self.ask.path = dir.to_owned();
        self.ask.at = now;
        self.ask.busy = true;
        let dir = dir.to_owned();
        self.finished
            .start(move || Done::Forge(jobs::ask_forge(&dir)));
    }

    fn scan(&mut self, now: u64) {
        self.rescan = false;
        let found = scan::find_projects(&self.config.roots);
        let mut projects = self.summaries.of(&found, &self.config.home);
        projects.sort_by_key(|project| std::cmp::Reverse(project["time"].as_u64().unwrap_or(0)));
        projects.truncate(MAX_PROJECTS);
        self.projects = projects;
        self.last_scan = now;
        // Until an app picks one, the most recently active project is open.
        if self.state.select.is_none() {
            let first = self
                .projects
                .first()
                .and_then(|project| project["path"].as_str());
            self.state.select = first.map(str::to_owned);
        }
    }

    fn current(&mut self, out: &mut Map<String, Value>, now: u64) {
        let Some(dir) = self.open_tree().filter(|dir| Path::new(dir).is_dir()) else {
            return;
        };
        let parent = dir.rsplit_once('/').map_or("", |(parent, _)| parent);
        let this = [Found {
            path: dir.clone(),
            group: parent.to_owned(),
        }];
        let rows = self.summaries.of(&this, &self.config.home);
        let info = rows.first().cloned().unwrap_or(Value::Null);
        let mut current = open::changes(&dir, &self.state.scope);
        current.insert("path".to_owned(), json!(dir));
        for key in ["name", "home", "branch"] {
            current.insert(key.to_owned(), info[key].clone());
        }
        current.insert("scope".to_owned(), json!(self.state.scope));
        current.insert("log".to_owned(), json!(open::commit_log(&dir)));
        out.insert("current".to_owned(), Value::Object(current));
        self.ask_forge(&dir, now);
        if let Some(forge) = self
            .forge
            .as_ref()
            .filter(|forge| forge["path"] == dir.as_str())
        {
            out.insert("forge".to_owned(), forge.clone());
        }
        let mut wanted = vec![String::new()];
        wanted.extend(self.completion.clone());
        wanted.extend(
            self.state
                .open
                .iter()
                .filter_map(|path| text_of(Some(path))),
        );
        out.insert(
            "dirs".to_owned(),
            Value::Object(open::listings(&dir, &wanted)),
        );
        if let Some(read) = self.state.read.clone().filter(|read| open::inside(read)) {
            out.insert("file".to_owned(), self.reader.read(&dir, &read));
        }
    }

    /// What to publish, when it changed. `force` looks for projects now.
    fn emit(&mut self, force: bool) -> Option<Value> {
        let now = scan::now();
        if force || self.rescan || now.saturating_sub(self.last_scan) >= SCAN_EVERY {
            self.scan(now);
        }
        let home = self.config.home.clone();
        let roots: Vec<String> = self.config.roots.iter().map(|r| tilde(r, &home)).collect();
        // To the minute: ages show no finer, and a clock that ticks every
        // look would publish, and redraw, with nothing else new.
        let out = json!({ "available": true, "now": now - now % 60, "roots": roots,
            "projects": self.projects });
        let mut out = match out {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        self.current(&mut out, now);
        if let Some(action) = &self.action {
            out.insert("action".to_owned(), action.clone());
        }
        if let Some(running) = &self.running {
            out.insert("running".to_owned(), json!(running));
        }
        let out = Value::Object(out);
        let text = out.to_string();
        if self.last.as_ref() == Some(&text) {
            return None;
        }
        self.last = Some(text);
        Some(out)
    }
}

#[derive(Default)]
struct Provider {
    projects: Option<Projects>,
    looked: Option<Instant>,
}

impl Provider {
    fn emit(&mut self, host: &Handle, force: bool) {
        self.looked = Some(Instant::now());
        let published = self
            .projects
            .as_mut()
            .and_then(|projects| projects.emit(force));
        if let Some(value) = published {
            host.publish_json(&value);
        }
    }
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        self.projects = Some(Projects::new(Config::of(host)));
        self.emit(host, true);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        let finished = self.projects.as_mut().is_some_and(Projects::collect);
        let due = self
            .looked
            .is_none_or(|looked| looked.elapsed() >= LOOK_EVERY);
        if finished || due {
            self.emit(host, finished);
        }
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        if let Some(projects) = &mut self.projects {
            projects.receive(from, &to_json(&message));
        }
        self.emit(host, true);
    }
}

#[cfg(test)]
mod tests;
