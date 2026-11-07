//! `ui.map`: tiles from the provider a map names, with its pins and routes.
//! Tile servers ask for a client that identifies itself, keeps what it fetched
//! a week at least, and holds few connections, so a few workers fetch and a
//! tile stays on disk. It has GPUI's pan and zoom, not its turning, gliding or
//! routing: a routed route is drawn straight through its stops.

use {
    crate::Ui,
    moonframe_core::{Data, Map, Markdown, unworld, world},
    rune::{Drag, Image, ImageCache, Phase, Tx, Waker, WheelEvent},
    std::{
        cell::{Cell, RefCell},
        collections::{HashMap, HashSet},
        hash::{DefaultHasher, Hash as _, Hasher as _},
        io::Read as _,
        path::PathBuf,
        sync::{
            Arc, Mutex,
            mpsc::{Sender, channel},
        },
        time::{Duration, Instant, SystemTime},
    },
};

const WORKERS: usize = 2;
/// How long a tile on disk is used before it is fetched again.
const FRESH: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// The largest tile read; a real one is tens of kilobytes.
const MOST_BYTES: u64 = 4 * 1024 * 1024;
const TILE: f64 = 256.;
/// How long a tile that failed is left before it is asked for again.
const RETRY: Duration = Duration::from_secs(30);
/// The decoded tiles kept: about four screens. Past it the longest unused go,
/// and the disk brings them back.
const MOST_LOADED: usize = 256;
/// How often a moving map tells its mod where it is.
const TELL_EVERY: Duration = Duration::from_millis(100);
const AGENT: &str = concat!(
    "moonframe/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/moonframehq/moonframe)"
);

/// Where the user has moved a map to, over what its mod gave.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Camera {
    pub(crate) center: [f64; 2],
    pub(crate) zoom: f64,
}

/// A mod's own say on where its map looks.
type Supplied = (Option<[f64; 2]>, Option<f32>);

/// Who hears a map move.
#[derive(Clone)]
pub(crate) struct Hearer {
    pub(crate) owner: String,
    pub(crate) to: String,
    pub(crate) msg: Data,
}

struct Telling {
    told: Option<Instant>,
    waiting: Option<Hearer>,
}

/// What the workers fetched, and what is asked of them.
#[derive(Default)]
pub(crate) struct Tiles {
    asked: RefCell<HashSet<String>>,
    /// What the workers finished: a tile's bytes, or nothing if it failed.
    ready: Arc<Mutex<Vec<(String, Option<Vec<u8>>)>>>,
    /// Each decoded tile, and when it was last used, as `clock` counts.
    loaded: RefCell<HashMap<String, (Image, u64)>>,
    clock: Cell<u64>,
    /// When each failed tile may be asked for again.
    failed: RefCell<HashMap<String, Instant>>,
    /// The `center` and `zoom` each map's mod last gave: a change takes over
    /// from where the user moved it.
    supplied: RefCell<HashMap<String, Supplied>>,
    /// Where each map being dragged looked as the press began.
    dragged_from: RefCell<HashMap<String, Camera>>,
    /// Maps that moved since their mod was last told, and when that was.
    telling: RefCell<HashMap<String, Telling>>,
    work: RefCell<Option<Sender<String>>>,
    pub(crate) cameras: RefCell<HashMap<String, Camera>>,
    /// Each map's box in points, as last laid out.
    pub(crate) boxes: RefCell<HashMap<String, (f32, f32)>>,
}

fn cached(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    std::env::temp_dir()
        .join("moonframe-rune-tiles")
        .join(format!("{:016x}.tile", hasher.finish()))
}

fn fetch(url: &str) -> Option<Vec<u8>> {
    let file = cached(url);
    let fresh = std::fs::metadata(&file)
        .and_then(|found| found.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < FRESH);
    if fresh && let Ok(bytes) = std::fs::read(&file) {
        return Some(bytes);
    }
    let mut response = ureq::get(url).header("User-Agent", AGENT).call().ok()?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MOST_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    if let Some(folder) = file.parent()
        && std::fs::create_dir_all(folder).is_ok()
        && let Err(problem) = std::fs::write(&file, &bytes)
    {
        tracing::debug!("cannot keep a tile: {problem}");
    }
    Some(bytes)
}

impl Tiles {
    fn start(&self, wake: &Waker) -> Sender<String> {
        let (sender, receiver) = channel::<String>();
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..WORKERS {
            let (receiver, ready, wake) = (
                Arc::clone(&receiver),
                Arc::clone(&self.ready),
                Arc::clone(wake),
            );
            let started = std::thread::Builder::new()
                .name("rune-tiles".into())
                .spawn(move || {
                    loop {
                        let next = receiver.lock().ok().and_then(|jobs| jobs.recv().ok());
                        let Some(url) = next else {
                            return;
                        };
                        let bytes = fetch(&url);
                        if let Ok(mut done) = ready.lock() {
                            done.push((url, bytes));
                            wake();
                        }
                    }
                });
            if let Err(problem) = started {
                tracing::warn!("cannot fetch tiles: {problem}");
            }
        }
        sender
    }

    /// Whether any tile has been asked for.
    pub(crate) fn fetching(&self) -> bool {
        !self.asked.borrow().is_empty()
    }

    /// The tile at `url`, asked for if it hasn't been.
    pub(crate) fn tile(&self, url: &str, wake: &Waker) -> Option<Image> {
        if let Some((image, used)) = self.loaded.borrow_mut().get_mut(url) {
            *used = self.tick();
            return Some(image.clone());
        }
        if self.may_ask(url, Instant::now()) && self.asked.borrow_mut().insert(url.to_owned()) {
            let mut work = self.work.borrow_mut();
            let sender = work.get_or_insert_with(|| self.start(wake));
            if sender.send(url.to_owned()).is_err() {
                tracing::warn!("no worker for {url}");
            }
        }
        None
    }

    fn tick(&self) -> u64 {
        self.clock.set(self.clock.get() + 1);
        self.clock.get()
    }

    /// Whether `url` may be asked for at `now`: a failed one is left a while,
    /// then forgotten, so a network that comes back brings it in.
    fn may_ask(&self, url: &str, now: Instant) -> bool {
        let mut failed = self.failed.borrow_mut();
        match failed.get(url) {
            Some(until) if now < *until => false,
            Some(_) => {
                failed.remove(url);
                self.asked.borrow_mut().remove(url);
                true
            },
            None => true,
        }
    }

    fn keep(&self, url: String, image: Image) {
        let used = self.tick();
        let mut loaded = self.loaded.borrow_mut();
        loaded.insert(url, (image, used));
        while loaded.len() > MOST_LOADED {
            let oldest = loaded
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(url, _)| url.clone());
            let Some(oldest) = oldest else {
                break;
            };
            loaded.remove(&oldest);
            self.asked.borrow_mut().remove(&oldest);
        }
    }

    fn fail(&self, url: String, now: Instant) {
        self.failed.borrow_mut().insert(url, now + RETRY);
    }

    /// Decodes what the workers brought; true when any arrived.
    pub(crate) fn take_ready(&self, cache: &ImageCache) -> bool {
        let Ok(mut ready) = self.ready.lock() else {
            return false;
        };
        let arrived = !ready.is_empty();
        for (url, bytes) in ready.drain(..) {
            match bytes {
                Some(bytes) => self.keep(url, cache.load(bytes)),
                None => self.fail(url, Instant::now()),
            }
        }
        arrived
    }

    /// When a failed tile is next due; the view asks for it from then on.
    pub(crate) fn next_retry(&self) -> Option<Instant> {
        self.failed.borrow().values().min().copied()
    }

    /// Notes what `key`'s mod says now. A new say beats where the user moved
    /// the map to, so a mod's recenter and zoom controls keep working.
    pub(crate) fn supplied(&self, key: &str, say: Supplied) {
        let mut supplied = self.supplied.borrow_mut();
        if supplied
            .insert(key.to_owned(), say)
            .is_some_and(|was| was != say)
        {
            self.cameras.borrow_mut().remove(key);
        }
    }
}

/// Where a map looks: what the user moved it to, else what its mod says, else
/// what fits its pins and routes.
pub(crate) fn camera(node: &Map, moved: Option<Camera>, size: (f32, f32)) -> Camera {
    if let Some(moved) = moved {
        return moved;
    }
    let places: Vec<[f64; 2]> = node
        .pins
        .iter()
        .map(|pin| [pin.lat, pin.lon])
        .chain(
            node.routes
                .iter()
                .flat_map(|route| route.places().iter().copied()),
        )
        .collect();
    let fit = framing(&places, size);
    Camera {
        center: node.center.unwrap_or(fit.0),
        zoom: node.zoom.map_or(fit.1, f64::from),
    }
}

/// The middle and zoom that show `places` in a box of `size`.
fn framing(places: &[[f64; 2]], size: (f32, f32)) -> ([f64; 2], f64) {
    let points: Vec<(f64, f64)> = places.iter().map(|at| world(at[0], at[1])).collect();
    let (Some(first), true) = (points.first(), !points.is_empty()) else {
        return ([0., 0.], 2.);
    };
    let (mut left, mut right, mut top, mut bottom) = (first.0, first.0, first.1, first.1);
    for (x, y) in &points {
        left = left.min(*x);
        right = right.max(*x);
        top = top.min(*y);
        bottom = bottom.max(*y);
    }
    let center = unworld((left + right) / 2., (top + bottom) / 2.);
    let across = (right - left).max(bottom - top).max(1e-6);
    let room = f64::from(size.0.min(size.1)).max(1.) * 0.8;
    (center, (room / (across * TILE)).log2().clamp(1., 16.))
}

/// A tile's address, with what its provider's template names.
pub(crate) fn address(template: &str, key: Option<&str>, (z, x, y): (u32, u32, u32)) -> String {
    template
        .replace("{z}", &z.to_string())
        .replace("{x}", &x.to_string())
        .replace("{y}", &y.to_string())
        .replace("{r}", "@2x")
        .replace("{key}", key.unwrap_or_default())
}

impl Ui {
    /// A drag over the map `key`. `start` is where it looks as the view last
    /// drew it, which moves with the drag: the press keeps the one it began from.
    pub(crate) fn pan_map(&mut self, key: &str, start: Camera, drag: &Drag) {
        let tiles = &self.map_tiles;
        if matches!(drag.phase, Phase::Down) {
            tiles
                .dragged_from
                .borrow_mut()
                .insert(key.to_owned(), start);
        }
        let start = tiles
            .dragged_from
            .borrow()
            .get(key)
            .copied()
            .unwrap_or(start);
        let world_size = TILE * 2f64.powf(start.zoom);
        let (x, y) = world(start.center[0], start.center[1]);
        let center = unworld(
            x - f64::from(drag.dx) / world_size,
            y - f64::from(drag.dy) / world_size,
        );
        tiles.cameras.borrow_mut().insert(key.to_owned(), Camera {
            center,
            zoom: start.zoom,
        });
    }

    /// The wheel over the map `key`.
    pub(crate) fn zoom_map(&mut self, key: &str, start: Camera, event: &WheelEvent) {
        let mut cameras = self.map_tiles.cameras.borrow_mut();
        let camera = cameras.entry(key.to_owned()).or_insert(start);
        camera.zoom = (camera.zoom + f64::from(event.dy) / 120.).clamp(1., 19.);
    }

    /// The map `key` moved: tells its mod, ten times a second at most while
    /// it moves. What is held back goes out from `poll`, or at once if `last`.
    pub(crate) fn map_moved_by_user(&mut self, key: &str, hearer: Hearer, last: bool, tx: &mut Tx) {
        let now = Instant::now();
        let mut telling = self.map_tiles.telling.borrow_mut();
        let entry = telling.entry(key.to_owned()).or_insert(Telling {
            told: None,
            waiting: None,
        });
        let due = entry.told.is_none_or(|told| now - told >= TELL_EVERY);
        if !last && !due {
            entry.waiting = Some(hearer);
            tx.wake_after(TELL_EVERY);
            return;
        }
        entry.told = Some(now);
        entry.waiting = None;
        drop(telling);
        self.tell_map(key, &hearer);
    }

    /// Sends what a moving map held back, once it is due.
    pub(crate) fn settle_maps(&mut self, tx: &mut Tx) {
        let now = Instant::now();
        let mut due = Vec::new();
        for (key, entry) in self.map_tiles.telling.borrow_mut().iter_mut() {
            if entry.waiting.is_none() {
                continue;
            }
            if entry.told.is_none_or(|told| now - told >= TELL_EVERY) {
                entry.told = Some(now);
                due.extend(entry.waiting.take().map(|hearer| (key.clone(), hearer)));
            } else {
                tx.wake_after(TELL_EVERY);
            }
        }
        for (key, hearer) in due {
            self.tell_map(&key, &hearer);
        }
    }

    /// Tells a mod where the user left its map.
    fn tell_map(&mut self, key: &str, hearer: &Hearer) {
        let camera = self.map_tiles.cameras.borrow().get(key).copied();
        if let Some(camera) = camera {
            let note = [
                ("lat", camera.center[0].into()),
                ("lon", camera.center[1].into()),
                ("zoom", camera.zoom.into()),
                ("bearing", 0f64.into()),
            ];
            let message = Markdown::note(&hearer.msg, note);
            self.send(&hearer.owner, &hearer.to, message);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn tiles_with(count: usize) -> (Tiles, ImageCache) {
        let tiles = Tiles::default();
        let cache = ImageCache::new(Arc::new(|| {}));
        for index in 0..count {
            tiles.keep(format!("tile-{index}"), cache.rgba(1, 1, [0u8; 4]));
        }
        (tiles, cache)
    }

    #[test]
    fn a_failed_tile_is_asked_for_again_after_a_while() {
        let tiles = Tiles::default();
        let now = Instant::now();
        tiles.asked.borrow_mut().insert("a".to_owned());
        tiles.fail("a".to_owned(), now);
        assert!(!tiles.may_ask("a", now), "not at once");
        assert!(tiles.may_ask("a", now + RETRY));
        assert!(
            !tiles.asked.borrow().contains("a"),
            "forgotten, so it goes out again"
        );
        assert!(tiles.next_retry().is_none());
    }

    #[test]
    fn the_tiles_kept_stay_bounded_and_the_oldest_go() {
        let (tiles, _cache) = tiles_with(MOST_LOADED + 10);
        let loaded = tiles.loaded.borrow();
        assert_eq!(loaded.len(), MOST_LOADED);
        assert!(!loaded.contains_key("tile-0"));
        assert!(loaded.contains_key(&format!("tile-{}", MOST_LOADED + 9)));
    }

    #[test]
    fn a_new_say_from_the_mod_beats_where_the_user_moved_the_map() {
        let tiles = Tiles::default();
        let camera = Camera {
            center: [1., 2.],
            zoom: 3.,
        };
        tiles.supplied("map", (Some([0., 0.]), Some(4.)));
        tiles.cameras.borrow_mut().insert("map".to_owned(), camera);
        tiles.supplied("map", (Some([0., 0.]), Some(4.)));
        assert!(
            tiles.cameras.borrow().contains_key("map"),
            "the same say changes nothing"
        );
        tiles.supplied("map", (Some([5., 5.]), Some(4.)));
        assert!(tiles.cameras.borrow().is_empty());
    }
}
