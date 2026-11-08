//! `ui.video`.

use super::*;

/// The most guides one video draws.
const MAX_GUIDES: usize = 256;

impl Build for Video {
    const KEYS: &'static [&'static [&'static str]] = &[
        TYPE,
        LOOK,
        ATTACHED,
        &[
            "id", "source", "width", "height", "zoom", "center", "filter", "color", "extended",
            "black", "white", "gain", "gamma", "guides", "probe", "stats", "probed",
        ],
    ];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let pair = |key: &str| -> Result<Option<[f32; 2]>, Error> {
            fields
                .get::<Vec<f32>>(key)?
                .map(|pair| match pair[..] {
                    [x, y] => Ok([x, y]),
                    _ => Err(fields.cx().key(key).fail("is { x, y }, two fractions")),
                })
                .transpose()
        };
        let guides: Vec<VideoGuide> = fields.get("guides")?.unwrap_or_default();
        if guides.len() > MAX_GUIDES {
            return Err(fields
                .cx()
                .key("guides")
                .fail(format!("is at most {MAX_GUIDES} guides")));
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields
                .get::<BrowserId>("id")?
                .map_or_else(|| "video".to_owned(), |id| id.0),
            source: fields.need::<Source>("source")?.0,
            view: VideoView {
                zoom: fields.number("zoom", 0.001..=1000.0)?,
                center: pair("center")?.unwrap_or([0.5, 0.5]),
                filter: fields.get("filter")?.unwrap_or(VideoFilter::Auto),
                color: fields.get("color")?.unwrap_or(VideoColor::Srgb),
                extended: fields.get("extended")?.unwrap_or(false),
                black: fields.number("black", 0.0..=65535.0)?.unwrap_or(0.),
                white: fields.number("white", 0.001..=65535.0)?,
                gain: fields.number("gain", 0.0..=1000.0)?.unwrap_or(1.),
                gamma: fields.number("gamma", 0.1..=10.0)?.unwrap_or(1.),
                guides,
                probe: pair("probe")?,
            },
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            stats: fields.get("stats")?,
            probed: fields.get("probed")?,
            extras: Extras::from_fields(fields)?,
        })
    }
}

/// A ring's name, as an engine made it. Only a name: never a file.
struct Source(String);

impl Read for Source {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let name = String::read(value, cx)?;
        let plain = !name.is_empty()
            && name.len() <= 30
            && !name.starts_with('.')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if plain {
            Ok(Self(name))
        } else {
            Err(cx.fail("a video's `source` is 1 to 30 of a-z, A-Z, 0-9, -, _ and ."))
        }
    }
}

impl Read for VideoGuide {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["x", "y", "width", "height", "color"]])?;
        let fraction =
            |key: &str| -> Result<f32, Error> { Ok(fields.number(key, -1.0..=2.0)?.unwrap_or(0.)) };
        let color = fields
            .get::<String>("color")?
            .map_or(Some([1., 1., 1., 1.]), |color| hex(&color))
            .ok_or_else(|| cx.key("color").fail("is #rrggbb or #rrggbbaa"))?;
        Ok(Self {
            x: fraction("x")?,
            y: fraction("y")?,
            width: fraction("width")?,
            height: fraction("height")?,
            color,
        })
    }
}

/// `#rrggbb` or `#rrggbbaa` as fractions.
fn hex(text: &str) -> Option<[f32; 4]> {
    let digits = text.strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.is_ascii() {
        return None;
    }
    let part = |at: usize| {
        digits
            .get(at..at + 2)
            .and_then(|part| u8::from_str_radix(part, 16).ok())
            .map(|value| f32::from(value) / 255.)
    };
    Some([
        part(0)?,
        part(2)?,
        part(4)?,
        if digits.len() == 8 {
            part(6)?
        } else {
            1.
        },
    ])
}
