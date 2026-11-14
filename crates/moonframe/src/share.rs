//! The system's share menu (`app:share`): macOS's share picker, at the
//! pointer in the window that asked, for a web address, text and files.
//! Its answer, the service the user picked or none, comes back once the
//! picker closes. Elsewhere there is none yet, and the answer is none.

#[cfg(all(feature = "share", target_os = "macos"))]
mod macos;

use {futures::channel::oneshot, gpui_kit::Window, std::path::PathBuf};

/// What to share, the files checked.
#[cfg_attr(not(all(feature = "share", target_os = "macos")), allow(dead_code))]
pub(crate) struct Items {
    pub(crate) url: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) files: Vec<PathBuf>,
}

/// Where the menu shows: the window that asked, at the pointer.
#[cfg(all(feature = "share", target_os = "macos"))]
pub(crate) type Anchor = macos::Anchor;
#[cfg(not(all(feature = "share", target_os = "macos")))]
pub(crate) struct Anchor;

/// Where in `window` the menu would show, if it can.
#[cfg(all(feature = "share", target_os = "macos"))]
pub(crate) fn anchor(window: &Window) -> Result<Anchor, String> {
    macos::anchor(window)
}

#[cfg(not(all(feature = "share", target_os = "macos")))]
pub(crate) fn anchor(_: &Window) -> Result<Anchor, String> {
    Ok(Anchor)
}

/// Shows the menu for `items` at `anchor`; the service the user picks (by
/// its name, "Mail"), or none when they close it, arrives once it does.
/// Run it outside any update of the app: the picker may track like a menu.
#[cfg(all(feature = "share", target_os = "macos"))]
pub(crate) fn show(anchor: &Anchor, items: Items) -> oneshot::Receiver<Option<String>> {
    macos::show(anchor, items)
}

#[cfg(not(all(feature = "share", target_os = "macos")))]
pub(crate) fn show(_: &Anchor, _: Items) -> oneshot::Receiver<Option<String>> {
    let (answer, answered) = oneshot::channel();
    let _ = answer.send(None);
    answered
}
