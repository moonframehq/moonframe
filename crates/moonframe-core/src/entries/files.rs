//! Entries whose files are compiled into the host binary. They are written
//! to a folder of their own once the host knows where its cache is, so one
//! selected before `run` waits until then.

use {
    super::{Entries, Registry, plain_name},
    std::{
        io,
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// Writes an entry's files under the folder it is given (a fresh folder
/// per content is fine) and returns the folder holding `main.luau`.
pub type EntryFiles = Arc<dyn Fn(&Path) -> io::Result<PathBuf> + Send + Sync>;

#[derive(Default)]
pub(super) struct Pending {
    /// Where entries' files go: `<cache>/entries`. None until the host sets it.
    dir: Option<PathBuf>,
    waiting: Vec<(String, String, EntryFiles)>,
}

impl Pending {
    pub(super) fn forget(&mut self, namespace: &str, id: &str) {
        self.waiting
            .retain(|(ns, entry, _)| !(ns == namespace && entry == id));
    }

    pub(super) fn forget_root(&mut self, namespace: &str) {
        self.waiting.retain(|(ns, ..)| ns != namespace);
    }
}

fn lock(entries: &Entries) -> std::sync::MutexGuard<'_, Registry> {
    entries
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Entries {
    /// Adds an entry whose files `files` writes, such as one compiled into
    /// the binary. Written to `<dir>/<namespace>/<id>` once
    /// [`set_files_dir`](Self::set_files_dir) has run, at once after that.
    pub fn add_entry_files(
        &self,
        namespace: &str,
        id: &str,
        files: EntryFiles,
    ) -> Result<(), String> {
        if !plain_name(id) {
            return Err(format!("invalid entry id `{id}`"));
        }
        let dir = {
            let mut registry = lock(self);
            if !registry.roots.contains_key(namespace) {
                return Err(format!("no entry root `{namespace}`"));
            }
            match registry.files.dir.clone() {
                Some(dir) => dir,
                None => {
                    registry.files.forget(namespace, id);
                    registry
                        .files
                        .waiting
                        .push((namespace.to_owned(), id.to_owned(), files));
                    return Ok(());
                },
            }
        };
        let folder = files(&dir.join(namespace).join(id))
            .map_err(|error| format!("cannot write entry `{namespace}:{id}`: {error}"))?;
        self.add_entry(namespace, id, folder)
    }

    /// Where embedded entries are written, set by the host as it starts:
    /// writes those added before. Returns a problem per entry it could not
    /// write.
    pub fn set_files_dir(&self, dir: impl Into<PathBuf>) -> Vec<String> {
        let waiting = {
            let mut registry = lock(self);
            registry.files.dir = Some(dir.into());
            std::mem::take(&mut registry.files.waiting)
        };
        waiting
            .into_iter()
            .filter_map(|(namespace, id, files)| self.add_entry_files(&namespace, &id, files).err())
            .collect()
    }
}
