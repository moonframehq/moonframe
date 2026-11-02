//! **Check for Updates…** in the app menu: asks GitHub for the newest
//! release of the app's repository (`owner/repo`), compares it with the
//! running version, and offers it. On macOS, an app running from its own
//! `.app` installs the release's disk image (or zip) in place of itself and
//! starts again; anywhere else, the release's page opens to download from.
//!
//! The repository is the one a library app names with
//! `moonframe::Moonframe::updates` (the starter's is
//! `moonframehq/moonframe`), or in a profile its manifest's `updates`, with its
//! `version`. Releases follow GitHub's usual path: a tag `v1.2.3` (the `v`
//! is optional), published (not a draft or pre-release), with the builds as
//! assets named for their platform, such as
//! `App-1.2.3-universal-apple-darwin.dmg`, and optionally a `SHA256SUMS`
//! file the download is checked against.

mod install;
mod release;

pub use {
    install::{relaunch, stage},
    release::{Found, Release, check},
};

use std::{
    fs,
    path::PathBuf,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

/// Where this run's updates come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// The GitHub repository, `owner/repo`.
    pub(crate) repo: String,
    /// The version running now.
    pub version: String,
    /// The `.app` this run can replace with a newer one: set only on macOS,
    /// when the bundle running is the app being updated.
    pub(crate) bundle: Option<PathBuf>,
}

/// This run's source of updates, set once at launch.
pub static SOURCE: OnceLock<Option<Source>> = OnceLock::new();

/// Where this run's updates come from, if anywhere: the menu shows Check
/// for Updates only then.
pub fn source() -> Option<&'static Source> {
    SOURCE.get().and_then(Option::as_ref)
}

/// Whether `repo` names a GitHub repository: `owner/repo`, of letters,
/// digits, `-`, `_` and `.`.
pub fn valid_repo(repo: &str) -> bool {
    let part = |part: &str| {
        !part.is_empty()
            && !part.starts_with('.')
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    matches!(repo.split_once('/'), Some((owner, name)) if part(owner) && part(name))
}

/// The `.app` the running executable is in, on macOS.
pub(crate) fn running_bundle() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let exe = std::env::current_exe()
        .and_then(|exe| exe.canonicalize())
        .ok()?;
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_owned())
}

/// When the last automatic check ran, kept in the state folder per app and
/// profile.
fn stamp(profile: Option<&str>) -> PathBuf {
    crate::paths::state(profile).with_file_name(match profile {
        Some(profile) => format!("update-checked-{profile}"),
        None => "update-checked".to_owned(),
    })
}

/// How often the app checks by itself.
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// How often an open app asks whether the daily check is due: it may stay
/// open for days, and sleep through some of them.
pub const POLL: Duration = Duration::from_secs(60 * 60);

/// Whether this run checks by itself: an installed app (a bundle, or a
/// Linux prefix), or an app on the library built for release. A run from
/// source checks only when asked, and Developer Mode changes neither.
pub fn checks_by_itself() -> bool {
    crate::paths::in_bundle() || (crate::identity().hosted && !cfg!(debug_assertions))
}

/// Whether the automatic check is due: a day after the last check that
/// reached GitHub.
pub fn due(profile: Option<&str>) -> bool {
    !fs::metadata(stamp(profile))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < EVERY)
}

/// Notes that a check reached GitHub, so the next is a day away.
pub fn checked(profile: Option<&str>) {
    let file = stamp(profile);
    let written = file
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&file, b""));
    if let Err(error) = written {
        tracing::warn!(
            "cannot note the update check in {}: {error}",
            file.display()
        );
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn repositories() {
        assert!(valid_repo("moonframehq/moonframe"));
        assert!(valid_repo("my-org/my.app_2"));
        assert!(!valid_repo("penso"));
        assert!(!valid_repo("moonframehq/moonframe/extra"));
        assert!(!valid_repo("../x"));
        assert!(!valid_repo("https://github.com/moonframehq/moonframe"));
    }
}
