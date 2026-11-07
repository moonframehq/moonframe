//! A snapshot laid out apart from its folder, to look at it or try it
//! without going back to it.

use {
    crate::{History, Result},
    std::{fs, io, path::Path},
};

impl History {
    /// Writes what the folder would hold at the snapshot `id` to the new
    /// folder `to`, leaving the kept folder alone. Going to a snapshot leaves
    /// what snapshots leave out where it is, so that comes along: ignored
    /// files as they are now, and ignored folders linked, however big.
    pub fn export(&self, id: &str, to: &Path) -> Result<()> {
        let target = self.resolve(id)?;
        fs::create_dir_all(to)?;
        self.git.export(&target, to)?;
        let left_out = self.git.run(&[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ])?;
        for path in left_out.split('\0').filter(|path| !path.is_empty()) {
            let path = path.trim_end_matches('/');
            lay(&self.git.work.join(path), &to.join(path))?;
        }
        Ok(())
    }
}

/// Puts `from` at `to`, around what the snapshot wrote there, which wins as
/// it would in the folder. A file is never a link: a provider refuses one.
fn lay(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    let there = fs::symlink_metadata(to).ok();
    if !from.is_dir() {
        if there.is_none() && from.exists() {
            fs::hard_link(from, to).or_else(|_| fs::copy(from, to).map(drop))?;
        }
        return Ok(());
    }
    match there {
        None if link_folder(from, to).is_ok() => return Ok(()),
        Some(there) if !there.is_dir() => return Ok(()),
        _ => {},
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        lay(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}

#[cfg(unix)]
fn link_folder(from: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(from, to)
}

/// Links need a privilege on Windows: without it the folder is copied.
#[cfg(windows)]
fn link_folder(from: &Path, to: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(from, to)
}
