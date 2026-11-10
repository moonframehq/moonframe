//! Embedding APIs for host capabilities and selected miniapps.

use {
    super::Moonframe,
    moonframe_core::entries::{Entries, EntryFiles, EntryStatus, RootPolicy},
    moonframe_providers::{HostCallLimits, HostCapability},
    std::{path::PathBuf, sync::Arc},
};

impl Moonframe {
    pub fn capability(mut self, name: &str, cap: impl HostCapability) -> Self {
        if let Err(error) = self.providers.capabilities.capability(name, cap) {
            self.errors.push(error);
        }
        self
    }

    #[cfg(feature = "tokio")]
    pub fn capability_async(
        mut self,
        name: &str,
        cap: impl moonframe_providers::AsyncHostCapability,
    ) -> Self {
        if let Err(error) = self.providers.capabilities.capability_async(name, cap) {
            self.errors.push(error);
        }
        self
    }

    pub fn host_call_limits(mut self, limits: HostCallLimits) -> Self {
        self.providers.capabilities.limits = limits;
        self
    }

    /// Registers policy for a namespace. The host selects entries explicitly
    /// with `entry` or `entry_handle().add_entry`, after its own filtering.
    pub fn root(mut self, namespace: &str, path: impl Into<PathBuf>, policy: RootPolicy) -> Self {
        if let Err(error) = self.entries.add_root(namespace, path, policy) {
            self.errors.push(error);
        }
        self
    }

    pub fn entry(mut self, namespace: &str, id: &str, path: impl Into<PathBuf>) -> Self {
        if let Err(error) = self.entries.add_entry(namespace, id, path) {
            self.errors.push(error);
        }
        self
    }

    /// A miniapp compiled into the binary (its `main.luau`, `style.luau`,
    /// `components/` and `provider/`), so a release bundle needs no files
    /// on disk: written to `~/.cache/<name>/entries/<namespace>/<id>/<hash>`
    /// as the app starts. In debug, point [`entry`](Self::entry) at the
    /// source folder instead, so edits reload live:
    ///
    /// ```ignore
    /// #[derive(moonframe::rust_embed::RustEmbed)]
    /// #[crate_path = "moonframe::rust_embed"]
    /// #[folder = "miniapps/clock"]
    /// struct Clock;
    ///
    /// let app = moonframe::Moonframe::new("hub").root("builtin", dir, policy);
    /// let app = if cfg!(debug_assertions) {
    ///     app.entry("builtin", "clock", concat!(env!("CARGO_MANIFEST_DIR"), "/miniapps/clock"))
    /// } else {
    ///     app.entry_embedded::<Clock>("builtin", "clock")
    /// };
    /// ```
    pub fn entry_embedded<E: rust_embed::RustEmbed + 'static>(
        mut self,
        namespace: &str,
        id: &str,
    ) -> Self {
        if let Err(error) = self.entries.add_entry_embedded::<E>(namespace, id) {
            self.errors.push(error);
        }
        self
    }

    /// Clone before `run` and use from any host thread. Guests never get it.
    pub fn entry_handle(&self) -> Entries {
        self.entries.clone()
    }

    /// Hears each element problem any mod's view draws (the shell's, the
    /// app's mods', entries' and their instances'): an unknown key, a wrong
    /// token, an icon that does not exist. Each is told once per module and
    /// source revision, until its view draws cleanly again; the same is
    /// logged at warn level, and an entry's go in its
    /// [`EntryStatus::view_errors`]. Called on the UI thread; keep it short.
    pub fn on_view_diagnostic(
        mut self,
        callback: impl Fn(moonframe_core::ViewDiagnostic) + Send + Sync + 'static,
    ) -> Self {
        self.view_diagnostics = Some(Arc::new(callback));
        self
    }

    /// Called on the loading thread (UI for mods, workers for providers).
    /// Keep callbacks short; enqueue work back to the embedder if needed.
    pub fn on_entry_status(
        mut self,
        callback: impl Fn(EntryStatus) + Send + Sync + 'static,
    ) -> Self {
        self.providers.on_entry_status = Some(Arc::new(callback));
        self
    }
}

/// [`Moonframe::entry_embedded`] on the [`Entries`] handle, from any host
/// thread. Before the app runs, the files wait until it knows its cache.
pub trait EmbeddedEntries {
    fn add_entry_embedded<E: rust_embed::RustEmbed + 'static>(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<(), String>;
}

impl EmbeddedEntries for Entries {
    fn add_entry_embedded<E: rust_embed::RustEmbed + 'static>(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<(), String> {
        let files: EntryFiles = Arc::new(|folder| crate::embed::materialize_into::<E>(folder));
        self.add_entry_files(namespace, id, files)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, moonframe_core::entries::RootPolicy};

    #[derive(rust_embed::RustEmbed)]
    #[folder = "tests/fixtures/miniapp"]
    struct Miniapp;

    #[test]
    fn an_embedded_entry_is_written_once_the_cache_is_known() {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let entries = Entries::default();
        entries
            .add_root("builtin", root.path(), RootPolicy::default())
            .unwrap();
        entries
            .add_entry_embedded::<Miniapp>("builtin", "clock")
            .unwrap();
        assert!(entries.snapshot().is_empty());
        assert!(entries.set_files_dir(cache.path()).is_empty());
        let entry = entries.snapshot().remove("builtin:clock").unwrap();
        assert!(entry.path.starts_with(cache.path().join("builtin/clock")));
        assert!(entry.mod_file().is_file());
        assert!(entry.style_file().is_file());
        assert!(entry.components().contains_key("badge"));
        assert!(entry.provider_dir().join("provider.toml").is_file());
        // The same files again: the same folder.
        entries
            .add_entry_embedded::<Miniapp>("builtin", "clock")
            .unwrap();
        assert_eq!(entries.snapshot()["builtin:clock"].path, entry.path);
    }
}
