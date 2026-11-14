//! The vault folder watched for changes made outside the app (Obsidian, a
//! sync, git, an agent): each changed note's path, or a full rescan when a
//! folder moved or the watcher lost track.

use {
    crate::scan,
    notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher},
    std::{
        collections::BTreeSet,
        path::{Path, PathBuf},
        sync::mpsc::{Receiver, channel},
    },
};

/// What changed since the last look.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Changes {
    pub notes: BTreeSet<String>,
    /// Something only a whole scan can follow (a folder renamed, an overflow).
    pub rescan: bool,
}

pub(super) struct Watch {
    root: PathBuf,
    events: Receiver<notify::Result<Event>>,
    // Kept alive: dropping it stops the watching.
    _watcher: RecommendedWatcher,
}

impl Watch {
    pub(super) fn start(root: &Path) -> notify::Result<Self> {
        let (sender, events) = channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        Ok(Self {
            root: root.to_path_buf(),
            events,
            _watcher: watcher,
        })
    }

    /// Everything that changed since the last call.
    pub(super) fn drain(&self) -> Changes {
        let mut changes = Changes::default();
        while let Ok(event) = self.events.try_recv() {
            match event {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Access(_)) {
                        continue;
                    }
                    if event.need_rescan() {
                        changes.rescan = true;
                    }
                    for path in &event.paths {
                        classify(&self.root, path, &mut changes);
                    }
                },
                Err(_) => changes.rescan = true,
            }
        }
        changes
    }
}

fn classify(root: &Path, path: &Path, changes: &mut Changes) {
    let Some(relative) = scan::relative(root, path) else {
        return;
    };
    if relative.is_empty() || relative.split('/').any(|part| part.starts_with('.')) {
        return;
    }
    if scan::is_note(&relative) {
        changes.notes.insert(relative);
    } else if path.is_dir() || path.extension().is_none() {
        // A folder came, went or moved: its notes did too.
        changes.rescan = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_change_folders_rescan_hidden_is_ignored() {
        let root = Path::new("/vault");
        let mut changes = Changes::default();
        classify(root, Path::new("/vault/notes/A.md"), &mut changes);
        classify(
            root,
            Path::new("/vault/.commonplace/journal/x.json"),
            &mut changes,
        );
        classify(root, Path::new("/vault/raw/assets/p.png"), &mut changes);
        assert_eq!(changes.notes.iter().collect::<Vec<_>>(), vec!["notes/A.md"]);
        assert!(!changes.rescan);
        classify(root, Path::new("/vault/old-folder"), &mut changes);
        assert!(changes.rescan);
    }
}
