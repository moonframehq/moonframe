//! Where each `ui.image` file is, worked out once rather than every frame:
//! resolving one canonicalizes it and every allowed folder, which is
//! several filesystem calls per image per frame while anything animates.

use {
    moonframe_core::Image,
    std::{
        cell::RefCell,
        collections::HashMap,
        path::{Path, PathBuf},
        time::{Duration, Instant, SystemTime},
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
    /// Files and folders the user picked (`app:pick`) or dropped on the
    /// window this run: what they chose to show, which `ui.image` may.
    static GRANTED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

/// Most picked or dropped paths kept; the oldest go first.
const MAX_GRANTED: usize = 256;

/// Lets `ui.image` show `paths` (and what is in those that are folders),
/// which the user picked or dropped.
pub(crate) fn grant(paths: &[PathBuf]) {
    GRANTED.with_borrow_mut(|granted| {
        for path in paths {
            let path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !granted.contains(&path) {
                granted.push(path);
            }
        }
        let over = granted.len().saturating_sub(MAX_GRANTED);
        granted.drain(..over);
    });
    // A picture looked for before it was granted is cached as missing.
    forget();
}

/// The paths [`grant`] let through.
pub(super) fn granted() -> Vec<PathBuf> {
    GRANTED.with_borrow(Clone::clone)
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

thread_local! {
    /// Each picture's own size in pixels, by file, with the length and
    /// time it had: a file written again is read again.
    static SIZES: RefCell<HashMap<PathBuf, (u64, Option<SystemTime>, Option<(f32, f32)>)>> =
        RefCell::new(HashMap::new());
}

/// The picture's own width and height in pixels, from its header; `None`
/// for a file that is not a picture this can read.
pub(super) fn natural(path: &Path) -> Option<(f32, f32)> {
    let metadata = std::fs::metadata(path).ok()?;
    let stamp = (metadata.len(), metadata.modified().ok());
    if let Some(size) = SIZES.with_borrow(|sizes| {
        sizes
            .get(path)
            .filter(|(len, at, _)| (*len, *at) == stamp)
            .map(|(.., size)| *size)
    }) {
        return size;
    }
    let size = imagesize::size(path)
        .ok()
        .filter(|size| size.width > 0 && size.height > 0)
        .map(|size| (size.width as f32, size.height as f32));
    SIZES.with_borrow_mut(|sizes| sizes.insert(path.to_owned(), (stamp.0, stamp.1, size)));
    size
}

/// Forgets every resolved file: the app's folders, or files in them, changed.
pub(crate) fn forget() {
    CACHE.with_borrow_mut(HashMap::clear);
}
