//! Leaf elements that show something: text, buttons, items, badges,
//! icons, lines, hatches and images.

use super::*;

impl Build for Text {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ATTACHED, &[
        "text", "tone", "size", "weight", "truncate", "lines", "wrap", "strike",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields.need("text")?,
            tone: fields.get("tone")?,
            size: fields.get("size")?,
            weight: fields.get("weight")?,
            truncate: fields.get("truncate")?.unwrap_or(false),
            lines: fields
                .number("lines", 1.0..=100.0)?
                .map(|lines| lines as u8),
            wrap: fields.get("wrap")?.unwrap_or(true),
            strike: fields.get("strike")?.unwrap_or(false),
            extras: Extras::from_fields(fields)?,
        })
    }
}

impl Build for Button {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ACTION, EXTRAS, &[
        "label", "icon", "variant", "small", "disabled",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let icon = fields.get("icon")?;
        let label = match icon {
            Some(_) => fields.get("label")?.unwrap_or_default(),
            None => fields.need("label")?,
        };
        Ok(Self {
            look: Look::from_fields(fields)?,
            label,
            icon,
            variant: fields.get("variant")?.unwrap_or(Variant::Normal),
            small: fields.get("small")?.unwrap_or(false),
            disabled: fields.get("disabled")?.unwrap_or(false),
            action: Action::from_fields(fields)?,
            extras: Extras::from_fields(fields)?,
        })
    }
}

impl Build for Item {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ACTION, EXTRAS, &[
        "label", "detail", "badge", "selected",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            label: fields.need("label")?,
            detail: fields.get("detail")?,
            badge: fields.get("badge")?,
            selected: fields.get("selected")?.unwrap_or(false),
            action: Action::from_fields(fields)?,
            extras: Extras::from_fields(fields)?,
        })
    }
}

impl Build for Badge {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ATTACHED, &["text", "tone"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields.need("text")?,
            tone: fields.get("tone")?,
            extras: Extras::from_fields(fields)?,
        })
    }
}

impl Build for Icon {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, ATTACHED, &["name", "tone", "size", "spin"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            name: fields.need("name")?,
            tone: fields.get("tone")?,
            size: fields.get("size")?,
            spin: fields.get("spin")?.unwrap_or(false),
            extras: Extras::from_fields(fields)?,
        })
    }
}

/// The most points a `ui.line` draws.
const MOST_POINTS: usize = 2000;

impl Build for Line {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "points", "min", "max", "baseline", "area", "dots", "stroke", "dashed", "width", "height",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        let points: Vec<f32> = fields.need("points")?;
        if points.len() > MOST_POINTS {
            return Err(fields
                .cx()
                .key("points")
                .fail(format!("at most {MOST_POINTS} points")));
        }
        if points.iter().any(|value| !value.is_finite()) {
            return Err(fields.cx().key("points").fail("points are numbers"));
        }
        let value = -1.0e12..=1.0e12;
        Ok(Self {
            look: Look::from_fields(fields)?,
            points,
            min: fields.number("min", value.clone())?,
            max: fields.number("max", value.clone())?,
            baseline: fields.number("baseline", value)?,
            area: fields.get("area")?.unwrap_or(false),
            dots: fields.get("dots")?,
            stroke: fields.number("stroke", 0.5..=8.0)?.unwrap_or(1.5),
            dashed: fields.get("dashed")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
        })
    }
}

/// The most lines a `ui.paths` draws, and points each line has.
const MOST_PATHS: usize = 500;
const MOST_CORNERS: usize = 200;

impl Build for Paths {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &["paths", "width", "height"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        let paths: Vec<Polyline> = fields.get("paths")?.unwrap_or_default();
        if paths.len() > MOST_PATHS {
            return Err(fields
                .cx()
                .key("paths")
                .fail(format!("at most {MOST_PATHS} paths")));
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            paths,
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
        })
    }
}

/// `{ points = { x1, y1, x2, y2, ... }, color, stroke, dashed, radius,
/// arrow }`: the points in pixels, flat.
impl Read for Polyline {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&[
            "points", "color", "stroke", "dashed", "radius", "arrow",
        ]])?;
        let flat: Vec<f32> = fields.need("points")?;
        let points = cx.key("points");
        if !flat.len().is_multiple_of(2) || flat.len() < 4 {
            return Err(
                points.fail("points are x, y pairs, at least two of them: { x1, y1, x2, y2 }")
            );
        }
        if flat.len() / 2 > MOST_CORNERS {
            return Err(points.fail(format!("at most {MOST_CORNERS} points")));
        }
        if flat
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 1e6)
        {
            return Err(points.fail("points are numbers of pixels"));
        }
        Ok(Self {
            points: flat.as_chunks::<2>().0.to_vec(),
            color: fields.get("color")?,
            stroke: fields.number("stroke", 0.5..=8.0)?.unwrap_or(1.5),
            dashed: fields.get("dashed")?.unwrap_or(false),
            radius: fields.number("radius", 0.0..=64.0)?.unwrap_or(0.0),
            arrow: fields.get("arrow")?,
        })
    }
}

impl Build for Hatch {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, &["gap", "width", "height", "grow"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            gap: fields.number("gap", 2.0..=64.0)?.unwrap_or(6.0),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            grow: fields.get("grow")?.unwrap_or(false),
        })
    }
}

impl Build for Image {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ATTACHED, &[
        "src", "width", "height", "fit", "zoom", "center", "measure", "overlay",
    ]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error> {
        let center = fields
            .get::<Vec<f32>>("center")?
            .map(|center| match center[..] {
                [x, y] => Ok([x, y]),
                _ => Err(fields.cx().key("center").fail(
                    "`center` is { x, y }, two fractions of the picture, such as { 0.5, 0.5 }",
                )),
            })
            .transpose()?;
        Ok(Self {
            look: Look::from_fields(fields)?,
            src: fields.need::<ImageSrc>("src")?.0,
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            fit: fields.get("fit")?.unwrap_or(Fit::Contain),
            zoom: fields.number("zoom", 0.001..=1000.0)?,
            center,
            measure: fields.get("measure")?,
            to: fields.get("to")?,
            overlay: tree.list(fields, "overlay")?,
            extras: Extras::from_fields(fields)?,
        })
    }
}

/// An image's `src`: a file. The host has no HTTP client, and a view must
/// not make the app fetch things, so a URL is refused with what to do.
struct ImageSrc(String);

impl Read for ImageSrc {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let src = String::read(value, cx)?;
        let scheme = src
            .split_once("://")
            .map(|(scheme, _)| scheme)
            .filter(|scheme| {
                !scheme.is_empty()
                    && scheme
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
            });
        if let Some(scheme) = scheme {
            return Err(cx.fail(format!(
                "images are files, not {scheme}:// URLs; have a provider download it to a \
                 cache and pass the path"
            )));
        }
        if src.trim().is_empty() {
            return Err(cx.fail("an empty path"));
        }
        Ok(Self(src))
    }
}

/// Why `name` is no icon, with the bundled names closest to it rather than
/// all of them, which would bury the message.
fn unknown_icon(name: &str) -> String {
    let close: Vec<&str> = ICONS
        .iter()
        .copied()
        .filter(|icon| {
            icon.contains(name)
                || name.contains(icon)
                || icon
                    .split('-')
                    .any(|part| part.len() > 2 && name.contains(part))
                || edits(icon, name) <= 2
        })
        .take(6)
        .collect();
    let all = "see `IconName` in types/moonframe.d.luau for all of them";
    if close.is_empty() {
        format!("no icon named `{name}`; {all}")
    } else {
        format!(
            "no icon named `{name}`; close ones: {}; {all}",
            close.join(", ")
        )
    }
}

/// How many single-letter changes turn `a` into `b`.
fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut last = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(last + usize::from(ca != *cb));
            last = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

impl Read for IconName {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let name = String::read(value, cx)?;
        Self::new(&name).ok_or_else(|| cx.fail(unknown_icon(&name)))
    }
}
