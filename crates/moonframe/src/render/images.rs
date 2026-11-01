//! Where each `ui.image` file is, worked out once rather than every frame:
//! resolving one canonicalizes it and every allowed folder, which is
//! several filesystem calls per image per frame while anything animates.

use {
    moonframe_core::Image,
    std::{
        cell::RefCell,
        collections::HashMap,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    },
};

/// How long a file that was not there (or not allowed) is taken to be
/// missing before it is looked for again: a provider may be downloading it.
const RETRY: Duration = Duration::from_secs(1);

/// What a file resolved to, and when a missing one was looked for.
enum Resolved {
    Found(PathBuf),
    Missing(Instant),
}

thread_local! {
    /// By the mod's folder and the image's `src`; the UI thread's only.
    static CACHE: RefCell<HashMap<(PathBuf, String), Resolved>> = RefCell::new(HashMap::new());
}

/// The file `image` names for a mod in `folder`, if it may be shown; see
/// [`Image::resolve`]. `allowed` is only asked for when it is not cached.
pub(super) fn resolve(
    image: &Image,
    folder: &Path,
    allowed: impl FnOnce() -> Vec<PathBuf>,
) -> Option<PathBuf> {
    let key = (folder.to_owned(), image.src.clone());
    let now = Instant::now();
    let cached = CACHE.with_borrow(|cache| match cache.get(&key) {
        Some(Resolved::Found(path)) => Some(Some(path.clone())),
        Some(Resolved::Missing(at)) if now.duration_since(*at) < RETRY => Some(None),
        _ => None,
    });
    if let Some(cached) = cached {
        return cached;
    }
    let home = std::env::home_dir();
    let path = image.resolve(folder, home.as_deref(), &allowed());
    let resolved = match &path {
        Some(path) => Resolved::Found(path.clone()),
        None => Resolved::Missing(now),
    };
    CACHE.with_borrow_mut(|cache| cache.insert(key, resolved));
    path
}

/// Forgets every resolved file: the app's folders, or files in them, changed.
pub(crate) fn forget() {
    CACHE.with_borrow_mut(HashMap::clear);
}
