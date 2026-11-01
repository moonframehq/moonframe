//! Where the app's folders are. The built-in app is read-only (inside a
//! macOS bundle it is signed, and each update replaces it); what the user
//! and their agents write goes in `~/.config/moonframe`, laid over it.

use {
    crate::packages,
    moonframe_core::Roots,
    std::{env, path::PathBuf},
};

/// The app's name: its folders, and its document in herdr-core.
pub(crate) const NAME: &str = "moonframe";

/// The app folder the binary ships with: `Contents/Resources/app` inside a
/// macOS bundle, else `app` in the current folder (running from the
/// repository).
pub(crate) fn builtin() -> PathBuf {
    env::current_exe()
        .and_then(|exe| exe.canonicalize())
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("Resources").join("app")))
        .filter(|resources| resources.is_dir())
        .unwrap_or_else(|| PathBuf::from("app"))
}

/// `$XDG_CONFIG_HOME/moonframe`, by default `~/.config/moonframe`:
/// the user's mods, providers, and stylesheet.
pub(crate) fn user() -> Option<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config")
}

/// Where wasm providers are built: `$XDG_CACHE_HOME/moonframe/providers`,
/// by default under `~/.cache`.
pub(crate) fn builds() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache")
        .unwrap_or_else(|| env::temp_dir().join(NAME))
        .join("providers")
}

/// Where providers save pictures for `ui.image` (avatars, previews):
/// `$XDG_CACHE_HOME/moonframe/images`, by default under `~/.cache`. Views
/// may show files from here and from the app's own folders, nowhere else.
pub(crate) fn images() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache")
        .unwrap_or_else(|| env::temp_dir().join(NAME))
        .join("images")
}

/// Where the inspector saves the selected element for agents:
/// `$XDG_CACHE_HOME/moonframe/selection.md`, by default under `~/.cache`.
pub(crate) fn selection() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache")
        .unwrap_or_else(|| env::temp_dir().join(NAME))
        .join("selection.md")
}

/// Where state outlives the app (each mod's state, and topics):
/// `$XDG_STATE_HOME/moonframe/state.sqlite`, by default under
/// `~/.local/state`; outside the user's folder, which the window watches.
pub(crate) fn state() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state")
        .unwrap_or_else(|| env::temp_dir().join(NAME))
        .join("state.sqlite")
}

/// The built-in app, under the packages the user enabled and the user's
/// folder, unless `user` is `None`.
pub(crate) fn roots(builtin: PathBuf, user: Option<PathBuf>) -> Roots {
    let packages = user
        .as_deref()
        .map(packages::enabled_folders)
        .unwrap_or_default();
    Roots::with_packages(builtin, packages, user)
}

fn xdg(variable: &str, fallback: &str) -> Option<PathBuf> {
    env::var_os(variable)
        .map(PathBuf::from)
        // The spec ignores relative values.
        .filter(|dir| dir.is_absolute())
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback)))
        .map(|dir| dir.join(NAME))
}
