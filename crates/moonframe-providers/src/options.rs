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

/// Checks a miniapp entry at `path` as a host would load it under
/// `policy`: its Luau ([`moonframe_core::entries::check_in`], over `app`
/// when given), and its `provider/provider.toml`, whose capabilities must be
/// among `capabilities`, the names the host registers.
pub fn check_entry(
    path: &Path,
    policy: &RootPolicy,
    app: Option<&moonframe_core::Roots>,
    capabilities: &[&str],
) -> Vec<moonframe_core::entries::Diagnostic> {
    use moonframe_core::entries::{Diagnostic, check_in};
    let mut found = check_in(path, policy, app);
    let dir = path.join("provider");
    let manifest = dir.join("provider.toml");
    if manifest.is_file() {
        let checked = Manifest::load_declared("check:entry", &dir, capabilities)
            .and_then(|loaded| check_policy(&loaded, &dir, policy));
        if let Err(error) = checked {
            found.push(Diagnostic::error(manifest, error.to_string()));
        }
    }
    found
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
