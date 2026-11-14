//! Where each `ui.image` file is, worked out once rather than every frame:
//! resolving one canonicalizes it and every allowed folder, which is
//! several filesystem calls per image per frame while anything animates.

use {
    moonframe_core::{Image, MAX_IMAGE_BYTES, resolve_file},
    std::{
        cell::RefCell,
        collections::{HashMap, HashSet},
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
    /// show that mod, which its `ui.image` (and its components') may. Kept
    /// for the whole run, however many: a picture opened stays shown.
    static GRANTED: RefCell<HashMap<String, Grants>> = RefCell::new(HashMap::new());
}

/// What one mod was given: files, looked up at once, and folders, whose
/// files it may show too.
#[derive(Default)]
struct Grants {
    files: HashSet<PathBuf>,
    folders: Vec<PathBuf>,
}

/// The mod a drawing mod's grants belong to: a component instance
/// (`<mod>/<component>#<id>`) shares its mod's.
fn grantee(owner: &str) -> &str {
    owner.split('/').next().unwrap_or(owner)
}

/// Lets the mod `owner` show `paths` in `ui.image` (and what is in those
/// that are folders): the user picked them for it or dropped them on it.
pub(crate) fn grant(owner: &str, paths: &[PathBuf]) {
    let owner = grantee(owner).to_owned();
    GRANTED.with_borrow_mut(|granted| {
        let grants = granted.entry(owner).or_default();
        for path in paths {
            let Ok(path) = path.canonicalize() else {
                continue;
            };
            if path.is_dir() {
                if !grants.folders.contains(&path) {
                    grants.folders.push(path);
                }
            } else {
                grants.files.insert(path);
            }
        }
    });
    // Pictures looked for before they were granted are cached as missing;
    // what was found stays found.
    CACHE.with_borrow_mut(|cache| {
        cache.retain(|_, resolved| matches!(resolved, Resolved::Found(_)));
    });
}

/// Whether the mod `owner` was given `real`, a canonical file path.
/// Whether the user gave the mod `owner` the file at `real` (canonical):
/// picked or dropped it, or a folder holding it.
pub(crate) fn is_granted(owner: &str, real: &Path) -> bool {
    granted(owner, real)
}

fn granted(owner: &str, real: &Path) -> bool {
    GRANTED.with_borrow(|granted| {
        granted.get(grantee(owner)).is_some_and(|grants| {
            grants.files.contains(real)
                || grants.folders.iter().any(|folder| real.starts_with(folder))
        })
    })
}

/// The picture `src` names for a mod in `folder`, if the mod `owner` was
/// given it: a regular file of at most [`MAX_IMAGE_BYTES`], named as
/// `ui.image` names pictures.
fn granted_file(owner: &str, src: &str, folder: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let src = Path::new(src);
    let path = match (home, src.strip_prefix("~")) {
        (Some(home), Ok(rest)) => home.join(rest),
        _ if src.is_absolute() => src.to_owned(),
        _ => folder.join(src),
    };
    let real = path.canonicalize().ok()?;
    let metadata = real.metadata().ok()?;
    (metadata.is_file() && metadata.len() <= MAX_IMAGE_BYTES && granted(owner, &real))
        .then_some(real)
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
    resolve_src(&image.src, owner, folder, allowed)
}

/// [`resolve`] for a picture named by its address alone, as a markdown
/// document names one.
pub(super) fn resolve_src(
    src: &str,
    owner: &str,
    folder: &Path,
    allowed: impl FnOnce() -> Vec<PathBuf>,
) -> Option<PathBuf> {
    let key = (owner.to_owned(), folder.to_owned(), src.to_owned());
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
    let path = resolve_file(src, folder, home.as_deref(), &allowed(), MAX_IMAGE_BYTES)
        .or_else(|| granted_file(owner, src, folder, home.as_deref()));
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
        let src = scan.to_str().unwrap();
        let shown = |owner: &str| granted_file(owner, src, dir.path(), None).is_some();
        assert!(shown("editor"));
        // The editor's image viewer shares it; another mod does not.
        assert!(shown("editor/image-viewer#main"));
        assert!(!shown("notes"));
        // A grant made inside a component goes to its mod.
        let other = dir.path().join("other.png");
        std::fs::write(&other, "png").unwrap();
        grant("notes/image-viewer#main", std::slice::from_ref(&other));
        let other_src = other.to_str().unwrap();
        assert!(granted_file("notes", other_src, dir.path(), None).is_some());
        assert!(granted_file("editor", other_src, dir.path(), None).is_none());
    }

    #[test]
    fn grants_last_however_many_files_are_opened() {
        let dir = tempfile::tempdir().unwrap();
        let files: Vec<PathBuf> = (0..300)
            .map(|index| {
                let path = dir.path().join(format!("{index}.png"));
                std::fs::write(&path, "png").unwrap();
                path
            })
            .collect();
        grant("batch", &files);
        // The first opened is still shown after 300.
        let first = files[0].to_str().unwrap();
        assert!(granted_file("batch", first, dir.path(), None).is_some());
        // A folder grants what is in it.
        let folder = tempfile::tempdir().unwrap();
        let inside = folder.path().join("in.png");
        std::fs::write(&inside, "png").unwrap();
        grant("folders", &[folder.path().to_owned()]);
        assert!(granted_file("folders", inside.to_str().unwrap(), folder.path(), None).is_some());
    }
}
