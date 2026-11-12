//! What the running window hit, for `--check` to report. A headless check
//! draws each mod from a fresh `init` with no provider data, so it passes a
//! view that fails only for real: on the state a mod kept from before an
//! edit (a field it now reads that the old state lacks), or on what a
//! provider publishes. The window draws with both. So each window writes
//! `<cache>/live/<pid>-<window>.json` as it reloads and as its problems
//! change, and removes it as it closes; a check of the same folders waits
//! for the window to reload past the newest edit, then adds what it hit.
//! An agent changing the app runs that check, and a run its check fails
//! is undone, so a change that breaks the window never stays.

use {
    moonframe_core::{Roots, Workspace},
    moonframe_packages::paths,
    serde::{Deserialize, Serialize},
    std::{
        fs,
        path::{Path, PathBuf},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    },
};

/// How long a check waits for the window to reload past the newest edit.
const WAIT: Duration = Duration::from_secs(5);
/// How often a check looks again while it waits.
const POLL: Duration = Duration::from_millis(100);
/// How often a window looks at its problems.
pub(crate) const TICK: Duration = Duration::from_millis(200);
/// How deep a check looks for the newest edit.
const MAX_DEPTH: usize = 8;

/// One window's account, as written.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Report {
    pid: u32,
    /// The folders it runs on, as `Roots` prints them.
    roots: String,
    /// When it last loaded its files, in milliseconds since the epoch.
    loaded: u64,
    /// Whether it reloads as files change; one that does not never catches up.
    watching: bool,
    /// Each problem, `<module>: <error>`, as it drew last.
    problems: Vec<String>,
}

fn folder() -> PathBuf {
    paths::cache().join("live")
}

fn now() -> u64 {
    millis(SystemTime::now())
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| {
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    })
}

/// A window's side: notes reloads, and writes the report. The workspace
/// runs the views as it loads and as data arrives, painted or not, so a
/// window behind others reports as well.
#[derive(Default)]
pub(crate) struct Live {
    /// The file it writes; none for runs that keep nothing (`--no-user`,
    /// tests, screenshots).
    path: Option<PathBuf>,
    loaded: u64,
    watching: bool,
    written: Option<(u64, Vec<String>)>,
}

impl Live {
    /// For window `id`; `enabled` false writes nothing.
    pub(crate) fn new(enabled: bool, id: u64, watching: bool) -> Self {
        let path = (enabled && !cfg!(test))
            .then(|| folder().join(format!("{}-{id}.json", std::process::id())));
        Self {
            path,
            loaded: now(),
            watching,
            written: None,
        }
    }

    /// The window loaded its files again.
    pub(crate) fn reloaded(&mut self, watching: bool) {
        self.loaded = now();
        self.watching = watching;
    }

    /// Writes the report when what it says changed.
    pub(crate) fn tick(&mut self, workspace: &Workspace) {
        let Some(path) = &self.path else {
            return;
        };
        let problems: Vec<String> = workspace
            .problems()
            .into_iter()
            .map(|(name, error)| format!("{name}: {error}"))
            .collect();
        if self.written.as_ref() == Some(&(self.loaded, problems.clone())) {
            return;
        }
        let report = Report {
            pid: std::process::id(),
            roots: workspace.roots().to_string(),
            loaded: self.loaded,
            watching: self.watching,
            problems: problems.clone(),
        };
        if let Err(error) = write(path, &report) {
            tracing::debug!("cannot write {}: {error}", path.display());
            return;
        }
        self.written = Some((self.loaded, problems));
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = fs::remove_file(path);
        }
    }
}

fn write(path: &Path, report: &Report) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let partial = path.with_extension("json.partial");
    fs::write(&partial, serde_json::to_vec(report)?)?;
    fs::rename(&partial, path)
}

/// What a check learned from the running windows.
#[derive(Debug, Default)]
pub(crate) struct Seen {
    /// Windows of these folders that answered.
    pub(crate) windows: usize,
    /// What they hit, each line `window: <module>: <error>`.
    pub(crate) problems: Vec<String>,
    /// Windows that had not reloaded past the newest edit in time.
    pub(crate) behind: usize,
}

/// Reads what the windows running on `roots` hit, waiting a little for
/// each to reload past the newest edit in them.
pub(crate) fn seen(roots: &Roots) -> Seen {
    let name = roots.to_string();
    let reports = read_all(&name);
    let mut seen = Seen::default();
    if reports.is_empty() {
        return seen;
    }
    let newest = newest_edit(roots);
    let deadline = Instant::now() + WAIT;
    for (path, mut report) in reports {
        while report.watching && report.loaded < newest && Instant::now() < deadline {
            std::thread::sleep(POLL);
            match read(&path) {
                Some(again) => report = again,
                None => break,
            }
        }
        seen.windows += 1;
        if report.loaded < newest && report.watching {
            seen.behind += 1;
        }
        seen.problems.extend(
            report
                .problems
                .iter()
                .map(|problem| format!("window: {problem}")),
        );
    }
    seen.problems.sort();
    seen.problems.dedup();
    seen
}

/// The reports of live windows on the folders `roots` names; a report
/// whose process is gone is removed.
fn read_all(roots: &str) -> Vec<(PathBuf, Report)> {
    let Ok(entries) = fs::read_dir(folder()) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter_map(|path| {
            let report = read(&path)?;
            if !alive(report.pid) {
                let _ = fs::remove_file(&path);
                return None;
            }
            (report.roots == roots).then_some((path, report))
        })
        .collect()
}

fn read(path: &Path) -> Option<Report> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(unix))]
fn alive(_pid: u32) -> bool {
    true
}

/// When a file the window reloads for (`.luau`, `.ftl`) last changed in
/// `roots`, in milliseconds since the epoch.
fn newest_edit(roots: &Roots) -> u64 {
    fn visit(folder: &Path, depth: usize, newest: &mut u64) {
        if depth > MAX_DEPTH {
            return;
        }
        let Ok(entries) = fs::read_dir(folder) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || matches!(&*name, "target" | "node_modules") {
                continue;
            }
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => visit(&path, depth + 1, newest),
                Ok(_) if name.ends_with(".luau") || name.ends_with(".ftl") => {
                    if let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) {
                        *newest = (*newest).max(millis(modified));
                    }
                },
                _ => {},
            }
        }
    }
    let mut newest = 0;
    for layer in roots.layers() {
        visit(layer, 0, &mut newest);
    }
    newest
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_report_reads_back_as_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live").join("1-1.json");
        let report = Report {
            pid: 1,
            roots: "/app + /user".to_owned(),
            loaded: 42,
            watching: true,
            problems: vec!["statusbar: view: runtime error".to_owned()],
        };
        write(&path, &report).unwrap();
        assert_eq!(read(&path), Some(report));
        assert!(!path.with_extension("json.partial").exists());
    }

    #[test]
    fn the_newest_edit_is_the_newest_luau_file() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods/status");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("main.luau"), "return mod {}").unwrap();
        fs::create_dir_all(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("notes.txt"), "").unwrap();
        let roots = Roots::new(vec![dir.path().to_owned()]);
        let newest = newest_edit(&roots);
        let written = millis(
            fs::metadata(mods.join("main.luau"))
                .unwrap()
                .modified()
                .unwrap(),
        );
        assert_eq!(newest, written);
    }

    #[cfg(unix)]
    #[test]
    fn this_process_is_alive() {
        assert!(alive(std::process::id()));
    }
}
