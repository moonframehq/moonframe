//! The folders a change to the app lands in, on one timeline: the window's
//! own, the source of a linked app, and the built-in app in a checkout.
//! Each keeps its snapshots apart; undo and redo take whichever change came
//! last or next, so stepping back walks them in the order they happened.
//!
//! An installed package is left out, as a clone with updates of its own,
//! and so is a built-in app outside a checkout, which only an update
//! changes.

use {
    crate::paths::{self, Folders},
    moonframe_history::{History, Result, Snapshot},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// The folders kept, the window's own first.
#[derive(Debug)]
pub(crate) struct Kept(Vec<History>);

/// The history of the app's folders, if it has a folder and git runs.
pub(crate) fn open(folders: &Folders) -> Option<Arc<Kept>> {
    if !cfg!(feature = "studio") {
        return None;
    }
    let own = folders.own()?;
    let state = paths::state(folders.profile.as_deref());
    let state = state.parent()?;
    let open = |folder: &Path| {
        let store = moonframe_history::store_in(state, folder);
        History::open(folder, &store)
            .inspect_err(|error| tracing::warn!("no undo for {}: {error}", folder.display()))
            .ok()
    };
    let mut kept = vec![open(&own)?];
    let roots = folders.roots();
    let layers = roots.app_layers().map(Path::to_owned).collect::<Vec<_>>();
    let others = others(&own, folders, &layers, in_checkout);
    kept.extend(others.iter().filter_map(|folder| open(folder)));
    Some(Arc::new(Kept(kept)))
}

/// The folders of `layers` kept beside `own`.
fn others(
    own: &Path,
    folders: &Folders,
    layers: &[PathBuf],
    in_checkout: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let resolved = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_owned());
    let (own, builtin) = (resolved(own), resolved(&folders.builtin));
    let user = folders.user.as_deref().map(resolved);
    let mut kept: Vec<PathBuf> = Vec::new();
    for layer in layers.iter().map(|layer| resolved(layer)) {
        // What is inside the user's folder (an installed app) is a clone.
        let installed = user.as_ref().is_some_and(|user| layer.starts_with(user));
        let shipped = layer == builtin && !in_checkout(&layer);
        let bundled = paths::resources()
            .and_then(|resources| resources.canonicalize().ok())
            .is_some_and(|resources| layer.starts_with(resources));
        if layer != own && !installed && !shipped && !bundled && !kept.contains(&layer) {
            kept.push(layer);
        }
    }
    kept
}

/// Whether `folder` is in a git checkout, where it is someone's source.
fn in_checkout(folder: &Path) -> bool {
    folder
        .ancestors()
        .any(|folder| folder.join(".git").exists())
}

impl Kept {
    #[cfg(test)]
    pub(crate) fn of(histories: Vec<History>) -> Self {
        Self(histories)
    }

    pub(crate) fn histories(&self) -> &[History] {
        &self.0
    }

    /// The window's own folder.
    pub(crate) fn folder(&self) -> &Path {
        self.0.first().map_or(Path::new(""), History::folder)
    }

    /// Whether more than the window's own folder is kept.
    pub(crate) fn several(&self) -> bool {
        self.0.len() > 1
    }

    /// Takes a snapshot of each folder that changed; those it took.
    pub(crate) fn capture(&self, reason: &str) -> Result<Vec<Snapshot>> {
        let mut taken = Vec::new();
        for history in &self.0 {
            taken.extend(history.capture(reason)?);
        }
        Ok(taken)
    }

    /// The last `count` snapshots of every folder, newest first.
    pub(crate) fn list(&self, count: usize) -> Result<Vec<(PathBuf, Snapshot)>> {
        let mut listed = Vec::new();
        for history in &self.0 {
            let folder = history.folder();
            let snapshots = history.list(count)?;
            listed.extend(snapshots.into_iter().map(|one| (folder.to_owned(), one)));
        }
        listed.sort_by_key(|(_, snapshot)| std::cmp::Reverse(snapshot.created));
        listed.truncate(count);
        Ok(listed)
    }

    /// Undoes the newest change in any folder.
    pub(crate) fn undo(&self) -> Result<Option<Snapshot>> {
        self.capture("before undo")?;
        let last = self
            .0
            .iter()
            .filter_map(|history| Some((history.undoable()?.created, history)))
            .max_by_key(|(created, _)| *created);
        last.map_or(Ok(None), |(_, history)| history.undo())
    }

    /// Redoes the oldest change undone in any folder.
    pub(crate) fn redo(&self) -> Result<Option<Snapshot>> {
        self.capture("before redo")?;
        let next = self
            .0
            .iter()
            .filter_map(|history| Some((history.redoable()?.created, history)))
            .min_by_key(|(created, _)| *created);
        next.map_or(Ok(None), |(_, history)| history.redo())
    }

    /// Takes the folder holding the snapshot `id` to it.
    pub(crate) fn restore(&self, id: &str) -> Result<Snapshot> {
        self.holding(id).restore(id)
    }

    /// What the snapshot `id` changed.
    pub(crate) fn show(&self, id: &str) -> Result<String> {
        self.holding(id).show(id)
    }

    /// The history with the snapshot `id`, else the first, which says so.
    fn holding(&self, id: &str) -> &History {
        let found = self.0.iter().find(|history| history.has(id));
        found.unwrap_or(&self.0[0])
    }

    pub(crate) fn tidy(&self) -> Result<()> {
        self.0.iter().try_for_each(History::tidy)
    }

    /// Marks every folder as it is before the run `run`.
    #[cfg(any(test, feature = "acp"))]
    pub(crate) fn mark_run(&self, run: &str) -> Result<()> {
        let reason = format!("before run {run}");
        self.0
            .iter()
            .try_for_each(|history| history.mark(run, &reason))
    }

    /// The newest run marked.
    pub(crate) fn last_run(&self) -> Option<String> {
        let marks = self.0.iter().filter_map(|history| history.marks().ok());
        marks.flatten().max()
    }

    /// Takes every folder back to before the run `run`, and says how many
    /// had a mark of it.
    pub(crate) fn undo_run(&self, run: &str) -> Result<usize> {
        let mut restored = 0;
        for history in &self.0 {
            if let Some(marked) = history.marked(run) {
                history.restore(&marked)?;
                restored += 1;
            }
        }
        Ok(restored)
    }
}

#[cfg(feature = "acp")]
impl moonframe_acp::Keeper for Kept {
    fn before(&self, run: &str) {
        if let Err(error) = self.mark_run(run) {
            tracing::warn!("no way back from the run {run}: {error}");
        }
    }

    fn after(&self, run: &str) -> bool {
        let reason = format!("after run {run}");
        if let Err(error) = self.capture(&reason) {
            tracing::warn!("no snapshot after the run {run}: {error}");
        }
        self.0.iter().any(|history| history.moved_from(run))
    }

    fn undo(&self, run: &str) -> std::result::Result<(), String> {
        self.undo_run(run)
            .map(drop)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, std::fs};

    fn folders(builtin: &Path, user: &Path) -> Folders {
        Folders {
            builtin: builtin.to_owned(),
            host: None,
            user: Some(user.to_owned()),
            profile: None,
            bundled: None,
        }
    }

    #[test]
    fn a_linked_source_is_kept_and_an_installed_clone_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (builtin, user) = (root.join("app"), root.join("user"));
        let (linked, clone) = (root.join("source"), user.join("packages/app"));
        for folder in [&builtin, &linked, &clone] {
            fs::create_dir_all(folder).unwrap();
        }
        let layers = [builtin.clone(), linked.clone(), clone, user.clone()];
        let found = others(&user, &folders(&builtin, &user), &layers, |_| false);
        assert_eq!(found, [linked]);
    }

    #[test]
    fn the_built_in_app_is_kept_only_in_a_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (builtin, user) = (root.join("app"), root.join("user"));
        fs::create_dir_all(&builtin).unwrap();
        fs::create_dir_all(&user).unwrap();
        let layers = [builtin.clone(), user.clone()];
        let window = folders(&builtin, &user);
        assert_eq!(others(&user, &window, &layers, |_| true), layers[..1]);
        assert!(others(&user, &window, &layers, |_| false).is_empty());
    }

    fn kept(root: &Path, names: &[&str]) -> Kept {
        Kept(
            names
                .iter()
                .map(|name| {
                    let folder = root.join(name);
                    fs::create_dir_all(&folder).unwrap();
                    fs::write(folder.join("main.luau"), "-- one\n").unwrap();
                    let store = root.join(format!("{name}.git"));
                    History::open(&folder, &store).unwrap()
                })
                .collect(),
        )
    }

    #[test]
    fn undoing_a_run_takes_every_folder_back_and_redo_returns() {
        let dir = tempfile::tempdir().unwrap();
        let kept = kept(dir.path(), &["own", "source"]);
        kept.capture("launch").unwrap();
        kept.mark_run("9-1").unwrap();
        for history in kept.histories() {
            fs::write(history.folder().join("main.luau"), "-- two\n").unwrap();
        }
        kept.capture("changed").unwrap();
        assert_eq!(kept.last_run().as_deref(), Some("9-1"));
        assert_eq!(kept.undo_run("9-1").unwrap(), 2);
        for history in kept.histories() {
            let text = fs::read_to_string(history.folder().join("main.luau")).unwrap();
            assert_eq!(text, "-- one\n");
        }
        assert_eq!(kept.undo_run("no-such-run").unwrap(), 0);
        kept.redo().unwrap();
        kept.redo().unwrap();
        for history in kept.histories() {
            let text = fs::read_to_string(history.folder().join("main.luau")).unwrap();
            assert_eq!(text, "-- two\n");
        }
    }

    #[test]
    fn a_snapshot_is_found_in_whichever_folder_holds_it() {
        let dir = tempfile::tempdir().unwrap();
        let kept = kept(dir.path(), &["own", "source"]);
        kept.capture("launch").unwrap();
        let source = &kept.histories()[1];
        let file = source.folder().join("main.luau");
        fs::write(&file, "-- two\n").unwrap();
        let two = source.capture("changed").unwrap().unwrap();
        fs::write(&file, "-- three\n").unwrap();
        source.capture("changed").unwrap();
        kept.restore(&two.id).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "-- two\n");
        assert!(kept.restore("abcdef12").is_err());
    }
}
