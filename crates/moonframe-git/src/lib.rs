//! The `git` native provider: the git state of the app's repository as
//! `app.git`, and the writes a mod asks for (commit, push, worktrees), each
//! answered to the mod that asked.
//!
//! The repository is `LAYOUTS_REPO` when set; else the project the user
//! works in (the repository of the terminal they last used), followed as
//! it changes; else the one holding the app.
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
//! - `{ commit = "<message>", root, id = ... }` stages everything and
//!   commits;
//! - `{ push = true, root, id = ... }` pushes the branch to `origin`;
//! - `{ worktree_add = { branch, base? }, root, id = ... }` adds a worktree;
//! - `{ worktree_remove = "<path>", root, id = ... }` removes one;
//! - `{ open = "<folder>", id = ... }` watches the repository holding that
//!   folder from now on (see [`open`]).
//!
//! A write names the repository it is for (`root`, as `app.git.root` gave
//! it) and is refused when the provider no longer watches that one: the
//! repository follows the terminal's project, so a sheet drawn for one must
//! never commit to another.
//!
//! Only the mods named in provider.toml's `writers` setting may write; any
//! other gets a refusal as its answer. Reading is open to all.
//!
//! An action answers its mod with `{ reply = id, action, ok, message }`,
//! or `error` instead of `message` when it failed; `reply` is the action's
//! name when the mod gave no `id`.

// keep-sorted start
mod diff;
mod log;
mod open;
mod refs;
mod status;
mod worktrees;
// keep-sorted end

/// A remote URL's host, owner and repository, as `app.git.remote` gives
/// them; the forge providers read their repository with it.
pub use refs::remote_json;

use {
    herdr_github::pull_request::Input,
    herdr_vcs::git::{self as vcs, Action},
    moonframe_core::Data,
    moonframe_providers::{Handle, Mods, Native},
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
    worktrees::Change,
};

/// How long a read may take; writes get herdr-vcs's own, longer deadline.
const READ_TIMEOUT: Duration = Duration::from_secs(15);
/// Output kept from one read command.
const READ_LIMIT: usize = 8 << 20;
/// Ticks (of provider.toml's `interval`) between looks at the status.
const LOOK_EVERY: u32 = 5;
/// Looks between reads of worktrees, branches, remote and stashes.
const SLOW_EVERY: u32 = 6;
/// The least time between two messages each reading everything.
const MIN_FORCED: Duration = Duration::from_millis(500);

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
    /// every other read, and set up as herdr-vcs sets up its own git.
    pub(crate) fn text_any_status(&self, args: &[&str]) -> Option<String> {
        let mut command = std::process::Command::new("git");
        // No console window of its own on Windows, as herdr-vcs's commands.
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
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
    /// The project it last moved to ([`moonframe_providers::project`]).
    followed: Option<PathBuf>,
    /// The mods that may commit and push (`[settings] writers`).
    writers: Mods,
    /// When a message last made it read everything; one coming sooner
    /// reads the status alone.
    forced_at: Option<Instant>,
    /// Status and numstat bytes last read: a look that finds them the same
    /// publishes nothing.
    last_look: Vec<u8>,
    status: Status,
    head: Value,
    worktrees: Vec<refs::Worktree>,
    /// How many commits a mod asked to see, and those read.
    log_limit: Option<usize>,
    log: Vec<Value>,
    /// How many times the log was read.
    #[cfg(test)]
    log_reads: usize,
    branches: Vec<Value>,
    remote: Value,
    stashes: u64,
    /// The file whose diff is kept in `diff`.
    diff_path: Option<String>,
    ticks: u32,
    looks: u32,
    /// The status moved while messages came too fast to read worktrees,
    /// branches, remote and stashes: the next tick reads them.
    refs_stale: bool,
    /// How often they were read, for the tests.
    #[cfg(test)]
    ref_reads: u32,
    last: Option<Value>,
    /// An action is running: one at a time.
    busy: Arc<AtomicBool>,
    /// That action removes a worktree: [`Git::follow`] waits for it.
    removing: Arc<AtomicBool>,
    /// An action finished: look again at the next tick.
    stale: Arc<AtomicBool>,
}

impl Native for Git {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        self.root = repository(host.dir());
        self.followed = moonframe_providers::project();
        self.writers = Mods::from_settings(host.settings(), "writers");
        self.refresh(host, true);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        if self.follow(moonframe_providers::project()) {
            self.refresh(host, true);
            return;
        }
        self.ticks += 1;
        let stale = self.stale.swap(false, Ordering::AcqRel);
        if stale || self.ticks >= LOOK_EVERY {
            self.ticks = 0;
            self.refresh(host, stale);
        }
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        // A move first: a write asked for the old repository is refused
        // below, with an answer, never run in the new one.
        if self.follow(moonframe_providers::project()) {
            self.refresh(host, true);
        }
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
        match field("log") {
            Some(Data::Integer(count)) => {
                let limit = usize::try_from(*count)
                    .unwrap_or(log::MAX_COMMITS)
                    .clamp(1, log::MAX_COMMITS);
                // Read at once only for a new request: a mod sending this
                // each frame must not get past the pace of `look`.
                if self.log_limit != Some(limit) {
                    self.log_limit = Some(limit);
                    self.read_log();
                }
            },
            Some(Data::Bool(false)) => {
                self.log_limit = None;
                self.log.clear();
            },
            _ => {},
        }
        if let Some(Data::String(folder)) = field("open") {
            self.open(host, from, field("id"), folder);
        }
        let root = match field("root") {
            Some(Data::String(root)) => Some(root.as_str()),
            _ => None,
        };
        if let Some(Data::String(text)) = field("commit") {
            self.act(
                host,
                from,
                field("id"),
                root,
                Job::Vcs(Action::Commit(text.clone())),
            );
        } else if let Some(Data::Bool(true)) = field("push") {
            self.act(host, from, field("id"), root, Job::Vcs(Action::Push));
        } else if let Some(asked) = field("worktree_add") {
            let job = Change::add(asked).map(Job::Worktree);
            self.act_or_refuse(host, from, field("id"), root, job, "worktree_add");
        } else if let Some(Data::String(path)) = field("worktree_remove") {
            let path = path.clone();
            self.act(
                host,
                from,
                field("id"),
                root,
                Job::Worktree(Change::Remove { path }),
            );
        }
        // Worktrees, branches, remote and stashes are some 60 git commands
        // with many worktrees: a mod sending a message each frame gets them
        // read once, and the status alone after that.
        let now = Instant::now();
        let read_refs = self
            .forced_at
            .is_none_or(|at| now.duration_since(at) >= MIN_FORCED);
        if read_refs {
            self.forced_at = Some(now);
        }
        self.look(host, true, read_refs, true);
    }
}

impl Git {
    /// Reads the commits a mod asked for; nothing if none did.
    fn read_log(&mut self) {
        if let (Some(limit), Some(root)) = (self.log_limit, &self.root) {
            self.log = log::commits(&Repo::at(root), limit);
            #[cfg(test)]
            {
                self.log_reads += 1;
            }
        }
    }

    /// Moves to `project`, the project the user works in now, unless
    /// `LAYOUTS_REPO` names the repository; returns whether it moved.
    /// Everything read of the old one goes; an action running there
    /// finishes there.
    ///
    /// A worktree being removed may be the very project moved to: the move
    /// waits, `followed` left as it was so the next look tries again, and it
    /// is never made to a checkout that is gone by then.
    fn follow(&mut self, project: Option<PathBuf>) -> bool {
        if std::env::var_os("LAYOUTS_REPO").is_some() {
            return false;
        }
        if self.removing.load(Ordering::Acquire) {
            return false;
        }
        if project.is_none() || project == self.followed {
            return false;
        }
        self.followed = project.clone();
        let Some(root) = project.as_deref().and_then(toplevel) else {
            return false;
        };
        if self.root.as_ref() == Some(&root) {
            return false;
        }
        self.move_to(root, project);
        true
    }

    /// Watches `root` from now on, `followed` being the project last moved
    /// to. Everything read of the old one goes; an action running there
    /// finishes there.
    fn move_to(&mut self, root: PathBuf, followed: Option<PathBuf>) {
        *self = Self {
            root: Some(root),
            followed,
            writers: std::mem::take(&mut self.writers),
            log_limit: self.log_limit,
            busy: self.busy.clone(),
            removing: self.removing.clone(),
            stale: self.stale.clone(),
            ..Self::default()
        };
    }

    /// Opens the repository holding `folder` for the mod `from` and answers
    /// it with that repository's root, or why not. The project followed
    /// stays as it was, so the next look does not move straight back.
    fn open(&mut self, host: &Handle, from: &str, id: Option<&Data>, folder: &str) {
        let answer = answerer(host, from, id, "open");
        if !self.writers.allows(from) {
            answer(Err(format!(
                "`{from}` may not open a repository: only the mods in providers/git's `writers` may"
            )));
            return;
        }
        // An action running in the old repository finishes first.
        if self.busy.load(Ordering::Acquire) {
            answer(Err(herdr_vcs::git::Error::Busy.to_string()));
            return;
        }
        match open::repository(folder) {
            Ok(root) => {
                if self.root.as_ref() != Some(&root) {
                    let followed = self.followed.take();
                    self.move_to(root.clone(), followed);
                }
                answer(Ok(root.to_string_lossy().into_owned()));
            },
            Err(problem) => answer(Err(problem)),
        }
    }

    /// Looks at the repository and publishes what changed; `force` reads
    /// everything and publishes even when nothing did.
    fn refresh(&mut self, host: &Handle, force: bool) {
        self.look(host, force, force, false);
    }

    /// [`Git::refresh`], reading worktrees, branches, remote and stashes on
    /// `force` only when `read_refs` is set too. A look a message asked for
    /// (`paced`) reads them only when `read_refs` allows it, whatever moved:
    /// the next tick catches up.
    fn look(&mut self, host: &Handle, force: bool, read_refs: bool, paced: bool) {
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
        let looked_new = look != self.last_look;
        if changed || looked_new {
            changed = true;
            self.status = Status::parse(&state, &Numstat::parse(&numstat));
            self.last_look = look;
            if self.status.oid != old_oid || self.head.is_null() {
                self.head = refs::head(&repo);
            }
        }
        let moved = looked_new || self.status.oid != old_oid || self.status.branch != old_branch;
        // Only ticks count toward the slow read: a message each frame would
        // otherwise reach it every few frames.
        if !paced {
            self.looks += 1;
        }
        let due = (read_refs && changed)
            || moved
            || self.refs_stale
            || self.last.is_none()
            || self.looks >= SLOW_EVERY;
        let allowed = !paced || read_refs || self.last.is_none();
        if due && !allowed {
            self.refs_stale |= moved;
        } else if due {
            self.looks = 0;
            self.refs_stale = false;
            #[cfg(test)]
            {
                self.ref_reads += 1;
            }
            changed = true;
            self.worktrees = refs::worktrees(&repo);
            self.branches = refs::branches(&repo);
            self.remote = refs::remote(&repo, &self.status.branch);
            self.stashes = refs::stashes(&repo);
            self.read_log();
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
        // Last, in the room the rest leaves: the host refuses a publish past
        // its limit whole, and a diff is what the user is looking at.
        if self.log_limit.is_some() {
            let log = log::fit(
                &self.log,
                moonframe_providers::MAX_ENTRIES.saturating_sub(log::values(&out)),
            );
            if let Value::Object(fields) = &mut out {
                fields.insert("log".to_owned(), log);
            }
        }
        self.publish(host, out, force);
    }

    fn publish(&mut self, host: &Handle, out: Value, force: bool) {
        if force || self.last.as_ref() != Some(&out) {
            host.publish_json(&out);
            self.last = Some(out);
        }
    }

    /// [`act`](Self::act), or the reason `job` could not even be read.
    fn act_or_refuse(
        &self,
        host: &Handle,
        from: &str,
        id: Option<&Data>,
        asked: Option<&str>,
        job: Result<Job, String>,
        name: &'static str,
    ) {
        match job {
            Ok(job) => self.act(host, from, id, asked, job),
            Err(problem) => answerer(host, from, id, name)(Err(problem)),
        }
    }

    /// Runs `job` on a thread of its own and answers `from` when it ends,
    /// if `asked` names the repository it watches.
    fn act(&self, host: &Handle, from: &str, id: Option<&Data>, asked: Option<&str>, job: Job) {
        let name = job.name();
        let answer = answerer(host, from, id, name);
        // A write changes the user's history and remote; a mod from
        // elsewhere must not do it unasked.
        if !self.writers.allows(from) {
            answer(Err(format!(
                "`{from}` may not {name}: only the mods in providers/git's `writers` may"
            )));
            return;
        }
        let Some(root) = self.root.clone() else {
            answer(Err("not a git repository".to_owned()));
            return;
        };
        if let Err(problem) = same_repository(asked, &root) {
            answer(Err(problem));
            return;
        }
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
        // Set here, on the thread `follow` runs on: no move can slip in
        // between the checks above and the removal starting.
        self.removing.store(job.removes(), Ordering::Release);
        let (busy, removing, stale) =
            (self.busy.clone(), self.removing.clone(), self.stale.clone());
        let spawned = thread::Builder::new()
            .name(format!("git-{name}"))
            .spawn(move || {
                answer(job.run(input, &root));
                removing.store(false, Ordering::Release);
                busy.store(false, Ordering::Release);
                stale.store(true, Ordering::Release);
            });
        if spawned.is_err() {
            self.removing.store(false, Ordering::Release);
            self.busy.store(false, Ordering::Release);
        }
    }
}

/// A write a mod asks for.
enum Job {
    // keep-sorted start
    Vcs(Action),
    Worktree(Change),
    // keep-sorted end
}

impl Job {
    fn name(&self) -> &'static str {
        match self {
            Self::Vcs(Action::Commit(_)) => "commit",
            Self::Vcs(Action::Push) => "push",
            Self::Vcs(Action::CreatePullRequest) => "pull_request",
            Self::Worktree(Change::Add { .. }) => "worktree_add",
            Self::Worktree(Change::Remove { .. }) => "worktree_remove",
        }
    }

    fn removes(&self) -> bool {
        matches!(self, Self::Worktree(Change::Remove { .. }))
    }

    fn run(self, input: Input, root: &Path) -> Result<String, String> {
        match self {
            Self::Vcs(action) => run(input, action),
            Self::Worktree(change) => change.apply(root, &in_use),
        }
    }
}

/// What answers `from` once a write named `name` ends: `id`, or the name
/// when it gave none, comes back as `reply`.
fn answerer(
    host: &Handle,
    from: &str,
    id: Option<&Data>,
    name: &'static str,
) -> impl FnOnce(Result<String, String>) + Send + use<> {
    let reply = id.cloned().unwrap_or_else(|| Data::from(name));
    let (host, from) = (host.clone(), from.to_owned());
    move |result: Result<String, String>| {
        let (key, text) = match &result {
            Ok(message) => ("message", message.clone()),
            Err(error) => ("error", error.clone()),
        };
        host.send(
            &from,
            Data::Map(BTreeMap::from([
                ("reply".to_owned(), reply),
                ("action".to_owned(), Data::from(name)),
                ("ok".to_owned(), Data::Bool(result.is_ok())),
                (key.to_owned(), Data::String(text)),
            ])),
        );
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

/// Whether `asked`, the repository a write is for, is `root`, the one
/// watched now.
fn same_repository(asked: Option<&str>, root: &Path) -> Result<(), String> {
    let Some(asked) = asked else {
        return Err("say which repository: `root`, as `app.git.root` gives it".to_owned());
    };
    if Path::new(asked) == root {
        return Ok(());
    }
    Err(format!(
        "the repository changed to {}; look at it before writing",
        root.display()
    ))
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

/// The repository to watch: `LAYOUTS_REPO`, else the project the user
/// works in, else the one the app was started in, else the one holding the
/// app folder this provider is in (two folders up), as git.sh did.
fn repository(dir: &Path) -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("LAYOUTS_REPO") {
        return toplevel(Path::new(&named));
    }
    moonframe_providers::project()
        .as_deref()
        .and_then(toplevel)
        // Before any terminal has made a project: an app that opens its
        // terminals from what git lists needs one to start from.
        .or_else(|| moonframe_providers::launch_dir().and_then(toplevel))
        .or_else(|| toplevel(&dir.join("..").join("..")))
}

/// The checkout the user works in now, as git names it.
fn in_use() -> Option<PathBuf> {
    moonframe_providers::project().as_deref().and_then(toplevel)
}

/// The repository holding `start`, as git names it.
fn toplevel(start: &Path) -> Option<PathBuf> {
    // The app's folder may be relative (`moonframe app`); git runs elsewhere.
    let start = std::path::absolute(start).unwrap_or_else(|_| start.to_owned());
    let top = Repo::at(&start).text(&["rev-parse", "--show-toplevel"], "find the repository")?;
    Some(PathBuf::from(top))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
