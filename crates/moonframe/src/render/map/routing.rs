//! Routes along streets for `ui.map` routes given as `via` stops: asked
//! of FOSSGIS's public OSRM servers (`routing.openstreetmap.de`), which
//! route on foot, by bike and by car on OpenStreetMap data with no key,
//! under a fair-use policy. One request at a time, newest first; each
//! answer is kept for the run, so a route asks once.

use {
    moonframe_core::{Route, TravelMode},
    serde_json::Value,
    std::{
        collections::{HashMap, VecDeque},
        io::Read as _,
        sync::{Arc, Condvar, LazyLock, Mutex},
        time::{Duration, Instant},
    },
};

/// How long a route that failed is left before it is asked for again.
const RETRY: Duration = Duration::from_secs(30);
/// Answers kept; past this the memory starts over.
const MOST_KEPT: usize = 512;
/// The largest answer read: a long route's geometry is tens of kilobytes.
const MOST_BYTES: u64 = 8 * 1024 * 1024;
/// Requests waiting beyond this drop the oldest.
const MOST_WAITING: usize = 64;

/// What a route is asked by: how it travels and its stops, to the
/// micro-degree, so a float's last bits do not ask again.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Ask {
    mode: TravelMode,
    stops: Vec<[i64; 2]>,
}

impl Ask {
    pub(super) fn of(route: &Route) -> Option<Self> {
        if route.via.len() < 2 {
            return None;
        }
        let micro = |degrees: f64| (degrees * 1e6).round() as i64;
        Some(Self {
            mode: route.mode,
            stops: route
                .via
                .iter()
                .map(|&[lat, lon]| [micro(lat), micro(lon)])
                .collect(),
        })
    }

    fn url(&self) -> String {
        let profile = match self.mode {
            TravelMode::Walk => "foot",
            TravelMode::Bike => "bike",
            TravelMode::Car => "car",
        };
        let stops: Vec<String> = self
            .stops
            .iter()
            .map(|&[lat, lon]| format!("{:.6},{:.6}", lon as f64 / 1e6, lat as f64 / 1e6))
            .collect();
        format!(
            "https://routing.openstreetmap.de/routed-{profile}/route/v1/driving/{}?overview=full&geometries=geojson",
            stops.join(";")
        )
    }
}

/// A route's way along streets.
#[derive(Debug, PartialEq)]
pub(super) struct Way {
    /// `[lat, lon]` corners in order.
    pub(super) points: Vec<[f64; 2]>,
    pub(super) seconds: f64,
    pub(super) meters: f64,
}

/// What is known of a route this run.
#[derive(Clone)]
pub(super) enum Routed {
    Waiting,
    Found(Arc<Way>),
    Failed(Instant),
}

#[derive(Default)]
struct Shared {
    routes: HashMap<Ask, Routed>,
    queue: VecDeque<Ask>,
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

/// The way for `ask` if it is known, asking for it (in the background)
/// when it is not.
pub(super) fn way(ask: &Ask) -> Routed {
    let mut shared = shared();
    match shared.routes.get(ask) {
        Some(Routed::Failed(at)) if at.elapsed() >= RETRY => {},
        Some(known) => return known.clone(),
        None => {},
    }
    if shared.routes.len() > MOST_KEPT {
        shared.routes.clear();
    }
    shared.routes.insert(ask.clone(), Routed::Waiting);
    shared.queue.push_back(ask.clone());
    while shared.queue.len() > MOST_WAITING {
        if let Some(dropped) = shared.queue.pop_front() {
            shared.routes.remove(&dropped);
        }
    }
    if !shared.started {
        shared.started = true;
        let spawned = std::thread::Builder::new()
            .name("map-routes".to_owned())
            .spawn(work);
        if let Err(error) = spawned {
            tracing::warn!("map routes: no worker: {error}");
        }
    }
    SHARED.1.notify_one();
    Routed::Waiting
}

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
        let ask = {
            let mut shared = shared();
            loop {
                if let Some(ask) = shared.queue.pop_back() {
                    break ask;
                }
                shared = SHARED
                    .1
                    .wait(shared)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        };
        let routed = match fetch(&agent, &ask) {
            Ok(way) => Routed::Found(Arc::new(way)),
            Err(problem) => {
                tracing::debug!("map route {ask:?}: {problem}");
                Routed::Failed(Instant::now())
            },
        };
        shared().routes.insert(ask, routed);
        super::tiles::wake();
    }
}

fn fetch(agent: &ureq::Agent, ask: &Ask) -> Result<Way, String> {
    let mut response = agent
        .get(&ask.url())
        .call()
        .map_err(|error| format!("the router is not reachable: {error}"))?;
    let status = response.status().as_u16();
    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .take(MOST_BYTES)
        .read_to_string(&mut body)
        .map_err(|error| error.to_string())?;
    if status != 200 {
        return Err(format!("the router failed ({status})"));
    }
    let answer: Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    way_of(&answer)
}

/// The first route of an OSRM answer.
fn way_of(answer: &Value) -> Result<Way, String> {
    if answer["code"] != "Ok" {
        return Err(format!("the router says {}", answer["code"]));
    }
    let route = &answer["routes"][0];
    let points = route["geometry"]["coordinates"]
        .as_array()
        .ok_or("no geometry")?
        .iter()
        .filter_map(|corner| Some([corner[1].as_f64()?, corner[0].as_f64()?]))
        .collect::<Vec<_>>();
    if points.len() < 2 {
        return Err("no way between the stops".to_owned());
    }
    Ok(Way {
        points,
        seconds: route["duration"].as_f64().unwrap_or_default(),
        meters: route["distance"].as_f64().unwrap_or_default(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn an_ask_names_its_profile_and_stops_lon_first() {
        let ask = Ask {
            mode: TravelMode::Walk,
            stops: vec![[37_788_000, -122_404_000], [37_786_500, -122_399_500]],
        };
        assert_eq!(
            ask.url(),
            "https://routing.openstreetmap.de/routed-foot/route/v1/driving/\
             -122.404000,37.788000;-122.399500,37.786500?overview=full&geometries=geojson"
        );
    }

    #[test]
    fn an_answer_gives_its_way_lat_first() {
        let answer = serde_json::json!({
            "code": "Ok",
            "routes": [{
                "duration": 480.5,
                "distance": 612.0,
                "geometry": { "coordinates": [[-122.404, 37.788], [-122.4, 37.7865]] },
            }],
        });
        let way = way_of(&answer).unwrap();
        assert_eq!(way.points, vec![[37.788, -122.404], [37.7865, -122.4]]);
        assert_eq!((way.seconds, way.meters), (480.5, 612.0));
        assert!(way_of(&serde_json::json!({ "code": "NoRoute" })).is_err());
    }
}
