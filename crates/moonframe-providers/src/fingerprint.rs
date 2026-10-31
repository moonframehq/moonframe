//! What a provider's folder holds, cheaply compared, so a reload restarts
//! only the providers that changed.

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::Path,
    time::SystemTime,
};

/// Files looked at before giving up and calling the folder changed.
const MAX_FILES: usize = 10_000;
/// Folders a provider's build or tools write, which are not its source.
const SKIPPED: &[&str] = &["target", ".git"];

/// The folder's path and each file's path, size, and modification time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fingerprint(u64);

impl Fingerprint {
    pub(crate) fn of(dir: &Path) -> Self {
        let mut hasher = DefaultHasher::new();
        dir.hash(&mut hasher);
        let mut files = Vec::new();
        collect(dir, &mut files);
        files.sort();
        files.hash(&mut hasher);
        Self(hasher.finish())
    }
}

fn collect(dir: &Path, files: &mut Vec<(String, u64, Option<SystemTime>)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= MAX_FILES {
            // Too many to tell: a timestamp makes every reload a change.
            files.push((String::new(), 0, Some(SystemTime::now())));
            return;
        }
        let path = entry.path();
        let name = entry.file_name();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if !SKIPPED.iter().any(|skipped| name == *skipped) {
                collect(&path, files);
            }
        } else if name != "Cargo.lock" {
            // Cargo rewrites the lock file as it builds.
            let metadata = entry.metadata().ok();
            files.push((
                path.to_string_lossy().into_owned(),
                metadata.as_ref().map_or(0, fs::Metadata::len),
                metadata.and_then(|metadata| metadata.modified().ok()),
            ));
        }
    }
}
