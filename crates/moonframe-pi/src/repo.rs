//! `app.repo`: the checkout a mod points it at with `{ at = path }`.
//!
//! ```text
//! { at, git, branch, head, base, base_name, files, more_files, compare,
//!   picked, diff, tree, open, file, found, worktrees, busy, error, message }
//! ```
//!
//! Messages: `{ at }`, `{ compare = "uncommitted"|"branch"|"range", from?,
//! to?, pick? }`, `{ pick = path }` (the diff shown), `{ stage = path }`,
//! `{ unstage = path }`, `{ stage_all = true }`, `{ discard = path }`,
//! `{ commit = message }`, `{ open_dir = path }`, `{ read = path }`,
//! `{ write = path, text, version }`, `{ close = true }`, `{ find = query,
//! root? }`, `{ remove_worktree = path }`, `{ refresh = true }`.
//!
//! Git can wait a long time (a signing key waiting for a tap), so
//! `Native::receive` only posts: one thread takes messages in order and
//! looks at the checkout every couple of seconds, publishing as it goes.

// keep-sorted start
mod diff;
mod files;
mod find;
mod status;
mod view;
// keep-sorted end

use {
    crate::{
        Config,
        core::guarded,
        git::{git as run, run_git},
        runtime::Runtime,
    },
    files::Listing,
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, to_json},
    serde_json::{Map, Value, json},
    std::{
        sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
        thread,
        time::{Duration, Instant},
    },
};

const POLL: Duration = Duration::from_secs(2);
const MAX_FILES: usize = 300;

pub(crate) fn native() -> Box<dyn Native> {
    Box::new(Provider::default())
}

#[derive(Default)]
struct Provider {
    inbox: Option<Sender<Value>>,
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let (inbox, messages) = mpsc::channel();
        let host = host.clone();
        thread::spawn(move || serve(&host, &messages));
        self.inbox = Some(inbox);
        Ok(())
    }

    /// The worker keeps its own time.
    fn tick(&mut self, _host: &Handle) {}

    fn receive(&mut self, _host: &Handle, _from: &str, message: Data) {
        if let Some(inbox) = &self.inbox {
            let _ = inbox.send(to_json(&message));
        }
    }
}

fn serve(host: &Handle, messages: &Receiver<Value>) {
    let mut repo = Repo::new(host.clone());
    repo.publish();
    let mut looked = Instant::now();
    loop {
        match messages.recv_timeout(POLL.saturating_sub(looked.elapsed())) {
            Ok(message) => {
                let handled = guarded(|| {
                    repo.handle(&message);
                    Ok(())
                });
                if let Err(problem) = handled {
                    tracing::error!("repo: {problem}");
                }
            },
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if looked.elapsed() >= POLL {
            looked = Instant::now();
            if let Err(problem) = guarded(|| {
                repo.refresh();
                Ok(())
            }) {
                tracing::error!("repo: {problem}");
            }
        }
    }
}

/// What is compared.
#[derive(Clone, Debug)]
struct Compare {
    mode: String,
    from: Option<String>,
    to: Option<String>,
}

impl Compare {
    fn uncommitted() -> Self {
        Self {
            mode: "uncommitted".to_owned(),
            from: None,
            to: None,
        }
    }
}

/// The state behind `app.repo`.
struct Repo {
    host: Handle,
    runtime: Runtime,
    at: Option<String>,
    is_git: bool,
    branch: Option<String>,
    head: Option<String>,
    base: Option<String>,
    base_name: Option<String>,
    files: Vec<Value>,
    more_files: usize,
    compare: Compare,
    picked: Option<String>,
    diff: Value,
    open: Vec<String>,
    tree: Map<String, Value>,
    file: Value,
    found: Value,
    worktrees: Vec<Value>,
    busy: Option<String>,
    error: Option<String>,
    message: Option<String>,
    listing: Option<Listing>,
    /// A refresh clears its own failures, not those of what the user did.
    refresh_failed: bool,
    last: String,
}

fn nothing_found() -> Value {
    json!({ "query": "", "results": [] })
}

/// A message's text, if it has any: an empty one asks for nothing.
fn filled<'a>(message: &'a Value, key: &str) -> Option<&'a str> {
    message[key].as_str().filter(|text| !text.is_empty())
}

impl Repo {
    fn new(host: Handle) -> Self {
        Self {
            host,
            runtime: Runtime::new(&Config::from_env()),
            at: None,
            is_git: false,
            branch: None,
            head: None,
            base: None,
            base_name: None,
            files: Vec::new(),
            more_files: 0,
            compare: Compare::uncommitted(),
            picked: None,
            diff: Value::Null,
            open: vec![String::new()],
            tree: Map::new(),
            file: Value::Null,
            found: nothing_found(),
            worktrees: Vec::new(),
            busy: None,
            error: None,
            message: None,
            listing: None,
            refresh_failed: false,
            last: String::new(),
        }
    }

    fn cwd(&self) -> &str {
        self.at.as_deref().unwrap_or(".")
    }

    fn git(&self, args: &[&str]) -> Result<String, String> {
        run(&self.runtime, self.cwd(), args)
    }

    fn publish(&mut self) {
        let (line, value) = view::encoded(self.view());
        if line != self.last {
            self.host.publish_json(&value);
            self.last = line;
        }
    }

    fn read_tree(&mut self) {
        let root = self.cwd().to_owned();
        self.tree = self
            .open
            .iter()
            .map(|path| (path.clone(), Value::Array(files::list_dir(&root, path))))
            .collect();
    }

    fn look(&mut self) -> Result<(), String> {
        self.is_git = self.git(&["rev-parse", "--is-inside-work-tree"]).is_ok();
        self.read_tree();
        if !self.is_git {
            self.files.clear();
            self.diff = Value::Null;
            self.branch = None;
            self.head = None;
            self.worktrees.clear();
            return Ok(());
        }
        let trimmed = |text: Result<String, String>| {
            Some(text.unwrap_or_default().trim().to_owned()).filter(|text| !text.is_empty())
        };
        self.branch = trimmed(self.git(&["symbolic-ref", "--short", "-q", "HEAD"]));
        self.head = trimmed(self.git(&["rev-parse", "--short", "HEAD"]));
        let base = self.base_branch();
        self.base = base.as_ref().map(|(_, sha)| sha.clone());
        self.base_name = base.map(|(name, _)| name);
        let mut files = self.read_status()?;
        self.more_files = files.len().saturating_sub(MAX_FILES);
        files.truncate(MAX_FILES);
        self.files = files;
        let listed = |path: &String| self.files.iter().any(|file| file["path"] == path.as_str());
        if !self.picked.as_ref().is_some_and(listed) {
            self.picked = self
                .files
                .first()
                .and_then(|file| file["path"].as_str().map(str::to_owned));
        }
        self.diff = match self.picked.clone() {
            Some(path) => self.read_diff(&path)?,
            None => Value::Null,
        };
        self.worktrees = self.read_worktrees()?;
        Ok(())
    }

    fn refresh(&mut self) {
        if self.at.is_none() {
            return;
        }
        match self.look() {
            Ok(()) => {
                if self.refresh_failed {
                    self.error = None;
                }
                self.refresh_failed = false;
            },
            Err(problem) => {
                self.error = Some(problem);
                self.refresh_failed = true;
            },
        }
        self.publish();
    }

    fn act(
        &mut self,
        label: &str,
        done: Option<&str>,
        action: impl FnOnce(&Self) -> Result<(), String>,
    ) {
        self.busy = Some(label.to_owned());
        self.error = None;
        self.publish();
        match action(self) {
            Ok(()) => {
                if let Some(done) = done {
                    self.message = Some(done.to_owned());
                }
            },
            Err(problem) => self.error = Some(problem),
        }
        self.busy = None;
        self.refresh();
    }

    fn commit(&self, text: &str) -> Result<(), String> {
        // A commit takes everything staged, and this list only shows the
        // project: what's staged beyond it must not go in unseen.
        let prefix = self.git(&["rev-parse", "--show-prefix"])?;
        let prefix = prefix.trim();
        let staged = self.git(&["diff", "--cached", "--name-only", "-z"])?;
        let staged: Vec<&str> = staged.split('\0').filter(|path| !path.is_empty()).collect();
        let outside: Vec<&str> = staged
            .iter()
            .copied()
            .filter(|path| !path.starts_with(prefix))
            .collect();
        if !outside.is_empty() {
            let named = outside[..outside.len().min(3)].join(", ");
            return Err(format!(
                "Staged outside this project: {named}. Unstage it first."
            ));
        }
        if staged.is_empty() {
            self.git(&["add", "-A", "--", "."])?;
        }
        let args = ["commit", "-q", "-F", "-"];
        run_git(&self.runtime, self.cwd(), &args, &[], Some(text))
            .output()
            .map(|_| ())
    }

    fn go_to(&mut self, at: String) {
        if self.at.as_deref() == Some(at.as_str()) {
            return;
        }
        self.at = Some(at);
        self.files.clear();
        self.diff = Value::Null;
        self.picked = None;
        self.file = Value::Null;
        self.open = vec![String::new()];
        self.compare = Compare::uncommitted();
        self.found = nothing_found();
        self.message = None;
        self.error = None;
        self.publish();
        self.refresh();
    }

    fn handle(&mut self, message: &Value) {
        if !message.is_object() {
            return;
        }
        if let Some(at) = message.get("at") {
            self.go_to(at.as_str().unwrap_or_default().to_owned());
            return;
        }
        // A draft searches the project it was given, whatever is open here.
        if let Some(query) = message.get("find") {
            let root = message["root"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| self.at.clone());
            if let Some(root) = root {
                self.find(query.as_str().unwrap_or_default(), &root);
                self.publish();
            }
            return;
        }
        if self.at.is_none() {
            return;
        }
        if let Some(mode) = filled(message, "compare") {
            let text = |key: &str| message[key].as_str().map(str::to_owned);
            self.compare = Compare {
                mode: mode.to_owned(),
                from: text("from"),
                to: text("to"),
            };
            self.picked = text("pick");
            self.refresh();
        } else if let Some(path) = message.get("pick") {
            self.pick(path);
        } else if let Some(path) = filled(message, "stage") {
            self.act("stage", None, |repo| {
                repo.git(&["add", "--", path]).map(|_| ())
            });
        } else if let Some(path) = filled(message, "unstage") {
            self.act("unstage", None, |repo| {
                repo.git(&["reset", "-q", "HEAD", "--", path]).map(|_| ())
            });
        } else if message["stage_all"].as_bool().unwrap_or(false) {
            self.act("stage", None, |repo| {
                repo.git(&["add", "-A", "--", "."]).map(|_| ())
            });
        } else if let Some(path) = filled(message, "discard") {
            let untracked = self
                .files
                .iter()
                .any(|file| file["path"] == path && file["status"] == "?");
            self.act("discard", None, |repo| {
                let args: &[&str] = if untracked {
                    &["clean", "-fq", "--", path]
                } else {
                    &["checkout", "HEAD", "--", path]
                };
                repo.git(args).map(|_| ())
            });
        } else if let Some(text) = filled(message, "commit") {
            self.act("commit", Some("Committed"), |repo| repo.commit(text));
        } else if let Some(path) = message.get("open_dir") {
            let path = path.as_str().unwrap_or_default().to_owned();
            match self.open.iter().position(|open| *open == path) {
                Some(at) => {
                    self.open.remove(at);
                },
                None => self.open.push(path),
            }
            self.read_tree();
            self.publish();
        } else if let Some(path) = filled(message, "read") {
            self.file = files::open_file(self.cwd(), path).unwrap_or_else(
                |reason| json!({ "path": path, "missing": true, "reason": reason }),
            );
            self.publish();
        } else if let Some(path) = filled(message, "write") {
            let text = message["text"].as_str().unwrap_or_default();
            match files::save_file(self.cwd(), path, text, message["version"].as_str()) {
                Ok((file, saved)) => {
                    self.file = file;
                    if saved {
                        self.message = Some(format!("Saved {path}"));
                    }
                },
                Err(problem) => self.error = Some(problem),
            }
            self.publish();
        } else if message["close"].as_bool().unwrap_or(false) {
            self.file = Value::Null;
            self.publish();
        } else if let Some(path) = filled(message, "remove_worktree") {
            self.act("worktree", None, |repo| {
                repo.git(&["worktree", "remove", "--force", path])
                    .map(|_| ())
            });
        } else {
            self.refresh();
        }
    }

    fn pick(&mut self, path: &Value) {
        self.picked = path.as_str().map(str::to_owned);
        self.diff = match path.as_str() {
            Some(path) => self.read_diff(path).unwrap_or(Value::Null),
            None => Value::Null,
        };
        self.publish();
    }
}
