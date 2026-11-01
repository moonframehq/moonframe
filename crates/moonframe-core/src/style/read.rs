//! Reads stylesheets, mod `styles`, and element `style` tables. Each value a
//! style can hold is a type that reads itself; colors resolve against the
//! [`Names`] the reader's context carries.

use {
    super::*,
    crate::{
        Error, Space,
        read::{Cx, Fields, NO_NAMES, Read, within},
    },
    mlua::{Table, Value},
    std::{str::FromStr, time::Duration},
};

/// What a shadow layer without a color casts: black at a quarter opacity.
const SHADOW_COLOR: u32 = 0x0000_0040;
const MAX_SHADOW_LAYERS: usize = 8;

const SHEET_KEYS: &[&str] = &[
    "font_size",
    "font_family",
    "mono",
    "radius",
    "colors",
    "speeds",
    "animation_speed",
    "looks",
    "rules",
];
const STYLE_KEYS: &[&str] = &[
    "background",
    "gradient",
    "color",
    "border",
    "border_color",
    "radius",
    "padding",
    "gap",
    "margin",
    "shadow",
    "size",
    "weight",
    "font",
    "case",
    "opacity",
    "width",
    "height",
    "transition",
];
const MONO_KEYS: &[&str] = &[
    "family",
    "size",
    "weight",
    "line_height",
    "fallbacks",
    "ligatures",
];
const EDGE_KEYS: &[&str] = &["x", "y", "top", "right", "bottom", "left"];
const LAYER_KEYS: &[&str] = &["x", "y", "blur", "spread", "color", "inset"];

/// `looks = { tabs = "cards", ... }`: plain names on both sides, as a look
/// becomes a class (`tabs-cards`).
fn looks(fields: &Fields) -> Result<BTreeMap<String, String>, Error> {
    let Some(table) = fields.get::<Table>("looks")? else {
        return Ok(BTreeMap::new());
    };
    let cx = fields.cx().key("looks");
    let mut looks = BTreeMap::new();
    for pair in table.pairs::<String, String>() {
        let (kind, look) = pair?;
        if !valid_name(&kind) || !valid_name(&look) {
            return Err(cx.key(&kind).fail("use a-z, 0-9, - and _ in looks"));
        }
        looks.insert(kind, look);
    }
    Ok(looks)
}

impl Stylesheet {
    /// Reads what `style.luau` returns. Colors and speeds come first, so
    /// rules can name them.
    pub(crate) fn read(value: &Value) -> Result<Self, Error> {
        let root = Cx::root(&NO_NAMES);
        if !matches!(value, Value::Table(_)) {
            return Err(root.fail("style.luau must return style { ... }"));
        }
        let fields = Fields::read(value, &root, &[SHEET_KEYS])?;
        let mut names = Names {
            speeds: speeds(&fields)?,
            animation_speed: fields.number("animation_speed", 0.0..=10.0)?.unwrap_or(1.0),
            ..Names::NONE
        };
        let mut overrides = Vec::new();
        if let Some(colors) = fields.get::<Table>("colors")? {
            let cx = root.key("colors");
            for pair in colors.pairs::<String, Value>() {
                let (name, value) = pair?;
                let cx = cx.key(&name);
                let shade = Shade::read(&value, &cx)?;
                match Token::parse(&name) {
                    Some(token) => overrides.push((token, shade)),
                    None if valid_name(&name) => {
                        names.colors.insert(name, shade);
                    },
                    None => return Err(cx.fail("use a-z, 0-9, - and _ in color names")),
                }
            }
        }
        overrides.sort_by_key(|(token, _)| *token);
        let rules = fields
            .raw("rules")
            .map(|rules| Rules::read(rules, &Cx::root(&names).key("rules")))
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            font_size: fields.number("font_size", 6.0..=64.0)?,
            font_family: fields.get("font_family")?,
            mono: fields
                .raw("mono")
                .map(|mono| MonoFont::read(mono, &root.key("mono")))
                .transpose()?
                .unwrap_or_default(),
            radius: fields.number("radius", 0.0..=64.0)?,
            overrides,
            looks: looks(&fields)?,
            names,
            rules,
            components: Rules::default(),
        })
    }
}

impl Read for MonoFont {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[MONO_KEYS])?;
        Ok(Self {
            family: fields.get("family")?,
            size: fields.number("size", 6.0..=64.0)?,
            weight: fields.number("weight", 100.0..=900.0)?,
            line_height: fields.number("line_height", 0.8..=3.0)?,
            fallbacks: fields.get("fallbacks")?,
            ligatures: fields.get("ligatures")?,
        })
    }
}

/// `speeds = { fast = 120, snappy = "90ms" }`: names for durations that
/// transitions can use instead of numbers.
fn speeds(fields: &Fields) -> Result<BTreeMap<String, Duration>, Error> {
    let Some(table) = fields.get::<Table>("speeds")? else {
        return Ok(BTreeMap::new());
    };
    let cx = fields.cx().key("speeds");
    let mut speeds = BTreeMap::new();
    for pair in table.pairs::<String, Value>() {
        let (name, value) = pair?;
        let cx = cx.key(&name);
        // A speed is one word in a transition; it must not read as
        // something else there.
        let reserved = TransitionKind::parse(&name).is_some() || Curve::named(&name).is_some();
        if !valid_name(&name) || reserved {
            return Err(
                cx.fail("use a-z, 0-9, - and _ in speed names, and not a transition kind or curve")
            );
        }
        speeds.insert(name, Speed::read(&value, &cx)?.0);
    }
    Ok(speeds)
}

impl Read for Rules {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::Table(table) = value else {
            return Err(cx.fail("expected a table of rules"));
        };
        let mut rules = BTreeMap::new();
        for pair in table.pairs::<String, Value>() {
            let (selector, style) = pair?;
            let cx = cx.quoted(&selector);
            let selector = selector
                .parse()
                .map_err(|problem: String| cx.fail(problem))?;
            rules.insert(selector, Style::read(&style, &cx)?);
        }
        Ok(Self(rules))
    }
}

impl FromStr for Selector {
    type Err = String;

    /// `panel`, `.card`, `item:selected`, `.card:hover`.
    fn from_str(selector: &str) -> Result<Self, String> {
        let (base, state) = match selector.split_once(':') {
            Some((base, state)) => (base, Some(state)),
            None => (selector, None),
        };
        // Any other name is a state of the view's own, which an element
        // enters with `state = "<name>"`.
        let state = state
            .map(|state| match State::parse(state) {
                Some(known) => Ok(Pseudo::Known(known)),
                None if valid_name(state) => Ok(Pseudo::Named(state.to_owned())),
                None => Err(format!(
                    "`{state}` is not a state: use {}, or a name of a-z, 0-9, - and _",
                    State::NAMES.join(", ")
                )),
            })
            .transpose()?;
        let base = match base.strip_prefix('.') {
            Some(class) if valid_name(class) => Base::Class(class.to_owned()),
            Some(_) => return Err("use a-z, 0-9, - and _ in class names".to_owned()),
            None => Base::Part(Part::parse(base).ok_or_else(|| {
                format!(
                    "`{base}` is not an element; use one of {}, or `.class`",
                    Part::NAMES.join(", ")
                )
            })?),
        };
        Ok(Self { base, state })
    }
}

impl Read for Style {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        if !matches!(value, Value::Table(_)) {
            return Err(cx.fail("expected a table of properties"));
        }
        let fields = Fields::read(value, cx, &[STYLE_KEYS])?;
        Ok(Self {
            background: fields.get("background")?,
            gradient: fields.get("gradient")?,
            color: fields.get("color")?,
            border: fields.get::<Edges>("border")?.map(|edges| edges.0),
            border_color: fields.get("border_color")?,
            radius: fields.get("radius")?,
            padding: fields.get("padding")?.unwrap_or_default(),
            gap: fields.get::<Spacing>("gap")?.map(|gap| gap.0),
            margin: fields.get("margin")?.unwrap_or_default(),
            shadow: fields.get("shadow")?,
            size: fields.get("size")?,
            weight: fields.get("weight")?,
            font: fields.get("font")?,
            case: fields.get("case")?,
            opacity: fields.number("opacity", 0.0..=1.0)?,
            width: fields.number("width", 0.0..=100_000.0)?,
            height: fields.number("height", 0.0..=100_000.0)?,
            transition: fields.get("transition")?,
        })
    }
}

impl Look {
    /// An element's `class`, `style` and `state` keys.
    pub(crate) fn from_fields(fields: &Fields) -> Result<Self, Error> {
        let state = fields.get::<String>("state")?;
        if let Some(state) = &state
            && !valid_name(state)
        {
            return Err(fields
                .cx()
                .key("state")
                .fail("use a-z, 0-9, - and _ in a state's name"));
        }
        Ok(Self {
            class: fields.get::<Classes>("class")?.unwrap_or_default().0,
            style: fields.get("style")?.unwrap_or_default(),
            state,
            line: fields.line(),
        })
    }
}

/// `class = "a b"` or `class = { "a", "b" }`.
#[derive(Default)]
struct Classes(Vec<String>);

impl Read for Classes {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let names = match value {
            Value::String(names) => names
                .to_string_lossy()
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            Value::Table(_) => Vec::<String>::read(value, cx)?,
            _ => return Err(cx.fail("expected class names")),
        };
        match names.iter().find(|name| !valid_name(name)) {
            Some(bad) => Err(cx.fail(format!(
                "`{bad}` is not a class name: use a-z, 0-9, - and _"
            ))),
            None => Ok(Self(names)),
        }
    }
}

/// A border: `true` for every edge, `false` for none, or an edge's name.
pub(crate) struct Edges(pub(crate) Border);

impl Read for Edges {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Boolean(true) => Ok(Self(Border::All)),
            Value::Boolean(false) => Ok(Self(Border::None)),
            value => Border::read(value, cx).map(Self),
        }
    }
}

/// A step on the spacing scale, or pixels.
struct Spacing(f32);

impl Read for Spacing {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let pixels = match value {
            Value::String(_) => Space::read(value, cx)?.pixels(),
            Value::Integer(_) | Value::Number(_) => {
                within(f32::read(value, cx)?, 0.0..=1000.0, cx)?
            },
            _ => return Err(cx.fail("expected a space name or pixels")),
        };
        Ok(Self(pixels))
    }
}

/// Rounding: one radius for every corner, or a CSS-style list clockwise
/// from the top left (`{ 8, 8, 0, 0 }` rounds the top only; `{ a, b }`
/// gives the top left and bottom right `a`, the others `b`).
impl Read for Corners {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let radius = |value: &Value, cx: &Cx| match value {
            Value::Integer(_) | Value::Number(_) => {
                let radius = f32::read(value, cx)?;
                if (0.0..=1000.0).contains(&radius) {
                    Ok(radius)
                } else {
                    Err(cx.fail("a radius is from 0 to 1000"))
                }
            },
            _ => Err(cx.fail("expected a radius in pixels, or a list of 2 or 4")),
        };
        let Value::Table(table) = value else {
            return radius(value, cx).map(Self::all);
        };
        let values = (1..=table.raw_len())
            .map(|index| radius(&table.raw_get::<Value>(index)?, &cx.index(index)))
            .collect::<Result<Vec<_>, _>>()?;
        if table.pairs::<Value, Value>().count() != values.len() {
            return Err(cx.fail("a radius list takes only numbers"));
        }
        let [top_left, top_right, bottom_right, bottom_left] = match values[..] {
            [all] => [all; 4],
            [a, b] => [a, b, a, b],
            [top_left, top_right, bottom_right, bottom_left] => {
                [top_left, top_right, bottom_right, bottom_left]
            },
            _ => return Err(cx.fail("a radius list takes 1, 2, or 4 values")),
        };
        Ok(Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        })
    }
}

/// Padding or margin: one spacing for every edge, a CSS-style list
/// (`{ y, x }` or `{ top, right, bottom, left }`), or named edges
/// (`{ x = 12, top = 4 }`, where edge names win over `x` and `y`).
impl Read for Sides {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::Table(table) = value else {
            return Ok(Self::all(Spacing::read(value, cx)?.0));
        };
        if table.raw_len() == 0 {
            let fields = Fields::table(table, cx, &[EDGE_KEYS])?;
            let edge = |name: &str, axis: &str| -> Result<Option<f32>, Error> {
                let own = fields.get::<Spacing>(name)?;
                Ok(own
                    .or(fields.get::<Spacing>(axis)?)
                    .map(|spacing| spacing.0))
            };
            return Ok(Self {
                top: edge("top", "y")?,
                right: edge("right", "x")?,
                bottom: edge("bottom", "y")?,
                left: edge("left", "x")?,
            });
        }
        if table.pairs::<Value, Value>().count() != table.raw_len() {
            return Err(cx.fail("use a list or named edges, not both"));
        }
        let values: Vec<f32> = Vec::<Spacing>::read(value, cx)?
            .into_iter()
            .map(|spacing| spacing.0)
            .collect();
        let [top, right, bottom, left] = match values[..] {
            [all] => [all; 4],
            [y, x] => [y, x, y, x],
            [top, right, bottom, left] => [top, right, bottom, left],
            _ => return Err(cx.fail("a list takes 1, 2, or 4 values")),
        };
        Ok(Self {
            top: Some(top),
            right: Some(right),
            bottom: Some(bottom),
            left: Some(left),
        })
    }
}

/// A shadow preset, a CSS string such as `"0 4px 12px #0003"` (layers
/// separated by commas), one layer as a table
/// (`{ y = 4, blur = 12, color = "accent/30" }`), or a list of layers.
impl Read for Shadow {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let layers = match value {
            Value::String(text) => {
                let text = text.to_string_lossy();
                if let Some(preset) = ShadowPreset::parse(&text) {
                    return Ok(Self::Preset(preset));
                }
                text.split(',')
                    .enumerate()
                    .map(|(index, layer)| ShadowLayer::css(layer, &cx.index(index + 1)))
                    .collect::<Result<Vec<_>, _>>()?
            },
            Value::Table(table) if table.raw_len() > 0 => Vec::<ShadowLayer>::read(value, cx)?,
            Value::Table(_) => vec![ShadowLayer::read(value, cx)?],
            _ => {
                return Err(cx.fail(format!(
                    "expected one of {}, a CSS shadow such as \"0 4px 12px #0003\", or a table",
                    ShadowPreset::NAMES.join(", ")
                )));
            },
        };
        if layers.len() > MAX_SHADOW_LAYERS {
            return Err(cx.fail(format!("more than {MAX_SHADOW_LAYERS} layers")));
        }
        Ok(Self::Layers(layers))
    }
}

/// A gradient as a CSS-like string, `"180deg, #ffa53a, #ff6200"` (the angle
/// optional, top to bottom by default), or `{ from, to, angle }`.
impl Read for Gradient {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::String(text) => {
                let text = text.to_string_lossy();
                let parts: Vec<&str> = text.split(',').map(str::trim).collect();
                let (angle, colors) = match parts.split_first() {
                    Some((first, rest)) if first.ends_with("deg") => {
                        let angle = first
                            .trim_end_matches("deg")
                            .trim()
                            .parse::<f32>()
                            .ok()
                            .filter(|angle| angle.is_finite())
                            .ok_or_else(|| cx.fail(format!("`{first}` is not an angle")))?;
                        (angle, rest)
                    },
                    _ => (180.0, &parts[..]),
                };
                match colors {
                    [from, to] => Ok(Self {
                        angle: angle.rem_euclid(360.0),
                        from: Color::named(from, cx)?,
                        to: Color::named(to, cx)?,
                    }),
                    _ => Err(cx.fail(
                        "a gradient is \"[angle deg,] from, to\", such as \"180deg, #ffa53a, #ff6200\"",
                    )),
                }
            },
            Value::Table(_) => {
                let fields = Fields::read(value, cx, &[&["from", "to", "angle"]])?;
                Ok(Self {
                    angle: fields.get::<f32>("angle")?.unwrap_or(180.0).rem_euclid(360.0),
                    from: fields.need("from")?,
                    to: fields.need("to")?,
                })
            },
            _ => Err(cx.fail("expected a gradient string such as \"180deg, #ffa53a, #ff6200\", or { from, to, angle }")),
        }
    }
}

/// A layer as a CSS string or a table of `x`, `y`, `blur`, `spread`,
/// `color`, and `inset`, where missing lengths are 0.
impl Read for ShadowLayer {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        if let Value::String(text) = value {
            return Self::css(&text.to_string_lossy(), cx);
        }
        let fields = Fields::read(value, cx, &[LAYER_KEYS])?;
        let length = |key| Ok::<_, Error>(fields.get::<f32>(key)?.unwrap_or(0.0));
        Self::new(
            [
                length("x")?,
                length("y")?,
                length("blur")?,
                length("spread")?,
            ],
            fields.get("color")?,
            fields.get("inset")?.unwrap_or(false),
            cx,
        )
    }
}

impl ShadowLayer {
    /// `[inset] x y [blur [spread]] [color]`, lengths in pixels with or
    /// without `px`.
    fn css(text: &str, cx: &Cx) -> Result<Self, Error> {
        let mut lengths = Vec::new();
        let mut color = None;
        let mut inset = false;
        for word in text.split_whitespace() {
            let length = word.strip_suffix("px").unwrap_or(word).parse::<f32>();
            match (word, length, &color) {
                ("inset", ..) => inset = true,
                (_, Ok(length), None) if length.is_finite() => lengths.push(length),
                (_, Ok(_), Some(_)) => {
                    return Err(cx.fail(format!("`{word}` must come before the color")));
                },
                (_, _, None) => color = Some(Color::named(word, cx)?),
                (_, _, Some(_)) => return Err(cx.fail(format!("unexpected `{word}`"))),
            }
        }
        let lengths = match lengths[..] {
            [x, y] => [x, y, 0.0, 0.0],
            [x, y, blur] => [x, y, blur, 0.0],
            [x, y, blur, spread] => [x, y, blur, spread],
            _ => {
                return Err(
                    cx.fail("a shadow takes 2 to 4 lengths: x y [blur [spread]], then a color")
                );
            },
        };
        Self::new(lengths, color, inset, cx)
    }

    fn new(
        [x, y, blur, spread]: [f32; 4],
        color: Option<Color>,
        inset: bool,
        cx: &Cx,
    ) -> Result<Self, Error> {
        // Both the CSS and the table forms come through here, so they share
        // one range.
        for length in [x, y, blur, spread] {
            within(length, -1000.0..=1000.0, cx)?;
        }
        if blur < 0.0 {
            return Err(cx.fail("blur cannot be negative"));
        }
        Ok(Self {
            x,
            y,
            blur,
            spread,
            color: color.unwrap_or(Color {
                paint: Paint::Fixed(Shade::both(SHADOW_COLOR)),
                alpha: 1.0,
            }),
            inset,
        })
    }
}

/// The longest transition: anything slower reads as a hang.
const MAX_DURATION_MS: f32 = 10_000.0;
const TRANSITION_KEYS: &[&str] = &["kind", "duration", "curve"];

/// A transition as CSS-like words in any order, such as
/// `"slide 220ms ease-out"` or `"fade 0.3s cubic-bezier(0.2, 0, 0, 1)"`, or
/// a table `{ kind = "slide", duration = 220, curve = "ease-out" }` with
/// the duration in milliseconds or a speed. Durations can be speed names
/// (`"slide fast"`). Missing parts are a slide, the `normal` speed, `ease`.
/// The stylesheet's `animation_speed` applies to the result.
impl Read for Transition {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let names = cx.names();
        let mut transition = Self {
            kind: TransitionKind::Slide,
            duration: names.speed("normal").unwrap_or(Duration::from_millis(220)),
            curve: Curve::EASE,
        };
        match value {
            Value::String(text) => {
                let text = text.to_string_lossy();
                // A Bézier's arguments hold spaces, so take it out first.
                let (rest, bezier) = match text.find("cubic-bezier(") {
                    Some(start) => {
                        let end = text[start..]
                            .find(')')
                            .map(|end| start + end + 1)
                            .ok_or_else(|| cx.fail("`cubic-bezier(` is missing its `)`"))?;
                        let rest = format!("{} {}", &text[..start], &text[end..]);
                        (rest, Some(Curve::parse(&text[start..end], cx)?))
                    },
                    None => (text.clone(), None),
                };
                if let Some(curve) = bezier {
                    transition.curve = curve;
                }
                for word in rest.split_whitespace() {
                    if let Some(kind) = TransitionKind::parse(word) {
                        transition.kind = kind;
                    } else if let Some(Speed(duration)) = Speed::word(word, cx)? {
                        transition.duration = duration;
                    } else {
                        transition.curve = Curve::parse(word, cx)?;
                    }
                }
            },
            Value::Table(_) => {
                let fields = Fields::read(value, cx, &[TRANSITION_KEYS])?;
                if let Some(kind) = fields.get("kind")? {
                    transition.kind = kind;
                }
                if let Some(Speed(duration)) = fields.get("duration")? {
                    transition.duration = duration;
                }
                if let Some(curve) = fields.get::<String>("curve")? {
                    transition.curve = Curve::parse(&curve, &fields.cx().key("curve"))?;
                }
            },
            _ => {
                return Err(
                    cx.fail("expected a transition such as \"slide 220ms ease-out\", or a table")
                );
            },
        }
        transition.duration = names.scaled(transition.duration);
        Ok(transition)
    }
}

/// How long an animation takes: milliseconds as a number, or a string such
/// as `"220ms"`, `"0.22s"`, or a speed name (`"fast"`).
struct Speed(Duration);

impl Read for Speed {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Integer(_) | Value::Number(_) => {
                within(f32::read(value, cx)?, 0.0..=MAX_DURATION_MS, cx).map(Self::milliseconds)
            },
            Value::String(word) => Self::word(&word.to_string_lossy(), cx)?.ok_or_else(|| {
                cx.fail(
                    "expected milliseconds, a duration such as \"220ms\", or a speed: fast, normal, slow, or one from `speeds`",
                )
            }),
            _ => Err(cx.fail("expected milliseconds or a speed")),
        }
    }
}

impl Speed {
    /// Whole microseconds, so `220ms` stays exactly 220ms.
    fn milliseconds(milliseconds: f32) -> Self {
        Self(Duration::from_micros((milliseconds * 1000.0).round() as u64))
    }

    /// A speed name, `220ms`, or `0.22s`; `None` when the word is none.
    fn word(word: &str, cx: &Cx) -> Result<Option<Self>, Error> {
        if let Some(duration) = cx.names().speed(word) {
            return Ok(Some(Self(duration)));
        }
        let milliseconds = match (word.strip_suffix("ms"), word.strip_suffix('s')) {
            (Some(number), _) => number.parse::<f32>().ok(),
            (None, Some(number)) => number.parse::<f32>().ok().map(|seconds| seconds * 1000.0),
            (None, None) => None,
        };
        milliseconds
            .map(|milliseconds| {
                within(milliseconds, 0.0..=MAX_DURATION_MS, cx).map(Self::milliseconds)
            })
            .transpose()
    }
}

impl Curve {
    const NAMES: &[&str] = &[
        "linear",
        "ease",
        "ease-in",
        "ease-out",
        "ease-in-out",
        "cubic-bezier(x1, y1, x2, y2)",
    ];

    /// A curve by name; Béziers are written out instead.
    fn named(text: &str) -> Option<Self> {
        match text {
            "linear" => Some(Self::Linear),
            "ease" => Some(Self::EASE),
            "ease-in" => Some(Self::EASE_IN),
            "ease-out" => Some(Self::EASE_OUT),
            "ease-in-out" => Some(Self::EASE_IN_OUT),
            _ => None,
        }
    }

    fn parse(text: &str, cx: &Cx) -> Result<Self, Error> {
        match Self::named(text) {
            Some(curve) => Ok(curve),
            None => {
                let bezier = text;
                let arguments = bezier
                    .strip_prefix("cubic-bezier(")
                    .and_then(|rest| rest.strip_suffix(')'))
                    .ok_or_else(|| {
                        cx.fail(format!(
                            "unknown `{text}`; expected a kind ({}), a duration (200ms, or a speed such as fast), or a curve: {}",
                            TransitionKind::NAMES.join(", "),
                            Self::NAMES.join(", ")
                        ))
                    })?;
                let numbers = arguments
                    .split(',')
                    .map(|number| number.trim().parse::<f32>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| cx.fail("cubic-bezier takes four numbers"))?;
                match numbers[..] {
                    [x1, y1, x2, y2] if (0.0..=1.0).contains(&x1) && (0.0..=1.0).contains(&x2) => {
                        Ok(Self::CubicBezier(x1, y1, x2, y2))
                    },
                    [_, _, _, _] => Err(cx.fail("cubic-bezier's x values must be from 0 to 1")),
                    _ => Err(cx.fail("cubic-bezier takes four numbers")),
                }
            },
        }
    }
}

impl Read for Color {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::String(text) => Self::named(&text.to_string_lossy(), cx),
            _ => Err(cx.fail("expected a color string")),
        }
    }
}

impl Color {
    /// `#rrggbb`, `#rrggbbaa`, `#rgb`, a palette name, or a theme color,
    /// each optionally with `/NN` percent opacity.
    fn named(text: &str, cx: &Cx) -> Result<Self, Error> {
        let (name, alpha) = match text.split_once('/') {
            None => (text, 1.0),
            Some((name, percent)) => match percent.parse::<u8>() {
                Ok(percent) if percent <= 100 => (name, f32::from(percent) / 100.0),
                _ => return Err(cx.fail("opacity after `/` is a percent from 0 to 100")),
            },
        };
        let paint = if name.starts_with('#') {
            let Rgba(rgba) = name.parse().map_err(|problem: String| cx.fail(problem))?;
            Paint::Fixed(Shade::both(rgba))
        } else if let Some(shade) = cx.names().colors.get(name) {
            Paint::Fixed(*shade)
        } else if let Some(token) = Token::parse(name) {
            Paint::Token(token)
        } else {
            return Err(cx.fail(format!(
                "unknown color `{name}`; use #rrggbb, a name from `colors`, or one of {}",
                Token::NAMES.join(", ")
            )));
        };
        Ok(Self { paint, alpha })
    }
}

/// A palette entry: one hex color, or `{ light = ..., dark = ... }`.
impl Read for Shade {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Table(_) => {
                let fields = Fields::read(value, cx, &[&["light", "dark"]])?;
                let Rgba(light) = fields.need("light")?;
                let Rgba(dark) = fields.need("dark")?;
                Ok(Self { light, dark })
            },
            value => Rgba::read(value, cx).map(|Rgba(rgba)| Self::both(rgba)),
        }
    }
}

impl Shade {
    fn both(rgba: u32) -> Self {
        Self {
            light: rgba,
            dark: rgba,
        }
    }
}

/// `0xRRGGBBAA`, written `#rgb`, `#rrggbb`, or `#rrggbbaa`.
struct Rgba(u32);

impl FromStr for Rgba {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let invalid = || format!("`{text}` is not a hex color such as #3b82f6");
        let digits = text.strip_prefix('#').ok_or_else(invalid)?;
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        let value = u32::from_str_radix(digits, 16).map_err(|_| invalid())?;
        let rgba = match digits.len() {
            3 => {
                let [r, g, b] = [8, 4, 0].map(|shift| (value >> shift) & 0xf);
                (r * 0x11) << 24 | (g * 0x11) << 16 | (b * 0x11) << 8 | 0xff
            },
            6 => value << 8 | 0xff,
            8 => value,
            _ => return Err(invalid()),
        };
        Ok(Self(rgba))
    }
}

impl Read for Rgba {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        String::read(value, cx)?
            .parse()
            .map_err(|problem: String| cx.fail(problem))
    }
}

/// Names a script chooses: palette colors, classes, mods.
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}
