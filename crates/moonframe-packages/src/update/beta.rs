//! **Beta Releases** in the app menu: whether updates include GitHub's
//! pre-releases. The choice is kept per app and profile, in the state
//! folder, beside the daily check's stamp.

use std::{
    fs,
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

/// Whether this run is offered pre-releases; see [`load`].
static ON: AtomicBool = AtomicBool::new(false);

/// Where the choice is kept, set by [`load`].
static FILE: OnceLock<PathBuf> = OnceLock::new();

/// The file keeping `profile`'s choice.
fn file(profile: Option<&str>) -> PathBuf {
    crate::paths::state(profile).with_file_name(match profile {
        Some(profile) => format!("update-beta-{profile}"),
        None => "update-beta".to_owned(),
    })
}

/// Reads `profile`'s choice at launch: off unless the user turned it on.
pub fn load(profile: Option<&str>) {
    let file = file(profile);
    let on = fs::read_to_string(&file).is_ok_and(|text| text.trim() == "on");
    ON.store(on, Ordering::Relaxed);
    let _ = FILE.set(file);
}

/// Whether updates include pre-releases.
pub fn beta() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Turns pre-releases on or off, for this run and the next ones.
pub fn set_beta(on: bool) {
    ON.store(on, Ordering::Relaxed);
    let Some(file) = FILE.get() else {
        return;
    };
    let written = file
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| {
            fs::write(
                file,
                if on {
                    "on\n"
                } else {
                    "off\n"
                },
            )
        });
    if let Err(error) = written {
        tracing::warn!("cannot keep Beta Releases in {}: {error}", file.display());
    }
}
