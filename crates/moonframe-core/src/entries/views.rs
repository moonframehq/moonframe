//! What entries' views last drew, published by the window as it draws, for
//! the host to read from any thread.

use {
    super::Entries,
    crate::{Data, ViewDiagnostic},
};

/// An entry's view as it was last drawn: see [`Entries::snapshot_view`].
#[derive(Clone, Debug, PartialEq)]
pub struct ViewSnapshot {
    /// The entry, `<namespace>:<id>`.
    pub name: String,
    /// The drawn tree as data, from [`Node::snapshot`](crate::Node::snapshot):
    /// `{ kind, path, line, class, text, width, height, children, ... }`.
    /// Sizes are those the view asked for; laid-out bounds come from a
    /// headless render (`moonframe::render_entry`).
    pub tree: Data,
    /// Whether the tree was cut at [`MAX_SNAPSHOT`](crate::MAX_SNAPSHOT).
    pub truncated: bool,
    /// What the view drew wrong, as [`ViewDiagnostic`]s.
    pub diagnostics: Vec<ViewDiagnostic>,
    /// The mod's state, when it is plain data.
    pub state: Option<Data>,
    /// The keys of `app` the view read; `None` when it read all of `app`.
    pub reads: Option<Vec<String>>,
    /// Why the mod did not load, update or draw, if it did not.
    pub error: Option<String>,
    /// Counts its draws, so a host can tell a new snapshot from the last.
    pub draws: u64,
}

impl Entries {
    /// The entry `name`'s view as the window last drew it, or `None` before
    /// it draws (or when no window shows it). Safe from any thread.
    pub fn snapshot_view(&self, name: &str) -> Option<ViewSnapshot> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .views
            .get(name)
            .cloned()
    }

    pub(crate) fn publish_view(&self, snapshot: ViewSnapshot) {
        let mut registry = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry.entries.contains_key(&snapshot.name) {
            registry.views.insert(snapshot.name.clone(), snapshot);
        }
    }
}
