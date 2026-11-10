//! Host-selected miniapps outside the app's normal layers.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Whether a root may build source on the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildPolicy {
    Cargo,
    #[default]
    PrebuiltOnly,
}

/// Permissions for an external root. Defaults suit end-user miniapps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RootPolicy {
    pub build: BuildPolicy,
    pub allow_process: bool,
    pub allow_socket: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub namespace: String,
    pub path: PathBuf,
    pub root: PathBuf,
    pub policy: RootPolicy,
}

/// A cloneable, thread-safe handle for host-selected entries. Roots set policy;
/// discovery/filtering belongs to the host, which calls `add_entry` for each.
#[derive(Clone, Default)]
pub struct Entries(Arc<Mutex<Registry>>);

#[derive(Default)]
struct Registry {
    roots: BTreeMap<String, (PathBuf, RootPolicy)>,
    entries: BTreeMap<String, Entry>,
    changed: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Entries {
    /// Registers a namespace and its policy; does not scan the directory.
    pub fn add_root(
        &self,
        namespace: &str,
        path: impl Into<PathBuf>,
        policy: RootPolicy,
    ) -> Result<(), String> {
        if !valid_namespace(namespace) {
            return Err(format!("invalid or reserved entry namespace `{namespace}`"));
        }
        let path = std::path::absolute(path.into()).map_err(|e| e.to_string())?;
        let changed = {
            let mut registry = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for entry in registry
                .entries
                .values_mut()
                .filter(|e| e.namespace == namespace)
            {
                entry.root = path.clone();
                entry.policy = policy.clone();
            }
            registry.roots.insert(namespace.to_owned(), (path, policy));
            registry.changed.clone()
        };
        if let Some(changed) = changed {
            changed();
        }
        Ok(())
    }

    pub fn remove_root(&self, namespace: &str) {
        let changed = {
            let mut registry = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            registry.roots.remove(namespace);
            registry
                .entries
                .retain(|_, entry| entry.namespace != namespace);
            registry.changed.clone()
        };
        if let Some(changed) = changed {
            changed();
        }
    }

    /// Adds/replaces one selected entry. Its path need not be inside the root:
    /// only the trusted embedder has this handle, never a guest.
    pub fn add_entry(
        &self,
        namespace: &str,
        id: &str,
        path: impl Into<PathBuf>,
    ) -> Result<(), String> {
        if !plain_name(id) {
            return Err(format!("invalid entry id `{id}`"));
        }
        let path = std::path::absolute(path.into()).map_err(|e| e.to_string())?;
        let changed = {
            let mut registry = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let (root, policy) = registry
                .roots
                .get(namespace)
                .cloned()
                .ok_or_else(|| format!("no entry root `{namespace}`"))?;
            let name = format!("{namespace}:{id}");
            registry.entries.insert(name.clone(), Entry {
                name,
                namespace: namespace.to_owned(),
                path,
                root,
                policy,
            });
            registry.changed.clone()
        };
        if let Some(changed) = changed {
            changed();
        }
        Ok(())
    }

    pub fn remove_entry(&self, namespace: &str, id: &str) {
        let changed = {
            let mut registry = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            registry.entries.remove(&format!("{namespace}:{id}"));
            registry.changed.clone()
        };
        if let Some(changed) = changed {
            changed();
        }
    }

    pub fn snapshot(&self) -> BTreeMap<String, Entry> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .clone()
    }

    /// Wakes an embedding event loop after registry changes, outside the lock.
    pub fn on_change(&self, callback: impl Fn() + Send + Sync + 'static) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .changed = Some(Arc::new(callback));
    }

    pub fn apply(&self, mut roots: crate::Roots) -> crate::Roots {
        roots.set_entries(self.snapshot());
        roots
    }
}

fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_".contains(&b))
}

fn valid_namespace(name: &str) -> bool {
    plain_name(name)
        && !matches!(
            name,
            "app" | "provider" | "topic" | "demo" | "component" | "shell" | "parent"
        )
}

/// A plain module name, or exactly one namespace and id separated by `:`.
pub fn module_name(name: &str) -> bool {
    match name.split_once(':') {
        Some((namespace, id)) => valid_namespace(namespace) && plain_name(id),
        None => crate::style::valid_name(name),
    }
}

/// Status of an attempted revision. `stale` means the last good one still runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryStatus {
    pub name: String,
    pub kind: EntryKind,
    pub state: EntryState,
    pub error: Option<String>,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Mod,
    Provider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryState {
    Ok,
    Building,
    Stale,
    Failed,
}

pub type EntryStatuses = Arc<dyn Fn(EntryStatus) + Send + Sync>;

impl Entry {
    pub fn provider_dir(&self) -> PathBuf {
        self.path.join("provider")
    }

    pub fn mod_file(&self) -> PathBuf {
        self.path.join("main.luau")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
