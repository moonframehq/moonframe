//! The history's repository on disk: where it goes, who may read it, which
//! folder it belongs to, and the lock every process takes to change it.

use {
    crate::{Error, Result, git::Git},
    std::{
        fs::{self, File, OpenOptions},
        path::{Path, PathBuf},
    },
};

/// The lock file inside the repository.
const LOCK: &str = "moonframe.lock";
/// The git config key naming the folder a repository keeps.
const FOLDER_KEY: &str = "moonframe.folder";

/// Where the history of `folder` is kept in the state folder `state`: one
/// repository per folder, named after it and a hash of its whole path, so
/// two folders (another `--user`, a profile) never share one.
pub fn store_in(state: &Path, folder: &Path) -> PathBuf {
    let folder = folder.canonicalize().unwrap_or_else(|_| folder.to_owned());
    let name: String = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    let hash = fnv1a(folder.as_os_str().as_encoded_bytes());
    state
        .join("history")
        .join(format!("{name}-{hash:016x}.git"))
}

/// FNV-1a, 64 bits: stable across Rust releases, as a store's name must be.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Makes `dir` (and missing parents) and keeps it to its owner: snapshots
/// copy the folder's files, which may be private, into it.
pub(crate) fn private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Records that the repository keeps `work`, or refuses one that keeps
/// another folder: its snapshots would be written into the wrong one.
pub(crate) fn bind(git: &Git) -> Result<()> {
    let folder = git.work.to_string_lossy();
    match git.ask(&["config", "--get", FOLDER_KEY]) {
        Some(kept) if kept == folder => Ok(()),
        Some(kept) => Err(Error(format!(
            "{} keeps the history of {kept}, not {folder}",
            git.dir.display()
        ))),
        None => git.run(&["config", FOLDER_KEY, &folder]).map(drop),
    }
}

/// Held while one operation changes the repository, by this process or
/// any other (the window and `moonframe history` share it); released when
/// dropped.
pub(crate) struct Lock(File);

impl Lock {
    /// Waits for the repository `dir`'s lock.
    pub(crate) fn take(dir: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(LOCK))?;
        file.lock()?;
        Ok(Self(file))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
