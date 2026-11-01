//! Finding programs a provider runs. An app opened from the Dock or a
//! launcher gets a short `PATH`, so the usual install places are searched
//! after it.

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

/// Where programs install themselves besides `PATH`, under the home
/// folder: the herdr-core installer, cargo, bun, npm, opencode, volta,
/// Claude, mise and Nix.
const USER_DIRS: [&str; 9] = [
    ".local/bin",
    ".cargo/bin",
    ".bun/bin",
    ".npm-global/bin",
    ".opencode/bin",
    ".volta/bin",
    ".claude/local",
    ".local/share/mise/shims",
    ".nix-profile/bin",
];
#[cfg(unix)]
const SYSTEM_DIRS: [&str; 4] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/run/current-system/sw/bin",
    "/usr/bin",
];
#[cfg(not(unix))]
const SYSTEM_DIRS: [&str; 0] = [];

/// `path`'s folders, then the user and system folders programs install to,
/// as a `PATH`-style list.
pub fn search_path(path: Option<&OsStr>, home: Option<&Path>) -> OsString {
    let mut dirs: Vec<PathBuf> = path
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .collect();
    dirs.extend(
        home.into_iter()
            .flat_map(|home| USER_DIRS.iter().map(|dir| home.join(dir))),
    );
    dirs.extend(SYSTEM_DIRS.iter().map(PathBuf::from));
    std::env::join_paths(dirs).unwrap_or_default()
}

/// [`search_path`] from this process's `PATH` and `HOME`.
pub fn default_search_path() -> OsString {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    search_path(std::env::var_os("PATH").as_deref(), home.as_deref())
}

/// The first executable named `program` in a `PATH`-style list.
pub fn find(program: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path).find_map(|dir| executable(&dir, program))
}

#[cfg(unix)]
fn executable(dir: &Path, program: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let candidate = dir.join(program);
    let metadata = candidate.metadata().ok()?;
    (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then_some(candidate)
}

#[cfg(windows)]
fn executable(dir: &Path, program: &str) -> Option<PathBuf> {
    ["exe", "cmd", "bat"]
        .iter()
        .map(|extension| dir.join(program).with_extension(extension))
        .find(|candidate| candidate.is_file())
}
