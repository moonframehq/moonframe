//! The items mods put in the menu bar (`app:status-item`), one per mod at
//! most, kept app-wide: every main window's copy of a mod shares its item.
//! A click on one of an item's menu lines is queued for the app, which
//! hands its `msg` to the front main window's mod; see
//! `app::menu_bar`. Only macOS draws them (AppKit's `NSStatusItem`);
//! elsewhere asking for one does nothing.

#[cfg(all(feature = "status-item", target_os = "macos"))]
mod macos;

use {
    futures::channel::mpsc,
    moonframe_core::{Chord, Data},
    std::sync::OnceLock,
};

/// A menu line's click, for the app to deliver: `msg` for the mod `to`.
pub(crate) struct Clicked {
    pub(crate) to: String,
    pub(crate) msg: Data,
}

/// Where clicks go, once the app listens.
static CLICKS: OnceLock<mpsc::UnboundedSender<Clicked>> = OnceLock::new();

/// Hears the clicks on every item's menu from now on.
pub(crate) fn listen() -> Option<mpsc::UnboundedReceiver<Clicked>> {
    let (sender, clicks) = mpsc::unbounded();
    CLICKS.set(sender).ok()?;
    Some(clicks)
}

#[cfg_attr(
    not(all(feature = "status-item", target_os = "macos")),
    allow(dead_code)
)]
fn clicked(to: String, msg: Data) {
    if let Some(clicks) = CLICKS.get() {
        let _ = clicks.unbounded_send(Clicked { to, msg });
    }
}

/// An item as the platform draws it: what the mod asked, its files read.
#[cfg_attr(
    not(all(feature = "status-item", target_os = "macos")),
    allow(dead_code)
)]
pub(crate) struct Look {
    pub(crate) icon: Option<Icon>,
    pub(crate) template: bool,
    pub(crate) title: Option<String>,
    pub(crate) tooltip: Option<String>,
    pub(crate) menu: Vec<Line>,
}

#[cfg_attr(
    not(all(feature = "status-item", target_os = "macos")),
    allow(dead_code)
)]
pub(crate) enum Icon {
    Symbol(String),
    /// An image file's bytes.
    Image(Vec<u8>),
}

/// A line of an item's menu.
#[cfg_attr(
    not(all(feature = "status-item", target_os = "macos")),
    allow(dead_code)
)]
pub(crate) enum Line {
    Separator,
    Item {
        label: String,
        /// Who hears a click, and what: none for a heading.
        action: Option<(String, Data)>,
        enabled: bool,
        checked: bool,
        shortcut: Option<Chord>,
    },
}

/// Shows the mod `owner`'s item as `look`, or changes it to that.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn show(owner: &str, look: Look) -> Result<(), String> {
    macos::show(owner, look)
}

/// Drops `menu` down from the mod `owner`'s item, once. It waits for the
/// user (AppKit tracks the menu until it closes), so run it outside any
/// update of the app.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn popup(owner: &str, menu: Vec<Line>) -> Result<(), String> {
    macos::popup(owner, menu)
}

/// Takes the mod `owner`'s item away, if it has one.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn remove(owner: &str) {
    macos::remove(owner);
}

/// The mods that have an item.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn owners() -> Vec<String> {
    macos::owners()
}

// Elsewhere nothing draws them, and a mod asking is not at fault, as with
// `app:dock`.
#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn show(_: &str, _: Look) -> Result<(), String> {
    Ok(())
}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn popup(_: &str, _: Vec<Line>) -> Result<(), String> {
    Ok(())
}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn remove(_: &str) {}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn owners() -> Vec<String> {
    Vec::new()
}
