//! `ui.map`: tiles from the provider a map names, with its pins and routes.
//! Tile servers ask for a client that identifies itself, keeps what it fetched
//! a week at least, and holds few connections, so a few workers fetch and a
//! tile stays on disk. It has GPUI's pan and zoom, not its turning, gliding or
//! routing: a routed route is drawn straight through its stops.

use {
    crate::Ui,
    moonframe_core::{Map, Markdown, unworld, world},
    rune::{Drag, Image, ImageCache, Phase, Tx, Waker, WheelEvent},
    std::{
        cell::RefCell,
        collections::{HashMap, HashSet},
        hash::{DefaultHasher, Hash as _, Hasher as _},
        io::Read as _,
        path::PathBuf,
        sync::{
            Arc, Mutex,
            mpsc::{Sender, channel},
        },
        time::{Duration, SystemTime},
    },
};

const WORKERS: usize = 2;
/// How long a tile on disk is used before it is fetched again.
const FRESH: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// The largest tile read; a real one is tens of kilobytes.
const MOST_BYTES: u64 = 4 * 1024 * 1024;
const TILE: f64 = 256.;
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

/// What the workers fetched, and what is asked of them.
#[derive(Default)]
pub(crate) struct Tiles {
    asked: RefCell<HashSet<String>>,
    ready: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    loaded: RefCell<HashMap<String, Image>>,
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
                        if let Some(bytes) = fetch(&url)
                            && let Ok(mut done) = ready.lock()
                        {
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
        if let Some(image) = self.loaded.borrow().get(url) {
            return Some(image.clone());
        }
        if self.asked.borrow_mut().insert(url.to_owned()) {
            let mut work = self.work.borrow_mut();
            let sender = work.get_or_insert_with(|| self.start(wake));
            if sender.send(url.to_owned()).is_err() {
                tracing::warn!("no worker for {url}");
            }
        }
        None
    }

    /// Decodes what the workers brought; true when any arrived.
    pub(crate) fn take_ready(&self, cache: &ImageCache) -> bool {
        let Ok(mut ready) = self.ready.lock() else {
            return false;
        };
        let arrived = !ready.is_empty();
        for (url, bytes) in ready.drain(..) {
            self.loaded.borrow_mut().insert(url, cache.load(bytes));
        }
        arrived
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
    /// A drag over the map `key`, which started from `start`.
    pub(crate) fn pan_map(&mut self, key: &str, start: Camera, drag: &Drag) {
        let world_size = TILE * 2f64.powf(start.zoom);
        let (x, y) = world(start.center[0], start.center[1]);
        let center = unworld(
            x - f64::from(drag.dx) / world_size,
            y - f64::from(drag.dy) / world_size,
        );
        self.map_tiles
            .cameras
            .borrow_mut()
            .insert(key.to_owned(), Camera {
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

    /// Tells a mod where the user left its map.
    pub(crate) fn map_moved(
        &mut self,
        key: &str,
        owner: &str,
        to: &str,
        msg: &moonframe_core::Data,
        _: &mut Tx,
    ) {
        let camera = self.map_tiles.cameras.borrow().get(key).copied();
        if let (Some(camera), true) = (camera, matches!(msg, moonframe_core::Data::Map(_))) {
            let note = [
                ("lat", camera.center[0].into()),
                ("lon", camera.center[1].into()),
                ("zoom", camera.zoom.into()),
                ("bearing", 0f64.into()),
            ];
            self.send(owner, to, Markdown::note(msg, note));
        }
    }

    /// Whether a map's drag ended.
    pub(crate) fn drag_ended(drag: &Drag) -> bool {
        matches!(drag.phase, Phase::Up)
    }
}
