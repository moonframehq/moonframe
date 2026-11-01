//! The native providers this build has: Rust linked into the app, each
//! behind its own cargo feature. A provider folder with `kind = "native"`
//! runs the one registered under its name; an app on the library adds its
//! own with `Moonframe::native`.

use moonframe_providers::Natives;

pub(crate) fn natives() -> Natives {
    let mut natives = Natives::new();
    #[cfg(feature = "acp")]
    natives.insert("acp".to_owned(), std::sync::Arc::new(moonframe_acp::native));
    #[cfg(feature = "docs")]
    natives.insert(
        "docs".to_owned(),
        std::sync::Arc::new(moonframe_docs::native),
    );
    // The demo gallery's (examples/demo): it opens apps on this binary.
    natives.insert(
        "demos".to_owned(),
        std::sync::Arc::new(crate::demos::native),
    );
    #[cfg(feature = "git")]
    natives.insert("git".to_owned(), std::sync::Arc::new(moonframe_git::native));
    natives
}
