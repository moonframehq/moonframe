//! The `git` native provider: the git state of the app's repository as
//! `app.git`, and the writes a mod asks for (commit, push), each answered
//! to the mod that asked.
//!
//! It reads what `providers/git/git.sh` read, in the same shape (see the
//! moonframe skill's "Built-in providers"), and runs every git command
//! through herdr-vcs: no shell, no terminal prompts, bounded output, a
//! deadline. Commit and push are herdr-vcs's own, on a thread each, so a
//! push waiting on a hardware key never holds up the status.
//!
//! Messages:
//! - anything refreshes;
//! - `{ diff = "<path>" }` adds that file's diff as `diff`, kept fresh
//!   until `{ diff = false }`;
//! - `{ commit = "<message>", id = ... }` stages everything and commits;
//! - `{ push = true, id = ... }` pushes the branch to `origin`.
//!
//! Only the mods named in provider.toml's `writers` setting may commit or
//! push; any other gets a refusal as its answer. Reading is open to all.
//!
//! An action answers its mod with `{ reply = id, action, ok, message }`,
//! or `error` instead of `message` when it failed; `reply` is the action's
//! name when the mod gave no `id`.

mod diff;
mod refs;
mod status;

use {
    herdr_github::pull_request::Input,
    herdr_vcs::git::{self as vcs, Action},
    moonframe_core::Data,
    moonframe_providers::{Handle, Native},
    serde_json::{Value, json},
    status::{Numstat, Status},
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    },
};

/// How long a read may take; writes get herdr-vcs's own, longer deadline.
const READ_TIMEOUT: Duration = Duration::from_secs(15);
/// Output kept from one read command.
const READ_LIMIT: usize = 8 << 20;
/// Ticks (of provider.toml's `interval`) between looks at the status.
const LOOK_EVERY: u32 = 5;
/// Looks between reads of worktrees, branches, remote and stashes.
const SLOW_EVERY: u32 = 6;

/// Makes the provider, for the app's registry.
pub fn native() -> Box<dyn Native> {
    Box::new(Git::default())
}

/// Before every read. Git's stderr comes back mixed into what it prints,
/// and with `core.autocrlf` (Git for Windows' default) every diff warns
/// that LF will become CRLF, in front of the numbers it is read for.
const QUIET: &[&str] = &["-c", "core.safecrlf=false"];

/// A repository, where git runs.
pub(crate) struct Repo {
    root: PathBuf,
}

impl Repo {
    pub(crate) fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn bytes(&self, args: &[&str], operation: &'static str) -> Option<Vec<u8>> {
        let checkout = self.root.to_string_lossy();
        vcs::git_bytes(
            &checkout,
            &[QUIET, args].concat(),
            operation,
            Instant::now() + READ_TIMEOUT,
            &|| false,
            READ_LIMIT,
        )
        .ok()
    }

    /// What git printed, when it succeeded.
    pub(crate) fn text(&self, args: &[&str], operation: &'static str) -> Option<String> {
        self.bytes(args, operation).map(|bytes| {
            String::from_utf8_lossy(&bytes)
                .trim_end_matches(['\r', '\n'])
                .to_owned()
        })
    }

    /// What git printed, whatever its exit status (`diff --no-index`
    /// exits 1 when the files differ), bounded and with a deadline like
    /// every other read.
    pub(crate) fn text_any_status(
        &self,
        args: &[&str],
        _operation: &'static str,
    ) -> Option<String> {
        let mut command = std::process::Command::new("git");
        command
            .args(["-c", "core.fsmonitor=false", "--no-optional-locks", "-C"])
            .arg(&self.root)
            .args(QUIET)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0");
        let (_, bytes) = herdr_github::pull_request::run_bytes(
            &mut command,
            Instant::now() + READ_TIMEOUT,
            &|| false,
            READ_LIMIT,
        )
        .ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[derive(Default)]
pub struct Git {
    root: Option<PathBuf>,
    /// The mods that may commit and push (`[settings] writers`).
    writers: Vec<String>,
    /// Status and numstat bytes last read: a look that finds them the same
    /// publishes nothing.
    last_look: Vec<u8>,
    status: Status,
    head: Value,
    worktrees: Vec<refs::Worktree>,
    branches: Vec<Value>,
    remote: Value,
    stashes: u64,
    /// The file whose diff is kept in `diff`.
    diff_path: Option<String>,
    ticks: u32,
    looks: u32,
    last: Option<Value>,
    /// An action is running: one at a time.
    busy: Arc<AtomicBool>,
    /// An action finished: look again at the next tick.
    stale: Arc<AtomicBool>,
}

impl Native for Git {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        self.root = repository(host.dir());
        self.writers = writers(host.settings());
        self.refresh(host, true);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        self.ticks += 1;
        let stale = self.stale.swap(false, Ordering::AcqRel);
        if stale || self.ticks >= LOOK_EVERY {
            self.ticks = 0;
            self.refresh(host, stale);
        }
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let fields = match &message {
            Data::Map(fields) => Some(fields),
            _ => None,
        };
        let field = |key: &str| fields.and_then(|fields| fields.get(key));
        match field("diff") {
            Some(Data::String(path)) => self.diff_path = Some(path.clone()),
            Some(Data::Bool(false) | Data::Nil) => self.diff_path = None,
            _ => {},
        }
        if let Some(Data::String(text)) = field("commit") {
            self.act(host, from, field("id"), Action::Commit(text.clone()));
        } else if let Some(Data::Bool(true)) = field("push") {
            self.act(host, from, field("id"), Action::Push);
        }
        self.refresh(host, true);
    }
}

impl Git {
    /// Looks at the repository and publishes what changed; `force` reads
    /// everything and publishes even when nothing did.
    fn refresh(&mut self, host: &Handle, force: bool) {
        let Some(root) = self.root.clone() else {
            self.publish(
                host,
                json!({
                    "available": false, "repository": false, "reason": "not a git repository",
                }),
                force,
            );
            return;
        };
        let repo = Repo::at(&root);
        let Some(state) = repo.bytes(
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=normal",
            ],
            "read the working tree",
        ) else {
            // A busy index fails a look now and then; try again next time.
            return;
        };
        let base = base(&repo);
        let numstat = repo
            .bytes(&["diff", "--numstat", "-z", &base], "count changed lines")
            .unwrap_or_default();
        let look = [state.as_slice(), &numstat].concat();
        let (old_oid, old_branch) = (self.status.oid.clone(), self.status.branch.clone());
        let mut changed = force || self.last.is_none();
        if changed || look != self.last_look {
            changed = true;
            self.status = Status::parse(&state, &Numstat::parse(&numstat));
            self.last_look = look;
            if self.status.oid != old_oid || self.head.is_null() {
                self.head = refs::head(&repo);
            }
        }
        self.looks += 1;
        if changed
            || self.looks >= SLOW_EVERY
            || self.status.oid != old_oid
            || self.status.branch != old_branch
        {
            self.looks = 0;
            changed = true;
            self.worktrees = refs::worktrees(&repo);
            self.branches = refs::branches(&repo);
            self.remote = refs::remote(&repo, &self.status.branch);
            self.stashes = refs::stashes(&repo);
        }
        if !changed {
            return;
        }
        let root_text = root.to_string_lossy().into_owned();
        let status = &self.status;
        let current = (status.changed, status.ahead, status.behind);
        let worktrees: Vec<Value> = self
            .worktrees
            .iter()
            .map(|worktree| worktree.json((worktree.path == root_text).then_some(current)))
            .collect();
        let mut out = json!({
            "available": true, "repository": true,
            "branch": status.branch, "changed": status.changed,
            "ahead": status.ahead, "behind": status.behind,
            "root": root_text, "detached": status.detached, "head": self.head,
            "upstream": status.upstream, "added": status.added, "deleted": status.deleted,
            "files": status.files, "files_truncated": status.truncated,
            "worktrees": worktrees, "branches": self.branches,
            "remote": self.remote, "stashes": self.stashes,
        });
        if let (Some(path), Value::Object(fields)) = (&self.diff_path, &mut out) {
            // Only a file git reports as changed: a path a mod makes up
            // (`../..`, `/etc/hosts`) would otherwise publish a file outside
            // the repository to every view.
            let listed = status
                .files
                .iter()
                .any(|file| file["path"] == path.as_str());
            let diff = if listed {
                diff::read(&repo, &base, path)
            } else {
                diff::parse(path, "")
            };
            fields.insert("diff".to_owned(), diff);
        }
        self.publish(host, out, force);
    }

    fn publish(&mut self, host: &Handle, out: Value, force: bool) {
        if force || self.last.as_ref() != Some(&out) {
            host.publish_json(&out);
            self.last = Some(out);
        }
    }

    /// Runs `action` on a thread of its own and answers `from` when it ends.
    fn act(&self, host: &Handle, from: &str, id: Option<&Data>, action: Action) {
        let name = match action {
            Action::Commit(_) => "commit",
            Action::Push => "push",
            Action::CreatePullRequest => "pull_request",
        };
        let reply = id.cloned().unwrap_or_else(|| Data::from(name));
        let answer = {
            let (host, from, reply) = (host.clone(), from.to_owned(), reply.clone());
            move |result: Result<String, String>| {
                let (key, text) = match &result {
                    Ok(message) => ("message", message.clone()),
                    Err(error) => ("error", error.clone()),
                };
                host.send(
                    &from,
                    Data::Map(BTreeMap::from([
                        ("reply".to_owned(), reply.clone()),
                        ("action".to_owned(), Data::from(name)),
                        ("ok".to_owned(), Data::Bool(result.is_ok())),
                        (key.to_owned(), Data::String(text)),
                    ])),
                );
            }
        };
        // A write changes the user's history and remote; a mod from
        // elsewhere must not do it unasked.
        if !self.writers.iter().any(|writer| writer == from) {
            answer(Err(format!(
                "`{from}` may not {name}: only the mods in providers/git's `writers` may"
            )));
            return;
        }
        let Some(root) = self.root.clone() else {
            answer(Err("not a git repository".to_owned()));
            return;
        };
        if self.busy.swap(true, Ordering::AcqRel) {
            answer(Err(herdr_vcs::git::Error::Busy.to_string()));
            return;
        }
        // No `repo_key`: herdr-vcs reads the repository from the checkout
        // and checks it is still that repository on that branch.
        let input = Input {
            checkout: Some(root.to_string_lossy().into_owned()),
            repo_key: None,
            branch: self.status.branch.clone(),
        };
        let (busy, stale) = (self.busy.clone(), self.stale.clone());
        let spawned = thread::Builder::new()
            .name(format!("git-{name}"))
            .spawn(move || {
                answer(run(input, action));
                busy.store(false, Ordering::Release);
                stale.store(true, Ordering::Release);
            });
        if spawned.is_err() {
            self.busy.store(false, Ordering::Release);
        }
    }
}

/// Runs one write through herdr-vcs and waits for it.
fn run(input: Input, action: Action) -> Result<String, String> {
    let mut git = vcs::Git::default();
    git.track(Some(input), false, Instant::now());
    git.start(action, None).map_err(|error| error.to_string())?;
    loop {
        git.poll(Instant::now());
        if git.running().is_none() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    match (git.outcome(), git.error()) {
        (Some(outcome), _) => Ok(outcome.message.clone()),
        (None, Some(error)) => Err(error.to_owned()),
        (None, None) => Err("git stopped without saying why".to_owned()),
    }
}

/// The mods `[settings] writers` names; none when it is missing.
fn writers(settings: &Data) -> Vec<String> {
    let Data::Map(settings) = settings else {
        return Vec::new();
    };
    match settings.get("writers") {
        Some(Data::List(names)) => names
            .iter()
            .filter_map(|name| match name {
                Data::String(name) => Some(name.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// HEAD, or the empty tree before the first commit.
fn base(repo: &Repo) -> String {
    if repo
        .text(&["rev-parse", "-q", "--verify", "HEAD"], "find HEAD")
        .is_some()
    {
        "HEAD".to_owned()
    } else {
        repo.text(
            &["hash-object", "-t", "tree", "/dev/null"],
            "name the empty tree",
        )
        .unwrap_or_else(|| "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_owned())
    }
}

/// The repository to watch: `LAYOUTS_REPO`, else the one holding the app
/// folder this provider is in (two folders up), as git.sh did.
fn repository(dir: &Path) -> Option<PathBuf> {
    let start = std::env::var_os("LAYOUTS_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.join("..").join(".."));
    // The app's folder may be relative (`moonframe app`); git runs elsewhere.
    let start = std::path::absolute(&start).unwrap_or(start);
    let top = Repo::at(&start).text(&["rev-parse", "--show-toplevel"], "find the repository")?;
    Some(PathBuf::from(top))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
