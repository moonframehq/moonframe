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
mod agent_files;
mod alerts;
mod app;
mod assets;
mod branch_bar;
mod browser;
mod build;
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
mod panes;
mod persist;
mod profiles;
mod render;
mod settings;
mod terminal;
mod theme_picker;
mod themes;

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

// Packages, the app's folders, updates and bundles: GPUI-free, in
// crates/moonframe-packages, used here under their old module names.
pub(crate) use moonframe_packages::{APP_ID, IDENTITY, Identity, identity};
use {
    moonframe_packages::{bundle, packages, paths, update},
    std::sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

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
