//! Loader configuration shared by the library and headless hosts.

use {
    crate::{Capabilities, Kind, Manifest},
    moonframe_core::{
        Error,
        entries::{BuildPolicy, EntryState, EntryStatuses, RootPolicy},
    },
    std::path::{Path, PathBuf},
};

#[derive(Clone, Default)]
pub struct ProviderOptions {
    pub capabilities: Capabilities,
    pub on_entry_status: Option<EntryStatuses>,
    /// Durable state for external entries, keyed by namespaced identity.
    /// None keeps state across reloads only (useful for tests/private runs).
    pub state_dir: Option<PathBuf>,
}

pub(crate) fn check_policy(
    manifest: &Manifest,
    dir: &Path,
    policy: &RootPolicy,
) -> Result<(), Error> {
    let error = match &manifest.kind {
        Kind::Process { .. } if !policy.allow_process => {
            Some("process providers are disabled for this root")
        },
        Kind::Socket { .. } if !policy.allow_socket => {
            Some("socket providers are disabled for this root")
        },
        Kind::Wasm
            if policy.build == BuildPolicy::PrebuiltOnly && dir.join("Cargo.toml").exists() =>
        {
            Some("PrebuiltOnly refuses crates; ship provider.wasm without Cargo.toml")
        },
        Kind::Wasm
            if policy.build == BuildPolicy::PrebuiltOnly
                && !dir.join(crate::PREBUILT).is_file() =>
        {
            Some("PrebuiltOnly requires provider.wasm")
        },
        _ => None,
    };
    error.map_or(Ok(()), |error| Err(Error::new(error)))
}

impl crate::Context {
    pub(crate) fn status(&self, state: EntryState, error: Option<String>) {
        if state == EntryState::Ok {
            self.ready.store(true, std::sync::atomic::Ordering::Release);
        }
        if let Some(report) = &self.statuses {
            report(moonframe_core::entries::EntryStatus {
                name: self.name.clone(),
                kind: moonframe_core::entries::EntryKind::Provider,
                state,
                error,
                revision: self.revision,
            });
        }
    }

    /// Commit only after a replacement component has instantiated. Dropping
    /// the old worker closes its queue; its final save precedes restore.
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))]
    pub(crate) fn activate(&self) {
        self.fallback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }

    pub(crate) fn has_fallback(&self) -> bool {
        self.fallback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|old| old.healthy())
    }
}
