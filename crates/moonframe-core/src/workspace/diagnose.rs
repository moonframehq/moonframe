//! Reporting what views draw wrong: each element problem once per module
//! and revision, to the host's callback, to an entry's status, and to the
//! log. A module's set clears when its source changes and when its view
//! draws without problems, so a problem that comes back is told again.

use {
    super::*,
    crate::{ViewDiagnostic, entries::EntryStatus},
    std::sync::Arc,
};

/// Called with each new element problem of any module's view.
pub type ViewDiagnostics = Arc<dyn Fn(ViewDiagnostic) + Send + Sync>;

/// What was told of one module: its revision, and each problem by path and
/// message.
#[derive(Default)]
pub(super) struct Told {
    loads: u64,
    seen: BTreeSet<(String, String)>,
}

impl Workspace {
    /// Hears each element problem a view draws, once until it is fixed or
    /// the module's source changes: every module's, the shell's included.
    pub fn on_view_diagnostic(&mut self, callback: ViewDiagnostics) {
        self.view_diagnostics = Some(callback);
        self.told.clear();
        self.report_views();
    }

    /// Reports what is new since the last render.
    pub(super) fn report_views(&mut self) {
        let slots = std::iter::once((SHELL, &self.shell))
            .chain(self.mods.iter().map(|(name, slot)| (name.as_str(), slot)))
            .chain(
                self.instances
                    .iter()
                    .map(|(key, mounted)| (key.as_str(), &mounted.slot)),
            );
        let mut changed_entries = BTreeSet::new();
        let mut fresh = Vec::new();
        let mut cleared = Vec::new();
        for (name, slot) in slots {
            let clean = slot.broken.is_empty() && slot.notes.is_empty();
            if clean {
                if self
                    .told
                    .get(name)
                    .is_some_and(|told| !told.seen.is_empty())
                {
                    cleared.push(name.to_owned());
                }
                continue;
            }
            let told = self.told.entry(name.to_owned()).or_default();
            if told.loads != slot.loads {
                *told = Told {
                    loads: slot.loads,
                    seen: BTreeSet::new(),
                };
            }
            for diagnostic in slot.view_diagnostics(name) {
                if told
                    .seen
                    .insert((diagnostic.path.clone(), diagnostic.message.clone()))
                {
                    fresh.push(diagnostic);
                }
            }
        }
        for name in cleared {
            self.told.remove(&name);
            if let Some(entry) = self.entry_of(&name) {
                changed_entries.insert(entry);
            }
        }
        for diagnostic in &fresh {
            tracing::warn!(module = %diagnostic.module, "view: {diagnostic}");
            if let Some(entry) = self.entry_of(&diagnostic.module) {
                changed_entries.insert(entry);
            }
        }
        if let Some(callback) = &self.view_diagnostics {
            for diagnostic in fresh {
                callback(diagnostic);
            }
        }
        // A reload reports every entry once it is done.
        if !self.reloading {
            for entry in changed_entries {
                self.report_entry(&entry);
            }
        }
    }

    /// Publishes what each entry's view draws to `entries`, for
    /// [`Entries::snapshot_view`](crate::entries::Entries::snapshot_view).
    pub fn share_views(&mut self, entries: crate::entries::Entries) {
        self.shared = Some((entries, BTreeMap::new()));
        self.publish_views();
    }

    /// Publishes each entry view drawn since it was last published.
    pub(super) fn publish_views(&mut self) {
        let Some((entries, published)) = &self.shared else {
            return;
        };
        let mut snapshots = Vec::new();
        for name in self.roots.external_entries().keys() {
            let Some(slot) = self.mods.get(name) else {
                continue;
            };
            if published.get(name) == Some(&slot.draws) && slot.draws > 0 {
                continue;
            }
            snapshots.push(snapshot(self, name, slot));
        }
        if snapshots.is_empty() {
            return;
        }
        let entries = entries.clone();
        for snapshot in snapshots {
            if let Some((_, published)) = &mut self.shared {
                published.insert(snapshot.name.clone(), snapshot.draws);
            }
            entries.publish_view(snapshot);
        }
    }

    /// The mod `name`'s last view as a snapshot: its tree as data, what it
    /// drew wrong, its state and the `app` keys it read.
    pub fn view_snapshot(&self, name: &str) -> Option<crate::entries::ViewSnapshot> {
        Some(snapshot(self, name, self.mods.get(name)?))
    }

    /// The external entry the module or instance `key` belongs to.
    fn entry_of(&self, key: &str) -> Option<String> {
        let mut key = key;
        for _ in 0..32 {
            if self.roots.external_entries().contains_key(key) {
                return Some(key.to_owned());
            }
            let mounted = self.instances.get(key)?;
            key = match &mounted.of {
                InstanceOf::Mod(name) if self.roots.external_entries().contains_key(name) => name,
                _ => &mounted.owner,
            };
        }
        None
    }

    /// Every element problem the entry `name`'s views drew last, its
    /// instances' included.
    pub(crate) fn entry_view_errors(&self, name: &str) -> Vec<ViewDiagnostic> {
        let own = self
            .mods
            .get(name)
            .map(|slot| slot.view_diagnostics(name))
            .unwrap_or_default();
        let instances = self
            .instances
            .iter()
            .filter(|(key, _)| self.entry_of(key).as_deref() == Some(name))
            .flat_map(|(key, mounted)| mounted.slot.view_diagnostics(key));
        own.into_iter().chain(instances).collect()
    }

    fn report_entry(&self, name: &str) {
        if let (Some(report), Some(status)) = (&self.entry_statuses, self.entry_status(name)) {
            report(status);
        }
    }

    pub(super) fn entry_status(&self, name: &str) -> Option<EntryStatus> {
        self.mods.get(name)?;
        self.roots.external_entries().get(name)?;
        Some(self.status_of(name))
    }
}

/// The mod `name`'s last view, from its `slot`, as a snapshot.
fn snapshot(workspace: &Workspace, name: &str, slot: &Slot) -> crate::entries::ViewSnapshot {
    let (tree, truncated) = slot.view.as_ref().map_or((Data::Nil, false), |view| {
        let snapshot = view.snapshot();
        (snapshot.tree, snapshot.truncated)
    });
    crate::entries::ViewSnapshot {
        name: name.to_owned(),
        tree,
        truncated,
        diagnostics: workspace.entry_view_errors(name),
        state: slot.state().and_then(Result::ok),
        reads: match &slot.reads {
            Reads::All => None,
            Reads::Keys(keys) => Some(keys.iter().cloned().collect()),
        },
        error: slot.error().map(ToString::to_string),
        draws: slot.draws,
    }
}
