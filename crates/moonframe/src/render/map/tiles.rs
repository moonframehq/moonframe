//! Map tiles: fetched in the background from the provider a `ui.map`
//! names, kept on disk under the cache folder, and shown from there.
//!
//! Tile servers ask for a client that identifies itself, keeps what it
//! fetched (a week at least, by OpenStreetMap's policy) and holds few
//! connections open, so two workers fetch, newest request first, and a
//! tile that failed is not asked for again for a while. A tile past its
//! week still shows while the server is asked whether it changed; a
//! request no map has wanted for a while is dropped unfetched.

use {
    moonframe_core::MapProvider,
    std::{
        collections::{HashMap, VecDeque},
        io::Read as _,
        path::PathBuf,
        sync::{Condvar, LazyLock, Mutex},
        time::{Duration, Instant, SystemTime},
    },
};

/// Connections open to tile servers at once, across every map.
const WORKERS: usize = 2;
/// Requests waiting beyond this drop the oldest: the map moved on.
const MOST_WAITING: usize = 256;
/// How long a tile on disk is used before it is fetched again (and shown
/// meanwhile).
const FRESH: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// How long a tile that failed is left before it is asked for again.
const RETRY: Duration = Duration::from_secs(30);
/// The largest tile read; a real one is tens of kilobytes.
const MOST_BYTES: u64 = 4 * 1024 * 1024;
/// A waiting request no map asked for again within this is dropped: the
/// map moved on. Visible tiles are asked for on every frame.
const UNWANTED: Duration = Duration::from_secs(3);

/// One tile of one provider's map.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Tile {
    pub(super) provider: MapProvider,
    /// The `@2x` tile, for a high-density screen.
    pub(super) retina: bool,
    pub(super) z: u8,
    pub(super) x: u32,
    pub(super) y: u32,
}

impl Tile {
    /// Where it is kept: by provider and density, never by key, so a key
    /// stays out of file names.
    fn file(&self) -> PathBuf {
        let density = if self.retina {
            "@2x"
        } else {
            ""
        };
        super::disk::folder()
            .join(format!("{}{density}", self.provider.name()))
            .join(self.z.to_string())
            .join(self.x.to_string())
            .join(format!("{}.png", self.y))
    }

    fn url(&self, key: Option<&str>) -> String {
        let source = self.provider.source();
        let retina = if self.retina {
            "@2x"
        } else {
            ""
        };
        source
            .url
            .replace("{z}", &self.z.to_string())
            .replace("{x}", &self.x.to_string())
            .replace("{y}", &self.y.to_string())
            .replace("{r}", retina)
            .replace("{key}", key.unwrap_or_default())
    }
}

/// Whether the provider has `@2x` tiles.
pub(super) fn has_retina(provider: MapProvider) -> bool {
    provider.source().url.contains("{r}")
}

/// What is known of a tile this run.
enum State {
    /// Available as a stand-in, but not yet checked for freshness.
    Unchecked(PathBuf),
    Waiting,
    Ready(PathBuf),
    Failed(Instant),
}

#[derive(Default)]
struct Shared {
    tiles: HashMap<Tile, State>,
    /// When a map last asked for each waiting tile.
    wanted: HashMap<Tile, Instant>,
    /// Newest last; workers take from the back. With the tile, its key and
    /// the file it already has, past its week, to revalidate.
    queue: VecDeque<(Tile, Option<String>, Option<SystemTime>)>,
    /// What to wake once a tile arrives.
    waiters: Vec<futures::channel::oneshot::Sender<()>>,
    /// The last failure, by provider: a wrong key says so on the map.
    failures: HashMap<MapProvider, String>,
    started: bool,
}

static SHARED: LazyLock<(Mutex<Shared>, Condvar)> =
    LazyLock::new(|| (Mutex::new(Shared::default()), Condvar::new()));

fn shared() -> std::sync::MutexGuard<'static, Shared> {
    SHARED
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The file to show for `tile`, if there is one yet; asks for it (in the
/// background) when it is missing or old. `key` is the provider's API key.
pub(super) fn tile(tile: &Tile, key: Option<&str>) -> Option<PathBuf> {
    tile_in(&mut shared(), tile, key, || tile.file())
}

fn tile_in(
    shared: &mut Shared,
    tile: &Tile,
    key: Option<&str>,
    file: impl FnOnce() -> PathBuf,
) -> Option<PathBuf> {
    match shared.tiles.get(tile) {
        Some(State::Ready(path)) => return Some(path.clone()),
        Some(State::Waiting) => {
            shared.wanted.insert(tile.clone(), Instant::now());
            return None;
        },
        Some(State::Failed(at)) if at.elapsed() < RETRY => return None,
        _ => {},
    }
    let file = file();
    let modified = std::fs::metadata(&file)
        .and_then(|metadata| metadata.modified())
        .ok();
    let age = modified.map(|modified| {
        SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default()
    });
    match age {
        Some(age) if age < FRESH => {
            shared
                .tiles
                .insert(tile.clone(), State::Ready(file.clone()));
            Some(file)
        },
        // Old: shown while the server says whether it changed.
        Some(_) => {
            ask(shared, tile, key, modified);
            shared
                .tiles
                .insert(tile.clone(), State::Ready(file.clone()));
            Some(file)
        },
        None => {
            ask(shared, tile, key, None);
            None
        },
    }
}

/// The file for `tile` if it is already at hand, in memory or on disk at
/// any age, without asking for it: what a coarser zoom can show while
/// finer tiles arrive.
pub(super) fn ready(tile: &Tile) -> Option<PathBuf> {
    ready_in(&mut shared(), tile, || tile.file())
}

fn ready_in(shared: &mut Shared, tile: &Tile, file: impl FnOnce() -> PathBuf) -> Option<PathBuf> {
    match shared.tiles.get(tile) {
        Some(State::Ready(path) | State::Unchecked(path)) => return Some(path.clone()),
        Some(State::Waiting | State::Failed(_)) => return None,
        None => {},
    }
    let file = file();
    if !file.is_file() {
        return None;
    }
    // Availability is not freshness: a stand-in may be years old. Leave
    // it unchecked so a later `tile` call still revalidates it, without
    // statting the same stand-in on every animation frame meanwhile.
    shared
        .tiles
        .insert(tile.clone(), State::Unchecked(file.clone()));
    Some(file)
}

/// Asks for `tile` ahead of need, unless it is at hand or on its way:
/// the ring around what shows, so panning finds it ready.
pub(super) fn prefetch(tile: &Tile, key: Option<&str>) {
    let _ = self::tile(tile, key);
}

/// What went wrong fetching `provider`'s tiles last, if anything.
pub(super) fn failure(provider: MapProvider) -> Option<String> {
    shared().failures.get(&provider).cloned()
}

/// Wakes every map waiting for something to arrive (a tile, a route),
/// to draw again.
pub(super) fn wake() {
    for waiter in shared().waiters.drain(..) {
        let _ = waiter.send(());
    }
}

/// Resolves once a tile arrives (or fails), to draw again.
pub(super) fn arrival() -> futures::channel::oneshot::Receiver<()> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    shared().waiters.push(sender);
    receiver
}

fn ask(shared: &mut Shared, tile: &Tile, key: Option<&str>, since: Option<SystemTime>) {
    shared.tiles.insert(tile.clone(), State::Waiting);
    shared.wanted.insert(tile.clone(), Instant::now());
    shared
        .queue
        .push_back((tile.clone(), key.map(str::to_owned), since));
    while shared.queue.len() > MOST_WAITING {
        if let Some((dropped, ..)) = shared.queue.pop_front() {
            forget(shared, &dropped);
        }
    }
    if !shared.started {
        shared.started = true;
        // Once a run, the cache is brought under its size.
        let pruning = std::thread::Builder::new()
            .name("map-tiles-prune".to_owned())
            .spawn(|| super::disk::prune(&super::disk::folder()));
        if let Err(error) = pruning {
            tracing::warn!("map tiles: no pruning: {error}");
        }
        for index in 0..WORKERS {
            let spawned = std::thread::Builder::new()
                .name(format!("map-tiles-{index}"))
                .spawn(work);
            if let Err(error) = spawned {
                tracing::warn!("map tiles: no worker: {error}");
            }
        }
    }
    SHARED.1.notify_one();
}

/// Drops a request: a map that still wants the tile asks again.
fn forget(shared: &mut Shared, tile: &Tile) {
    shared.wanted.remove(tile);
    // A tile past its week that is still shown stays shown.
    if matches!(shared.tiles.get(tile), Some(State::Waiting)) {
        shared.tiles.remove(tile);
    }
}

/// A worker: takes the newest request, fetches it, and says it arrived.
fn work() {
    let agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(concat!(
            "Moonframe/",
            env!("CARGO_PKG_VERSION"),
            " (+https://moonframe.dev)"
        ))
        .build()
        .new_agent();
    loop {
        let (tile, key, since) = {
            let mut shared = shared();
            loop {
                match shared.queue.pop_back() {
                    // Nobody asked for it lately: the map moved on. Drawing
                    // again lets a map that still shows it ask anew.
                    Some((tile, ..))
                        if shared
                            .wanted
                            .get(&tile)
                            .is_none_or(|at| at.elapsed() > UNWANTED) =>
                    {
                        forget(&mut shared, &tile);
                        for waiter in shared.waiters.drain(..) {
                            let _ = waiter.send(());
                        }
                    },
                    Some(job) => break job,
                    None => {
                        shared = SHARED
                            .1
                            .wait(shared)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                    },
                }
            }
        };
        let fetched = fetch(&agent, &tile, key.as_deref(), since);
        let mut shared = shared();
        shared.wanted.remove(&tile);
        match fetched {
            Ok(path) => {
                shared.failures.remove(&tile.provider);
                shared.tiles.insert(tile, State::Ready(path));
            },
            Err(problem) => {
                tracing::debug!("map tile {tile:?}: {problem}");
                // An old copy on disk still shows.
                let file = tile.file();
                let state = if file.is_file() {
                    State::Ready(file)
                } else {
                    State::Failed(Instant::now())
                };
                shared.failures.insert(tile.provider, problem);
                shared.tiles.insert(tile, state);
            },
        }
        for waiter in shared.waiters.drain(..) {
            let _ = waiter.send(());
        }
    }
}

/// Downloads `tile` to its file, whole or not at all. With `since`, the
/// date of the copy on disk, an unchanged tile costs the server nothing:
/// it answers 304, and the copy counts as fetched now.
fn fetch(
    agent: &ureq::Agent,
    tile: &Tile,
    key: Option<&str>,
    since: Option<SystemTime>,
) -> Result<PathBuf, String> {
    let mut request = agent.get(&tile.url(key));
    if let Some(since) = since {
        request = request.header("If-Modified-Since", super::disk::http_date(since));
    }
    let mut response = request
        .call()
        .map_err(|error| format!("{} is not reachable: {error}", tile.provider.name()))?;
    let status = response.status().as_u16();
    if status == 304 {
        let file = tile.file();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .and_then(|open| open.set_modified(SystemTime::now()))
            .map_err(|error| error.to_string())?;
        return Ok(file);
    }
    if status != 200 {
        let why = match status {
            401 | 403 => "refused the key",
            404 => "has no such tile",
            429 => "asks to slow down",
            _ => "failed",
        };
        return Err(format!("{} {why} ({status})", tile.provider.name()));
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MOST_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if imagesize::blob_size(&bytes).is_err() {
        return Err(format!(
            "{} sent something that is not a picture",
            tile.provider.name()
        ));
    }
    let file = tile.file();
    let folder = file.parent().ok_or("no folder")?;
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(folder).map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut temporary, &bytes).map_err(|error| error.to_string())?;
    temporary
        .persist(&file)
        .map_err(|error| error.to_string())?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_fills_in_its_address_and_keeps_its_key_out_of_its_file() {
        let tile = Tile {
            provider: MapProvider::MaptilerStreets,
            retina: true,
            z: 14,
            x: 2620,
            y: 6332,
        };
        assert_eq!(
            tile.url(Some("abc")),
            "https://api.maptiler.com/maps/streets-v2/256/14/2620/6332@2x.png?key=abc"
        );
        let file = tile.file();
        assert!(file.ends_with("tiles/maptiler-streets@2x/14/2620/6332.png"));
        assert!(!file.to_string_lossy().contains("abc"));
        let osm = Tile {
            provider: MapProvider::OpenStreetMap,
            retina: false,
            ..tile
        };
        assert_eq!(
            osm.url(None),
            "https://tile.openstreetmap.org/14/2620/6332.png"
        );
        assert!(!has_retina(MapProvider::OpenStreetMap));
        assert!(has_retina(MapProvider::CartoLight));
    }

    #[test]
    fn an_old_stand_in_is_still_revalidated_when_requested()
    -> Result<(), Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let file = folder.path().join("tile.png");
        std::fs::write(&file, b"cached tile")?;
        let old = SystemTime::now() - FRESH * 2;
        std::fs::File::options()
            .write(true)
            .open(&file)?
            .set_modified(old)?;
        let modified = std::fs::metadata(&file)?.modified()?;
        let tile = Tile {
            provider: MapProvider::OpenStreetMap,
            retina: false,
            z: 1,
            x: 0,
            y: 0,
        };
        // Inspect the queue without starting workers or reaching the network.
        let mut shared = Shared {
            started: true,
            ..Shared::default()
        };
        assert_eq!(
            ready_in(&mut shared, &tile, || file.clone()),
            Some(file.clone())
        );
        assert_eq!(
            tile_in(&mut shared, &tile, None, || file.clone()),
            Some(file.clone())
        );
        assert_eq!(
            shared.queue.front(),
            Some(&(tile.clone(), None, Some(modified)))
        );
        // The stale copy stays visible; drawing again must not queue a duplicate.
        assert_eq!(
            tile_in(&mut shared, &tile, None, || file.clone()),
            Some(file)
        );
        assert_eq!(shared.queue.len(), 1);
        Ok(())
    }
}
