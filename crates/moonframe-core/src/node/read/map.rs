//! `ui.map`'s keys: its provider, where it looks, its pins and routes.

use super::*;

/// The most pins and routes a map draws, and corners each route has.
const MOST_PINS: usize = 500;
const MOST_ROUTES: usize = 100;
const MOST_CORNERS: usize = 5000;
/// The most stops a routed route passes: what public routers take.
const MOST_STOPS: usize = 25;
/// The longest API key a map takes.
const LONGEST_KEY: usize = 512;

impl Build for Map {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "provider", "key", "center", "zoom", "pins", "routes", "move", "select", "routed", "to",
        "width", "height", "grow", "glide", "bearing",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let provider = fields
            .get("provider")?
            .unwrap_or(MapProvider::OpenStreetMap);
        let key: Option<String> = fields
            .get::<String>("key")?
            .filter(|key| !key.trim().is_empty());
        if let Some(key) = &key
            && (key.len() > LONGEST_KEY || !key.chars().all(|c| c.is_ascii_graphic()))
        {
            return Err(fields
                .cx()
                .key("key")
                .fail("an API key is letters, digits and punctuation, without spaces"));
        }
        if provider.source().key && key.is_none() {
            return Err(fields.cx().key("key").fail(format!(
                "`{}` needs an API key; `openstreetmap` and the `carto-*` maps need none",
                provider.name()
            )));
        }
        let center = fields
            .get::<Vec<f64>>("center")?
            .map(|center| match center[..] {
                [lat, lon] if lat.abs() <= 90.0 && lon.abs() <= 180.0 => Ok([lat, lon]),
                _ => Err(fields
                    .cx()
                    .key("center")
                    .fail("`center` is { lat, lon }, such as { 37.79, -122.40 }")),
            })
            .transpose()?;
        let pins: Vec<Pin> = fields.get("pins")?.unwrap_or_default();
        if pins.len() > MOST_PINS {
            return Err(fields
                .cx()
                .key("pins")
                .fail(format!("at most {MOST_PINS} pins")));
        }
        let routes: Vec<Route> = fields.get("routes")?.unwrap_or_default();
        if routes.len() > MOST_ROUTES {
            return Err(fields
                .cx()
                .key("routes")
                .fail(format!("at most {MOST_ROUTES} routes")));
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            provider,
            key,
            center,
            zoom: fields.number("zoom", 0.0..=MAX_MAP_ZOOM)?,
            bearing: fields.number("bearing", -360.0..=360.0)?,
            pins,
            routes,
            moved: fields.get("move")?,
            select: fields.get("select")?,
            routed: fields.get("routed")?,
            to: fields.get("to")?,
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            grow: fields.get("grow")?.unwrap_or(false),
            glide: fields.get("glide")?.unwrap_or(false),
        })
    }
}

/// A latitude and longitude, each in its range.
fn place(lat: f64, lon: f64, cx: &Cx) -> Result<[f64; 2], Error> {
    if lat.abs() > 90.0 || lon.abs() > 180.0 {
        return Err(cx.fail(format!(
            "{lat}, {lon} is not a place: latitude is -90 to 90, longitude -180 to 180"
        )));
    }
    Ok([lat, lon])
}

/// `{ lat, lon, label, icon, color, shape, selected, pulse, msg }`.
impl Read for Pin {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&[
            "lat", "lon", "label", "icon", "color", "shape", "selected", "pulse", "msg",
        ]])?;
        let [lat, lon] = place(fields.need("lat")?, fields.need("lon")?, cx)?;
        let label: Option<String> = fields.get("label")?;
        if label
            .as_ref()
            .is_some_and(|label| label.chars().count() > 3)
        {
            return Err(cx
                .key("label")
                .fail("a pin's label is short, three characters at most: `1`, `A`, `12`"));
        }
        Ok(Self {
            lat,
            lon,
            label,
            icon: fields.get("icon")?,
            color: fields.get("color")?,
            shape: fields.get("shape")?,
            selected: fields.get("selected")?.unwrap_or(false),
            pulse: fields.get("pulse")?.unwrap_or(false),
            msg: fields.get("msg")?,
        })
    }
}

/// `{ points = { lat1, lon1, ... } }`, or `{ via = { lat1, lon1, ... },
/// mode }` to route along streets; and `color`, `stroke`, `dashed`.
impl Read for Route {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&[
            "points", "via", "mode", "color", "stroke", "dashed",
        ]])?;
        let (points, via) = match (fields.raw("points"), fields.raw("via")) {
            (Some(_), None) => (places(&fields, "points", MOST_CORNERS)?, Vec::new()),
            (None, Some(_)) => (Vec::new(), places(&fields, "via", MOST_STOPS)?),
            _ => {
                return Err(cx.fail(
                    "a route takes `points` (drawn as they are) or `via` (stops a router joins \
                     along streets), one of them",
                ));
            },
        };
        if via.is_empty() && fields.raw("mode").is_some() {
            return Err(cx.key("mode").fail("`mode` is how a `via` route travels"));
        }
        Ok(Self {
            points,
            via,
            mode: fields.get("mode")?.unwrap_or(TravelMode::Walk),
            color: fields.get("color")?,
            stroke: fields.number("stroke", 0.5..=12.0)?.unwrap_or(4.0),
            dashed: fields.get("dashed")?.unwrap_or(false),
        })
    }
}

/// The places a list key holds, flat lat, lon pairs: two to `most`.
fn places(fields: &Fields, key: &str, most: usize) -> Result<Vec<[f64; 2]>, Error> {
    let flat: Vec<f64> = fields.need(key)?;
    let at = fields.cx().key(key);
    if !flat.len().is_multiple_of(2) || flat.len() < 4 {
        return Err(at.fail(format!(
            "{key} are lat, lon pairs, at least two of them: {{ lat1, lon1, lat2, lon2 }}"
        )));
    }
    if flat.len() / 2 > most {
        return Err(at.fail(format!("at most {most} places")));
    }
    flat.as_chunks::<2>()
        .0
        .iter()
        .map(|&[lat, lon]| place(lat, lon, &at))
        .collect()
}
