//! Undo for an app's own folder: a snapshot each time its files change,
//! so any change (an agent's, an editor's, or a restore's) can be undone
//! and redone while the window reloads live.
//!
//! Snapshots are commits in a git repository of the history's own, kept
//! outside the folder (in the state folder), with the folder as its work
//! tree. The folder needs no repository and a repository it is in is
//! never touched: not its `HEAD`, index, refs, or hooks.
//!
//! - `refs/heads/history` is the newest snapshot; each one's parent is the
//!   one the folder was at when it was taken, so history is a line.
//! - `refs/moonframe/cursor` is the snapshot the folder is at now. Undo
//!   moves it to its parent and writes that snapshot's files; redo moves
//!   it back towards `history`. A change after an undo starts a new line
//!   from the cursor, as an editor's undo does; the reflog keeps the old
//!   one for a while.
//!
//! Undo, redo and restore first take a snapshot of whatever changed since
//! the last one, so nothing is lost, and refuse (as git does) when a file
//! changes under them halfway through.
//!
//! What a window keeps rather than what it is made of stays out: its
//! `settings.toml`, databases, installed packages (clones of their own),
//! other profiles, and build output.

mod git;
#[cfg(feature = "herdr")]
mod herdr;
#[cfg(test)]
mod tests;

use {
    git::Git,
    std::{
        fmt, fs,
        path::{Path, PathBuf},
        sync::Mutex,
    },
};

/// The newest snapshot.
const BRANCH: &str = "refs/heads/history";
/// The snapshot the folder is at.
const CURSOR: &str = "refs/moonframe/cursor";
/// What a snapshot leaves out, beside the folder's own `.gitignore`s.
const EXCLUDE: &str = "\
# Written by Moonframe each launch: what snapshots leave out.
.git/
.DS_Store
/settings.toml
/packages/
/profiles/
target/
*.sqlite
*.sqlite-*
*.db
*.db-*
";
/// Files a snapshot's label names before it counts the rest.
const NAMED: usize = 2;

/// Why a history operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// One snapshot of the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// The commit's id.
    pub id: String,
    /// Unix seconds.
    pub created: i64,
    /// The files it changed, as "mods/clock/init.luau and 2 more".
    pub label: String,
    /// Why it was taken: "changed", "launch", "before undo".
    pub reason: String,
    /// The folder is at this snapshot.
    pub current: bool,
}

impl Snapshot {
    /// The id's first characters, as git shortens it.
    pub fn short(&self) -> &str {
        self.id.get(..8).unwrap_or(&self.id)
    }
}

/// The history of one folder.
#[derive(Debug)]
pub struct History {
    git: Git,
    /// One operation at a time: they share the repository's index.
    busy: Mutex<()>,
}

impl History {
    /// The history of `folder`, kept in the repository `store`, made if it
    /// is not there yet.
    pub fn open(folder: &Path, store: &Path) -> Result<Self> {
        let work = folder.canonicalize()?;
        if !store.join("HEAD").is_file() {
            create(store, &work)?;
        }
        let info = store.join("info");
        fs::create_dir_all(&info)?;
        fs::write(info.join("exclude"), EXCLUDE)?;
        Ok(Self {
            git: Git {
                dir: store.to_owned(),
                work,
            },
            busy: Mutex::new(()),
        })
    }

    /// The folder this history keeps.
    pub fn folder(&self) -> &Path {
        &self.git.work
    }

    /// Takes a snapshot if anything changed since the folder's current
    /// one, saying `reason`; `None` when nothing did.
    pub fn capture(&self, reason: &str) -> Result<Option<Snapshot>> {
        let _busy = self.lock();
        self.capture_now(reason)
    }

    /// The snapshots, newest first, at most `limit` of them.
    pub fn list(&self, limit: usize) -> Result<Vec<Snapshot>> {
        if self
            .git
            .ask(&["rev-parse", "--verify", "-q", BRANCH])
            .is_none()
        {
            return Ok(Vec::new());
        }
        let cursor = self.cursor();
        let limit = format!("-n{limit}");
        let listed = self.git.run(&[
            "log",
            "--first-parent",
            &limit,
            "--format=%H%x1f%ct%x1f%s%x1f%b%x1e",
            BRANCH,
        ])?;
        Ok(listed
            .split('\x1e')
            .filter_map(|record| parse(record.trim_start_matches('\n'), cursor.as_deref()))
            .collect())
    }

    /// Takes the folder back one snapshot; the one it undid, or `None`
    /// when there is none before.
    pub fn undo(&self) -> Result<Option<Snapshot>> {
        let _busy = self.lock();
        self.capture_now("before undo")?;
        let Some(cursor) = self.cursor() else {
            return Ok(None);
        };
        let Some(parent) = self
            .git
            .ask(&["rev-parse", "--verify", "-q", &format!("{cursor}^")])
        else {
            return Ok(None);
        };
        let undone = self.snapshot(&cursor)?;
        self.move_to(&parent, &format!("undo {}", undone.label))?;
        Ok(Some(undone))
    }

    /// Takes the folder forward again one snapshot after an undo; the one
    /// it redid, or `None` when there is none after.
    pub fn redo(&self) -> Result<Option<Snapshot>> {
        let _busy = self.lock();
        self.capture_now("before redo")?;
        let Some(cursor) = self.cursor() else {
            return Ok(None);
        };
        let range = format!("{cursor}..{BRANCH}");
        let later = self.git.run(&[
            "rev-list",
            "--first-parent",
            "--ancestry-path",
            "--reverse",
            &range,
        ])?;
        let Some(next) = later.lines().next() else {
            return Ok(None);
        };
        let redone = self.snapshot(next)?;
        self.move_to(next, &format!("redo {}", redone.label))?;
        Ok(Some(redone))
    }

    /// Takes the folder to the snapshot `id` (a full or short id).
    pub fn restore(&self, id: &str) -> Result<Snapshot> {
        let _busy = self.lock();
        let target = self.resolve(id)?;
        self.capture_now("before restore")?;
        let snapshot = self.snapshot(&target)?;
        self.move_to(&target, &format!("restore {}", snapshot.short()))?;
        Ok(Snapshot {
            current: true,
            ..snapshot
        })
    }

    /// What the snapshot `id` changed, as a patch.
    pub fn show(&self, id: &str) -> Result<String> {
        let target = self.resolve(id)?;
        self.git
            .run(&["show", "--no-color", "--format=%s%n%n%b", &target])
    }

    /// Lets git pack and prune the repository when it has grown enough.
    pub fn tidy(&self) -> Result<()> {
        let _busy = self.lock();
        self.git.run(&["gc", "--auto", "--quiet"]).map(drop)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.busy
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn cursor(&self) -> Option<String> {
        self.git.ask(&["rev-parse", "--verify", "-q", CURSOR])
    }

    /// The commit `id` names in this history, or an error saying so.
    fn resolve(&self, id: &str) -> Result<String> {
        let valid = !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit());
        valid
            .then(|| {
                self.git
                    .ask(&["rev-parse", "--verify", "-q", &format!("{id}^{{commit}}")])
            })
            .flatten()
            .ok_or_else(|| Error(format!("no snapshot {id}")))
    }

    fn snapshot(&self, id: &str) -> Result<Snapshot> {
        let shown = self
            .git
            .run(&["show", "-s", "--format=%H%x1f%ct%x1f%s%x1f%b", id])?;
        parse(&shown, self.cursor().as_deref()).ok_or_else(|| Error(format!("no snapshot {id}")))
    }

    fn capture_now(&self, reason: &str) -> Result<Option<Snapshot>> {
        self.git.run(&["add", "--all", "."])?;
        let tree = self.git.run(&["write-tree"])?;
        let cursor = self.cursor();
        let base = cursor
            .as_deref()
            .and_then(|cursor| self.git.ask(&["rev-parse", &format!("{cursor}^{{tree}}")]));
        if base.as_deref() == Some(tree.as_str()) {
            return Ok(None);
        }
        let changed = match &base {
            Some(base) => self
                .git
                .run(&["diff-tree", "-r", "--name-only", "-z", base, &tree])?,
            None => self
                .git
                .run(&["ls-tree", "-r", "--name-only", "-z", &tree])?,
        };
        let label = label(changed.split('\0').filter(|path| !path.is_empty()));
        let message = format!("{label}\n\n{reason}");
        let mut args = vec!["commit-tree", tree.as_str(), "-m", message.as_str()];
        if let Some(cursor) = &cursor {
            args.extend(["-p", cursor.as_str()]);
        }
        let commit = self.git.run(&args)?;
        let log = format!("{reason}: {label}");
        self.git.run(&["update-ref", "-m", &log, BRANCH, &commit])?;
        self.git.run(&["update-ref", "-m", &log, CURSOR, &commit])?;
        self.snapshot(&commit).map(Some)
    }

    /// Writes the snapshot `target`'s files into the folder and makes it
    /// the current one. The index must hold the folder as it is: taking a
    /// snapshot first sees to it.
    fn move_to(&self, target: &str, why: &str) -> Result<()> {
        #[cfg(feature = "herdr")]
        herdr::checkpoint(&self.git.work, why);
        let now = self.git.run(&["write-tree"])?;
        // Two trees: the files that differ are written or deleted, and a
        // file changed since the snapshot just taken stops it, as `git
        // checkout` would, rather than being lost.
        self.git.run(&["read-tree", "-m", "-u", &now, target])?;
        self.git.run(&["update-ref", "-m", why, CURSOR, target])?;
        Ok(())
    }
}

/// A new history repository for the folder `work`.
fn create(store: &Path, work: &Path) -> Result<()> {
    if let Some(parent) = store.parent() {
        fs::create_dir_all(parent)?;
    }
    let made = git::plain(work)
        .args(["init", "--quiet", "--bare", "--initial-branch=history"])
        .arg(store)
        .output()
        .map_err(|error| Error(format!("cannot run git: {error}")))?;
    if !made.status.success() {
        return Err(Error(format!(
            "git init: {}",
            String::from_utf8_lossy(&made.stderr).trim()
        )));
    }
    let git = Git {
        dir: store.to_owned(),
        work: work.to_owned(),
    };
    // A work tree comes from GIT_WORK_TREE each time; the reflog keeps a
    // line of snapshots an edit after an undo left behind.
    git.run(&["config", "core.bare", "false"])?;
    git.run(&["config", "core.logAllRefUpdates", "always"])?;
    Ok(())
}

/// A snapshot from git's `%H %ct %s %b`, separated by `\x1f`.
fn parse(record: &str, cursor: Option<&str>) -> Option<Snapshot> {
    let mut fields = record.splitn(4, '\x1f');
    let id = fields.next()?.trim().to_owned();
    if id.is_empty() {
        return None;
    }
    let created = fields.next()?.trim().parse().ok()?;
    let label = fields.next()?.trim().to_owned();
    let reason = fields.next().unwrap_or_default().trim().to_owned();
    Some(Snapshot {
        current: cursor == Some(id.as_str()),
        id,
        created,
        label,
        reason,
    })
}

/// "a.luau", "a.luau, b.luau", or "a.luau, b.luau and 3 more".
fn label<'a>(paths: impl Iterator<Item = &'a str>) -> String {
    let paths: Vec<&str> = paths.collect();
    let named = paths[..paths.len().min(NAMED)].join(", ");
    match paths.len() {
        0 => "no files".to_owned(),
        count if count <= NAMED => named,
        count => format!("{named} and {} more", count - NAMED),
    }
}

/// Where a folder's history is kept beside the app's other state, as
/// `history.git` or, for a profile, `history-<profile>.git`.
pub fn store_in(state: &Path, profile: Option<&str>) -> PathBuf {
    state.join(match profile {
        Some(name) => format!("history-{name}.git"),
        None => "history.git".to_owned(),
    })
}
