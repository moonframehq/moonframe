//! Moonframe: a window whose whole layout comes from Luau modules, the
//! app's own folder with the user's over it, reloaded as they change.
//!
//! The `moonframe` binary is the starter app built on this library. Another
//! app uses it the same way, as a dependency: it names itself, gives its own
//! app folder (a `main.luau`, mods, a `style.luau`), and adds what only Rust
//! can do, as native providers ([`Native`]) and views ([`HostView`]) that
//! its Luau places with `ui.view`. Moonframe brings the rest: the shell's
//! window and menus, the components, the built-in providers, themes, the
//! user's folder, packages and profiles, and the `--check`, `mod` and `app`
//! commands.
//!
//! ```no_run
//! fn main() -> std::process::ExitCode {
//!     moonframe::Moonframe::new("notes")
//!         .title("Notes")
//!         .app_id("dev.example.notes")
//!         .app_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/app"))
//!         .run()
//! }
//! ```
//!
//! Use the GPUI this crate re-exports ([`gpui_kit`]): an app links one GPUI.

mod actions;
mod app;
mod assets;
mod branch_bar;
mod browser;
mod build;
mod bundle;
mod cli;
mod color;
mod demos;
mod dock;
mod embed;
mod gallery;
mod hosted;
mod init;
mod input;
mod inspect;
mod keys;
mod launch;
mod mod_window;
mod motion;
mod natives;
mod overlay;
mod packages;
mod paths;
mod persist;
mod profiles;
mod render;
mod settings;
mod terminal;
mod theme_picker;
mod themes;
mod update;

#[cfg(feature = "tokio")]
pub use moonframe_providers::{AsyncNative, on_runtime, runtime, tokio};
pub use {
    gpui_kit,
    hosted::{HostView, Messenger, ViewInit},
    launch::Moonframe,
    moonframe_core::{self, Data},
    moonframe_providers::{Handle, Native},
    rust_embed,
};

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

/// The starter's id for the window system, and its bundle's
/// (scripts/bundle-app.sh). A profile adds its name, so each app shows as
/// its own.
pub(crate) const APP_ID: &str = "dev.moonframe.app";

/// Who this run is: set once at launch from [`Moonframe`], read wherever
/// the app names itself or its folders.
#[derive(Clone, Debug)]
pub(crate) struct Identity {
    /// Names its folders (`~/.config/<name>`, `~/.cache/<name>`), its state
    /// and its document in herdr-core.
    pub(crate) name: String,
    /// Its id for the window system; a profile adds its own name.
    pub(crate) id: String,
    /// Its name for people, unless the app's manifest says another.
    pub(crate) title: String,
    /// Its icon (a PNG), unless its app folder has one.
    pub(crate) icon: Option<&'static [u8]>,
    /// Its version, for its bundle.
    pub(crate) version: String,
    /// The GitHub repository (`owner/repo`) whose releases update it:
    /// Check for Updates in the app menu.
    pub(crate) updates: Option<String>,
    /// Built on the library by another app rather than the starter: the
    /// built-in app is only a toolbox under the host's own.
    pub(crate) hosted: bool,
}

impl Identity {
    pub(crate) fn starter() -> Self {
        Self {
            name: "moonframe".to_owned(),
            id: APP_ID.to_owned(),
            title: "Moonframe".to_owned(),
            icon: None,
            version: env!("CARGO_PKG_VERSION").to_owned(),
            updates: Some("penso/moonframe".to_owned()),
            hosted: false,
        }
    }
}

pub(crate) static IDENTITY: OnceLock<Identity> = OnceLock::new();

/// This run's identity; the starter's until one is set (tests, tools).
pub(crate) fn identity() -> &'static Identity {
    IDENTITY.get_or_init(Identity::starter)
}

/// This run's app id, set once at launch: a profile's adds its name.
pub(crate) static RUNNING_ID: OnceLock<String> = OnceLock::new();

/// The app id every window of this run opens with.
pub(crate) fn app_id() -> String {
    RUNNING_ID
        .get()
        .cloned()
        .unwrap_or_else(|| identity().id.clone())
}

/// Whether this run is a release (`--release`, or an app bundle): set once
/// at launch, read by the menus and key bindings.
pub(crate) static RELEASE: AtomicBool = AtomicBool::new(false);

/// Whether developer tools are left out; see [`RELEASE`].
pub(crate) fn release() -> bool {
    RELEASE.load(Ordering::Relaxed)
}

/// Where this app (or profile) keeps the user's Developer Mode choice:
/// set once at launch.
pub(crate) static DEVELOPER_FILE: OnceLock<std::path::PathBuf> = OnceLock::new();

/// The user's Developer Mode choice for this app, if they made one.
pub(crate) fn developer_choice(file: &std::path::Path) -> Option<bool> {
    match std::fs::read_to_string(file).ok()?.trim() {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

/// Turns Developer Mode on or off for this run, and keeps the choice for
/// the next ones: the Components menu, Inspect Element and Reload Modules,
/// and their keys.
pub(crate) fn set_developer(on: bool) {
    RELEASE.store(!on, Ordering::Relaxed);
    if let Some(file) = DEVELOPER_FILE.get() {
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::write(
                    file,
                    if on {
                        "on\n"
                    } else {
                        "off\n"
                    },
                )
            });
        if let Err(error) = written {
            tracing::warn!("cannot keep Developer Mode in {}: {error}", file.display());
        }
    }
}

/// This run's name for the menu bar: the app's title in a profile.
pub(crate) static TITLE: OnceLock<String> = OnceLock::new();

/// The app's name in the menu bar; see [`TITLE`].
pub(crate) fn title() -> String {
    TITLE
        .get()
        .cloned()
        .unwrap_or_else(|| identity().title.clone())
}

/// Whether this run opens its windows behind others (`--background`): set
/// once at launch, read by every window the app opens.
pub(crate) static BACKGROUND: AtomicBool = AtomicBool::new(false);

/// Whether windows open without taking focus; see [`BACKGROUND`].
pub(crate) fn background() -> bool {
    BACKGROUND.load(Ordering::Relaxed)
}

/// Whether the components window shows only the chosen component's preview,
/// filling the window (`--preview-only`, for screenshots): set once at launch.
pub(crate) static PREVIEW_ONLY: AtomicBool = AtomicBool::new(false);

/// Whether the components window draws the preview alone; see [`PREVIEW_ONLY`].
pub(crate) fn preview_only() -> bool {
    PREVIEW_ONLY.load(Ordering::Relaxed)
}
