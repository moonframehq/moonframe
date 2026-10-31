//! The native providers this build has: Rust linked into the app, each
//! behind its own cargo feature. A provider folder with `kind = "native"`
//! runs the one registered under its name.

use moonframe_providers::Natives;

pub(crate) fn natives() -> Natives {
    #[allow(unused_mut, reason = "empty when every native feature is off")]
    let mut natives = Natives::new();
    #[cfg(feature = "git")]
    natives.insert("git", moonframe_git::native);
    natives
}
