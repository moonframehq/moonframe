//! `ui.map`'s keys: its provider, where it looks, its pins and routes.

use super::*;

/// The most pins and routes a map draws, and corners each route has.
const MOST_PINS: usize = 500;
const MOST_ROUTES: usize = 100;
const MOST_CORNERS: usize = 5000;
/// The longest API key a map takes.
const LONGEST_KEY: usize = 512;

impl Build for Map {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "provider", "key", "center", "zoom", "pins", "routes", "move", "select", "to", "width",
        "height", "grow",
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
            pins,
            routes,
            moved: fields.get("move")?,
            select: fields.get("select")?,
            to: fields.get("to")?,
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            grow: fields.get("grow")?.unwrap_or(false),
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

/// `{ lat, lon, label, color, msg }`.
impl Read for Pin {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["lat", "lon", "label", "color", "msg"]])?;
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
            color: fields.get("color")?,
            msg: fields.get("msg")?,
        })
    }
}

/// `{ points = { lat1, lon1, lat2, lon2, ... }, color, stroke, dashed }`.
impl Read for Route {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["points", "color", "stroke", "dashed"]])?;
        let flat: Vec<f64> = fields.need("points")?;
        let at = cx.key("points");
        if !flat.len().is_multiple_of(2) || flat.len() < 4 {
            return Err(at.fail(
                "points are lat, lon pairs, at least two of them: { lat1, lon1, lat2, lon2 }",
            ));
        }
        if flat.len() / 2 > MOST_CORNERS {
            return Err(at.fail(format!("at most {MOST_CORNERS} points")));
        }
        let points = flat
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[lat, lon]| place(lat, lon, &at))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            points,
            color: fields.get("color")?,
            stroke: fields.number("stroke", 0.5..=12.0)?.unwrap_or(4.0),
            dashed: fields.get("dashed")?.unwrap_or(false),
        })
    }
}
