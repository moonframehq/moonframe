//! `ui.map`: a street map from a tile provider, with numbered pins and
//! routes drawn over it. The host fetches tiles only from the providers
//! listed here, never from an address a view writes, so a map cannot make
//! the app reach anywhere else.

use super::*;

/// What a provider's tiles are: where they come from and what the map
/// must credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileSource {
    /// The tile address, with `{z}`, `{x}` and `{y}`; `{r}` is `@2x` on a
    /// high-density screen and nothing otherwise; `{key}` is the map's key.
    pub url: &'static str,
    /// Shown in the map's corner, as the provider's terms ask.
    pub attribution: &'static str,
    /// Whether the provider needs an API key (`key`).
    pub key: bool,
    /// The deepest zoom it serves.
    pub max_zoom: u8,
}

/// Declares each provider once: its token, and its [`TileSource`].
macro_rules! providers {
    ($($(#[$doc:meta])* $variant:ident = $name:literal {
        url: $url:literal, attribution: $attribution:literal, key: $key:literal, max_zoom: $max:literal $(,)?
    }),+ $(,)?) => {
        tokens! {
            /// Where a `ui.map` gets its tiles.
            MapProvider {
                $($(#[$doc])* $variant = $name),+
            }
        }

        impl MapProvider {
            pub fn source(self) -> TileSource {
                match self {
                    $(Self::$variant => TileSource {
                        url: $url,
                        attribution: $attribution,
                        key: $key,
                        max_zoom: $max,
                    },)+
                }
            }
        }
    };
}

providers! {
    // keep-sorted start
    /// CARTO's Dark Matter: a dark, quiet basemap of OpenStreetMap data.
    CartoDark = "carto-dark" {
        url: "https://basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png",
        attribution: "© OpenStreetMap contributors © CARTO",
        key: false,
        max_zoom: 20,
    },
    /// CARTO's Positron: a light, quiet basemap of OpenStreetMap data.
    CartoLight = "carto-light" {
        url: "https://basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png",
        attribution: "© OpenStreetMap contributors © CARTO",
        key: false,
        max_zoom: 20,
    },
    /// CARTO's Voyager: OpenStreetMap data in soft colors.
    CartoVoyager = "carto-voyager" {
        url: "https://basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}{r}.png",
        attribution: "© OpenStreetMap contributors © CARTO",
        key: false,
        max_zoom: 20,
    },
    /// Esri's World Street Map.
    EsriStreets = "esri-streets" {
        url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}",
        attribution: "Tiles © Esri",
        key: false,
        max_zoom: 19,
    },
    /// Mapbox Streets; needs an access token.
    MapboxStreets = "mapbox-streets" {
        url: "https://api.mapbox.com/styles/v1/mapbox/streets-v12/tiles/256/{z}/{x}/{y}{r}?access_token={key}",
        attribution: "© Mapbox © OpenStreetMap contributors",
        key: true,
        max_zoom: 22,
    },
    /// MapTiler Streets; needs an API key.
    MaptilerStreets = "maptiler-streets" {
        url: "https://api.maptiler.com/maps/streets-v2/256/{z}/{x}/{y}{r}.png?key={key}",
        attribution: "© MapTiler © OpenStreetMap contributors",
        key: true,
        max_zoom: 22,
    },
    /// OpenStreetMap's own tiles: free for light, interactive use under
    /// its tile usage policy.
    OpenStreetMap = "openstreetmap" {
        url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
        attribution: "© OpenStreetMap contributors",
        key: false,
        max_zoom: 19,
    },
    /// Stadia's Alidade Smooth: light and clean; needs an API key.
    StadiaSmooth = "stadia-smooth" {
        url: "https://tiles.stadiamaps.com/tiles/alidade_smooth/{z}/{x}/{y}{r}.png?api_key={key}",
        attribution: "© Stadia Maps © OpenMapTiles © OpenStreetMap contributors",
        key: true,
        max_zoom: 20,
    },
    /// Stadia's Alidade Smooth Dark; needs an API key.
    StadiaSmoothDark = "stadia-smooth-dark" {
        url: "https://tiles.stadiamaps.com/tiles/alidade_smooth_dark/{z}/{x}/{y}{r}.png?api_key={key}",
        attribution: "© Stadia Maps © OpenMapTiles © OpenStreetMap contributors",
        key: true,
        max_zoom: 20,
    },
    /// Thunderforest's Transport map; needs an API key.
    ThunderforestTransport = "thunderforest-transport" {
        url: "https://tile.thunderforest.com/transport/{z}/{x}/{y}{r}.png?apikey={key}",
        attribution: "Maps © Thunderforest, Data © OpenStreetMap contributors",
        key: true,
        max_zoom: 22,
    },
    // keep-sorted end
}

/// A street map. Unset, `center` and `zoom` frame the pins and routes;
/// once the user drags or scrolls it, the map keeps where they left it
/// until the view gives another `center` or `zoom`.
#[derive(Clone, Debug, PartialEq)]
pub struct Map {
    pub look: Look,
    pub provider: MapProvider,
    /// The provider's API key, for those that need one.
    pub key: Option<String>,
    /// `[lat, lon]` at the middle of the box.
    pub center: Option<[f64; 2]>,
    /// The zoom level, 0 (the whole world) to the provider's deepest.
    pub zoom: Option<f32>,
    pub pins: Vec<Pin>,
    pub routes: Vec<Route>,
    /// Sent with `lat`, `lon` and `zoom` added after the user moves the map.
    pub moved: Option<Data>,
    /// Sent with `pin` (its index, from 1) added when a pin is clicked.
    pub select: Option<Data>,
    /// The mod `moved` and `select` go to; the drawing mod when unset.
    pub to: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub grow: bool,
}

/// A marker on a map: a dot, or a disc with a short label such as `1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    pub lat: f64,
    pub lon: f64,
    pub label: Option<String>,
    pub color: Option<crate::Color>,
    /// Sent when it is clicked, instead of the map's `select`.
    pub msg: Option<Data>,
}

/// A line through places on a map: a route, a walk, a boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// `[lat, lon]` corners in order.
    pub points: Vec<[f64; 2]>,
    pub color: Option<crate::Color>,
    /// The stroke's width in pixels.
    pub stroke: f32,
    pub dashed: bool,
}

/// The deepest zoom any map takes, whatever its provider serves.
pub const MAX_MAP_ZOOM: f32 = 22.0;
/// The most Mercator latitude can show; tiles end there.
pub const MAX_LATITUDE: f64 = 85.051_128_78;

impl Map {
    /// The zoom and center that frame the pins and routes in a box of
    /// `width` by `height` pixels, leaving `margin` pixels around them;
    /// the whole world with nothing to frame.
    pub fn framing(&self, width: f64, height: f64, margin: f64) -> ([f64; 2], f64) {
        let places = self.pins.iter().map(|pin| [pin.lat, pin.lon]).chain(
            self.routes
                .iter()
                .flat_map(|route| route.points.iter().copied()),
        );
        let mut bounds: Option<([f64; 2], [f64; 2])> = None;
        for [lat, lon] in places {
            let (x, y) = world(lat, lon);
            bounds = Some(match bounds {
                None => ([x, y], [x, y]),
                Some((low, high)) => ([low[0].min(x), low[1].min(y)], [
                    high[0].max(x),
                    high[1].max(y),
                ]),
            });
        }
        let most = f64::from(self.provider.source().max_zoom).min(16.0);
        let Some((low, high)) = bounds else {
            return ([0.0, 0.0], 1.0);
        };
        let center = unworld((low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0);
        let (span_x, span_y) = (high[0] - low[0], high[1] - low[1]);
        let room_x = (width - 2.0 * margin).max(1.0);
        let room_y = (height - 2.0 * margin).max(1.0);
        // World units span 0..1; at zoom z the world is 256 * 2^z pixels.
        let fits = |room: f64, span: f64| {
            if span <= 0.0 {
                most
            } else {
                (room / (256.0 * span)).log2()
            }
        };
        let zoom = fits(room_x, span_x).min(fits(room_y, span_y));
        (center, zoom.clamp(1.0, most))
    }
}

/// Where `lat`, `lon` falls on the Web Mercator square, 0 to 1 across and
/// down from the top-left corner.
pub fn world(lat: f64, lon: f64) -> (f64, f64) {
    let lat = lat.clamp(-MAX_LATITUDE, MAX_LATITUDE).to_radians();
    let x = (lon + 180.0) / 360.0;
    let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0;
    (x, y)
}

/// The `[lat, lon]` at `x`, `y` on the Web Mercator square.
pub fn unworld(x: f64, y: f64) -> [f64; 2] {
    let lon = x * 360.0 - 180.0;
    let n = std::f64::consts::PI * (1.0 - 2.0 * y);
    let lat = n.sinh().atan().to_degrees();
    [lat.clamp(-MAX_LATITUDE, MAX_LATITUDE), lon]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_and_unworld_round_trip() {
        for (lat, lon) in [(37.7749, -122.4194), (0.0, 0.0), (-33.86, 151.21)] {
            let (x, y) = world(lat, lon);
            let [back_lat, back_lon] = unworld(x, y);
            assert!((back_lat - lat).abs() < 1e-9);
            assert!((back_lon - lon).abs() < 1e-9);
        }
        assert_eq!(world(0.0, 0.0), (0.5, 0.5));
    }

    #[test]
    fn every_provider_names_a_tile_and_credits_its_source() {
        for name in MapProvider::NAMES {
            let provider = MapProvider::parse(name).map(MapProvider::source);
            let Some(source) = provider else {
                panic!("{name} does not parse");
            };
            for part in ["{z}", "{x}", "{y}"] {
                assert!(source.url.contains(part), "{name} lacks {part}");
            }
            assert!(source.url.starts_with("https://"), "{name} is not https");
            assert_eq!(source.key, source.url.contains("{key}"), "{name}'s key");
            assert!(!source.attribution.is_empty());
        }
    }
}
