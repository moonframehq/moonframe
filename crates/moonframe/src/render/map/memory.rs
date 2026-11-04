//! Decoded tiles in memory, bounded. GPUI keeps every picture it decoded
//! for the whole run unless an image cache lets it go, and a map panned
//! for a while decodes thousands (a high-density tile is a megabyte), so
//! each window's maps draw through a cache of their own that drops the
//! tiles drawn longest ago past [`MOST`], never one drawn just now.

use {
    gpui_kit::*,
    std::{
        cell::RefCell,
        collections::HashMap,
        path::{Path, PathBuf},
        sync::Arc,
        time::{Duration, Instant},
    },
};

/// Decoded tiles kept per window: what a large window's tiles, their
/// ring, stand-ins and base layer take, with room to pan back.
const MOST: usize = 256;
/// A tile drawn this recently is on screen (or about to be): kept.
const RECENT: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Kept {
    cache: Option<Entity<RetainAllImageCache>>,
    /// Drops this window's cache when the window closes.
    closed: Option<Subscription>,
    /// When each tile was last drawn.
    drawn: HashMap<PathBuf, Instant>,
}

thread_local! {
    static WINDOWS: RefCell<HashMap<WindowId, Kept>> = RefCell::new(HashMap::new());
    /// The window being drawn, whose cache tiles count against.
    static CURRENT: RefCell<Option<WindowId>> = const { RefCell::new(None) };
}

/// The window the next tiles are drawn in; `None` (a preview) keeps them
/// in GPUI's own cache.
pub(super) fn begin(window: Option<WindowId>) {
    CURRENT.set(window);
}

/// The picture of the tile in `file`, counted as drawn now.
pub(super) fn picture(file: PathBuf) -> Img {
    if let Some(window) = CURRENT.with_borrow(|current| *current) {
        WINDOWS.with_borrow_mut(|windows| {
            windows
                .entry(window)
                .or_default()
                .drawn
                .insert(file.clone(), Instant::now());
        });
    }
    img(file)
}

/// The decoded tile in `file`, counted as drawn now, for pixels of our own
/// rather than a picture; `None` if it cannot be decoded.
pub(super) fn decoded(file: &Path, window: &mut Window, cx: &mut App) -> Option<Decoded> {
    let id = window.window_handle().window_id();
    WINDOWS.with_borrow_mut(|windows| {
        let drawn = &mut windows.entry(id).or_default().drawn;
        drawn.insert(file.to_owned(), Instant::now());
    });
    let cache = TileCache(id).provide(window, cx);
    match cache.load(&Resource::Path(file.into()), window, cx) {
        None => Some(Decoded::Waiting),
        Some(Ok(image)) => Some(Decoded::Ready(image)),
        Some(Err(_)) => None,
    }
}

/// A tile on its way to pixels.
pub(super) enum Decoded {
    Waiting,
    Ready(Arc<RenderImage>),
}

/// The cache a window's tiles are drawn through, made on first use.
pub(super) struct TileCache(pub(super) WindowId);

impl ImageCacheProvider for TileCache {
    fn provide(&mut self, _: &mut Window, cx: &mut App) -> AnyImageCache {
        WINDOWS.with_borrow_mut(|windows| {
            let kept = windows.entry(self.0).or_default();
            if kept.closed.is_none() {
                let id = self.0;
                kept.closed = Some(cx.on_window_closed(move |_, closed| {
                    if closed == id {
                        let kept = WINDOWS.with_borrow_mut(|windows| windows.remove(&id));
                        if let Some(kept) = kept {
                            super::stand_in::forget(&kept.drawn.into_keys().collect::<Vec<_>>());
                        }
                        super::rotated::forget(id);
                        if CURRENT.with_borrow(|current| *current == Some(id)) {
                            CURRENT.set(None);
                        }
                    }
                }));
            }
            kept.cache
                .get_or_insert_with(|| RetainAllImageCache::new(cx))
                .clone()
                .into()
        })
    }
}

/// Lets go of the tiles drawn longest ago in `window`, past [`MOST`].
pub(super) fn evict(window: &mut Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    let (cache, gone) = WINDOWS.with_borrow_mut(|windows| {
        let Some(kept) = windows.get_mut(&id) else {
            return (None, Vec::new());
        };
        let gone = oldest(&mut kept.drawn, MOST, Instant::now());
        (kept.cache.clone(), gone)
    });
    let Some(cache) = cache else {
        return;
    };
    if gone.is_empty() {
        return;
    }
    super::stand_in::forget(&gone);
    cache.update(cx, |cache, cx| {
        for file in gone {
            let path: Arc<Path> = file.into();
            cache.remove(&Resource::Path(path), window, cx);
        }
    });
}

/// Takes out of `drawn` the files drawn longest ago until `most` are
/// left, sparing those drawn within [`RECENT`] of `now`.
fn oldest(drawn: &mut HashMap<PathBuf, Instant>, most: usize, now: Instant) -> Vec<PathBuf> {
    if drawn.len() <= most {
        return Vec::new();
    }
    let mut by_age: Vec<(PathBuf, Instant)> = drawn
        .iter()
        .filter(|(_, at)| now.duration_since(**at) > RECENT)
        .map(|(file, at)| (file.clone(), *at))
        .collect();
    by_age.sort_by_key(|(_, at)| *at);
    let over = drawn.len() - most;
    let gone: Vec<PathBuf> = by_age
        .into_iter()
        .take(over)
        .map(|(file, _)| file)
        .collect();
    for file in &gone {
        drawn.remove(file);
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::{HashMap, Instant, PathBuf, RECENT, oldest};

    #[test]
    fn eviction_takes_the_oldest_and_spares_what_was_just_drawn() {
        let now = Instant::now() + RECENT * 10;
        let mut drawn: HashMap<PathBuf, Instant> = (0..6u32)
            .map(|index| {
                let at = now - RECENT * (2 + 6 - index);
                (PathBuf::from(format!("{index}.png")), at)
            })
            .collect();
        // Two just drawn: never let go, even over the bound.
        drawn.insert(PathBuf::from("a.png"), now);
        drawn.insert(PathBuf::from("b.png"), now);
        let mut gone = oldest(&mut drawn, 3, now);
        gone.sort();
        let names: Vec<String> = gone.iter().map(|file| file.display().to_string()).collect();
        assert_eq!(names, ["0.png", "1.png", "2.png", "3.png", "4.png"]);
        assert!(drawn.contains_key(&PathBuf::from("a.png")));
        assert!(drawn.contains_key(&PathBuf::from("5.png")));
        // Under the bound, nothing goes.
        assert!(oldest(&mut drawn, 3, now).is_empty());
    }
}
