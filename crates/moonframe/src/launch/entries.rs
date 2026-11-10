//! Embedding APIs for host capabilities and selected miniapps.

use {
    super::Moonframe,
    moonframe_core::entries::{Entries, EntryStatus, RootPolicy},
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

    /// Clone before `run` and use from any host thread. Guests never get it.
    pub fn entry_handle(&self) -> Entries {
        self.entries.clone()
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
