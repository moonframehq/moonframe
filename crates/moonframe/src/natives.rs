//! The native providers this build has: Rust linked into the app, each
//! behind its own cargo feature. A provider folder with `kind = "native"`
//! runs the one registered under its name; an app on the library adds its
//! own with `Moonframe::native`.

use moonframe_providers::Natives;

pub(crate) fn natives() -> Natives {
    #[allow(unused_mut, reason = "empty when every native feature is off")]
    let mut natives = Natives::new();
    #[cfg(feature = "acp")]
    natives.insert("acp".to_owned(), std::sync::Arc::new(moonframe_acp::native));
    #[cfg(feature = "docs")]
    natives.insert(
        "docs".to_owned(),
        std::sync::Arc::new(moonframe_docs::native),
    );
    #[cfg(feature = "git")]
    natives.insert("git".to_owned(), std::sync::Arc::new(moonframe_git::native));
    natives
}
