//! What Moonframe does without a window: installs, links and updates
//! packages (`moonframe mod`), finds the app's folders and profiles, checks
//! for and stages updates, and bundles an app on the library. The window
//! (crates/moonframe) builds on it; nothing here names GPUI, so edits here
//! build and test without it.

pub mod bundle;
pub mod packages;
pub mod paths;
pub mod update;

use std::sync::OnceLock;

/// The starter's id for the window system, and its bundle's
/// (scripts/bundle-app.sh). A profile adds its name, so each app shows as
/// its own.
pub const APP_ID: &str = "dev.moonframe.app";

/// Who this run is: set once at launch from `moonframe::Moonframe`, read
/// wherever the app names itself or its folders.
#[derive(Clone, Debug)]
pub struct Identity {
    /// Names its folders (`~/.config/<name>`, `~/.cache/<name>`), its state
    /// and its document in herdr-core.
    pub name: String,
    /// Its id for the window system; a profile adds its own name.
    pub id: String,
    /// Its name for people, unless the app's manifest says another.
    pub title: String,
    /// Its icon (a PNG), unless its app folder has one.
    pub icon: Option<&'static [u8]>,
    /// Its version, for its bundle.
    pub version: String,
    /// The GitHub repository (`owner/repo`) whose releases update it:
    /// Check for Updates in the app menu.
    pub updates: Option<String>,
    /// Built on the library by another app rather than the starter: the
    /// built-in app is only a toolbox under the host's own.
    pub hosted: bool,
}

impl Identity {
    pub fn starter() -> Self {
        Self {
            name: "moonframe".to_owned(),
            id: APP_ID.to_owned(),
            title: "Moonframe".to_owned(),
            icon: None,
            version: env!("CARGO_PKG_VERSION").to_owned(),
            updates: Some("moonframehq/moonframe".to_owned()),
            hosted: false,
        }
    }
}

pub static IDENTITY: OnceLock<Identity> = OnceLock::new();

/// This run's identity; the starter's until one is set (tests, tools).
pub fn identity() -> &'static Identity {
    IDENTITY.get_or_init(Identity::starter)
}
