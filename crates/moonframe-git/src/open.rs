//! `{ open = "<folder>", id = ... }`: a mod names the repository to watch,
//! for an app started where there is none (from the Finder, in `/`) or to
//! switch to another one the user picked.
//!
//! It is a write: what every mod then reads, and where every later write
//! goes, changes, so only the mods in `writers` may ask. The folder must
//! be absolute and inside a git working tree; the repository watched is
//! that tree's top, as git names it. From then on the provider follows the
//! user's terminal as before, starting from the repository opened.

use {
    crate::toplevel,
    std::path::{Path, PathBuf},
};

/// The repository holding `folder`, or why it cannot be opened.
pub(crate) fn repository(folder: &str) -> Result<PathBuf, String> {
    let path = Path::new(folder);
    if folder.is_empty() {
        return Err("say which folder to open".to_owned());
    }
    // A relative path would be read against wherever the app runs.
    if !path.is_absolute() {
        return Err(format!("{folder} is not a full path"));
    }
    if !path.is_dir() {
        return Err(format!("{folder} is not a folder"));
    }
    toplevel(path).ok_or_else(|| format!("{folder} is not a git repository"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
