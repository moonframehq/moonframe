//! Finding the programs backends run. An app opened from the Dock or a
//! launcher doesn't get the user's shell PATH, so the usual install places
//! are searched after PATH.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// The first of `names` on PATH or in the usual places that `accept` takes.
pub(crate) fn find(names: &[&str], accept: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(usual_dirs())
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .filter(|candidate| candidate.is_file())
        .find(|candidate| accept(candidate))
}

/// The first line `program` prints for `args`, when it runs and succeeds.
pub(crate) fn first_line(program: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_owned())
}

/// Where these programs get installed: Homebrew, the herdr-core installer
/// (`~/.local/bin`), mise, Nix, cargo, and the system.
fn usual_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let in_home = |rest: &str| home.as_ref().map(|home| home.join(rest));
    [
        in_home(".local/bin"),
        Some(PathBuf::from("/opt/homebrew/bin")),
        Some(PathBuf::from("/usr/local/bin")),
        in_home(".local/share/mise/shims"),
        in_home(".nix-profile/bin"),
        Some(PathBuf::from("/run/current-system/sw/bin")),
        in_home(".cargo/bin"),
        Some(PathBuf::from("/usr/bin")),
    ]
    .into_iter()
    .flatten()
    .collect()
}
