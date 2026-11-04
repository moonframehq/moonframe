//! `app:icon`: the app's icon in the Dock and the app switcher, changed
//! while it runs, as a mod's state changes (busy, unread mail, a recording
//! going on).

use crate::{Data, Error, request::APP};

/// What `app:icon` shows.
#[derive(Clone, Debug, PartialEq)]
pub enum AppIcon {
    /// `"default"`: the icon the app started with, its own or Moonframe's.
    Default,
    /// An image file, as `ui.image` names one: relative to the sending
    /// mod's folder, `~/...`, or absolute, and inside the app's folders.
    File(String),
}

/// The images `app:icon` shows, by extension: those AppKit reads.
pub const ICON_TYPES: &[&str] = &["png", "icns", "jpg", "jpeg", "tiff", "pdf"];

/// Longest an icon file's name can be.
const MAX_PATH: usize = 1024;

/// `"default"`, or an image file's path: never an address.
pub(super) fn icon(msg: &Data) -> Result<AppIcon, Error> {
    let types = ICON_TYPES.join(", ");
    let Data::String(name) = msg else {
        return Err(Error::new(format!(
            "`{APP}icon` takes \"default\" or an image file's path ({types})"
        )));
    };
    if name == "default" {
        return Ok(AppIcon::Default);
    }
    if name.contains("://") {
        return Err(Error::new(format!(
            "`{APP}icon`: `{name}` is an address; it shows files in the app's folders"
        )));
    }
    let known = std::path::Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| ICON_TYPES.contains(&extension.to_lowercase().as_str()));
    if name.is_empty() || name.len() > MAX_PATH || !known {
        return Err(Error::new(format!(
            "`{APP}icon`: `{name}` is not an image file ({types})"
        )));
    }
    Ok(AppIcon::File(name.clone()))
}
