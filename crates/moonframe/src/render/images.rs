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
    /// By the drawing mod, its folder and the image's `src`; the UI
    /// thread's only. The mod is part of the key because what it may show
    /// is its own: components drawn by two mods share a folder.
    static CACHE: RefCell<HashMap<(String, PathBuf, String), Resolved>> =
        RefCell::new(HashMap::new());
    /// Files and folders the user picked (`app:pick`) or dropped on the
    /// window this run, by the mod they were given to: what they chose to
    /// show that mod, which its `ui.image` (and its components') may.
    static GRANTED: RefCell<Vec<(String, PathBuf)>> = const { RefCell::new(Vec::new()) };
}

/// Most picked or dropped paths kept; the oldest go first.
const MAX_GRANTED: usize = 256;

/// The mod a drawing mod's grants belong to: a component instance
/// (`<mod>/<component>#<id>`) shares its mod's.
fn grantee(owner: &str) -> &str {
    owner.split('/').next().unwrap_or(owner)
}

/// Lets the mod `owner` show `paths` in `ui.image` (and what is in those
/// that are folders): the user picked them for it or dropped them on it.
pub(crate) fn grant(owner: &str, paths: &[PathBuf]) {
    let owner = grantee(owner);
    GRANTED.with_borrow_mut(|granted| {
        for path in paths {
            let path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !granted
                .iter()
                .any(|(to, held)| to == owner && *held == path)
            {
                granted.push((owner.to_owned(), path));
            }
        }
        let over = granted.len().saturating_sub(MAX_GRANTED);
        granted.drain(..over);
    });
    // A picture looked for before it was granted is cached as missing.
    forget();
}

/// The paths [`grant`] gave the mod `owner`.
pub(super) fn granted(owner: &str) -> Vec<PathBuf> {
    let owner = grantee(owner);
    GRANTED.with_borrow(|granted| {
        granted
            .iter()
            .filter(|(to, _)| to == owner)
            .map(|(_, path)| path.clone())
            .collect()
    })
}

/// The file `image` names for the mod `owner` in `folder`, if it may be
/// shown; see [`Image::resolve`]. `allowed` is only asked for when it is
/// not cached.
pub(super) fn resolve(
    image: &Image,
    owner: &str,
    folder: &Path,
    allowed: impl FnOnce() -> Vec<PathBuf>,
) -> Option<PathBuf> {
    let key = (owner.to_owned(), folder.to_owned(), image.src.clone());
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_grant_is_the_mods_it_was_given_to_and_its_components() {
        let dir = tempfile::tempdir().unwrap();
        let scan = dir.path().join("scan.png");
        std::fs::write(&scan, "png").unwrap();
        grant("editor", std::slice::from_ref(&scan));
        let scan = scan.canonicalize().unwrap();
        assert_eq!(granted("editor"), std::slice::from_ref(&scan));
        // The editor's image viewer shares it; another mod does not.
        assert_eq!(granted("editor/image-viewer#main"), [scan]);
        assert!(granted("notes").is_empty());
        // A grant made inside a component goes to its mod.
        let other = dir.path().join("other.png");
        std::fs::write(&other, "png").unwrap();
        grant("notes/image-viewer#main", &[other]);
        assert_eq!(granted("notes").len(), 1);
        assert_eq!(granted("editor").len(), 1);
    }
}
