//! Keys many elements share: frames, actions, extras, `float` and
//! `scroll`.

use {super::*, crate::style::Edges};

/// Widths, heights, and pane sizes.
pub(super) const PIXELS: std::ops::RangeInclusive<f32> = 0.0..=100_000.0;

impl Frame {
    pub(super) fn from_fields(fields: &Fields) -> Result<Self, Error> {
        Ok(Self {
            gap: fields.get("gap")?,
            padding: fields.get("padding")?,
            grow: fields.get("grow")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            surface: fields.get("surface")?,
            border: fields.get::<Edges>("border")?.map(|edges| edges.0),
            scroll: fields
                .get::<ScrollKey>("scroll")?
                .map_or(Scroll::No, |key| key.0),
            follow: fields.get("follow")?.unwrap_or(false),
            shrink: fields.get("shrink")?.unwrap_or(true),
            self_align: fields.get("self_align")?,
            hug: fields.get("hug")?.unwrap_or(false),
            max_width: fields.number("max_width", PIXELS)?,
            max_height: fields.number("max_height", PIXELS)?,
            float: fields.get("float")?,
            area: fields.get("area")?,
        })
    }
}

impl Action {
    /// A control's `msg` (with an optional `to`), `command` or `window`; a browser
    /// command names its browser with `browser` (and its mod with `to`).
    pub(crate) fn from_fields(fields: &Fields) -> Result<Option<Self>, Error> {
        if let Some(name) = fields.get::<PlainId>("window")? {
            let alone = ["msg", "command", "to", "browser"]
                .iter()
                .all(|key| fields.raw(key).is_none());
            return if alone {
                Ok(Some(Self::Window(name.0)))
            } else {
                Err(fields.cx().key("window").fail(
                    "`window` (a mod to open in its own window) takes no `msg`, `to` or `command`",
                ))
            };
        }
        let browser = fields.get::<BrowserId>("browser")?.map(|id| id.0);
        let misplaced = || {
            fields.cx().key("browser").fail(format!(
                "`browser` goes with a browser command: {}",
                BrowserCommand::NAMES.join(", ")
            ))
        };
        match (
            fields.get::<Data>("msg")?,
            fields.get::<String>("to")?,
            fields.get::<AnyCommand>("command")?,
        ) {
            (Some(_), _, Some(_)) => Err(fields.cx().fail("takes `msg` or `command`, not both")),
            (Some(_), _, None) if browser.is_some() => Err(misplaced()),
            (Some(msg), to, None) => Ok(Some(Self::Send { to, msg })),
            (None, to, Some(AnyCommand::Browser(command))) => {
                let id = browser.ok_or_else(|| {
                    fields
                        .cx()
                        .key("command")
                        .fail("a browser command needs `browser = \"<id>\"`, its ui.browser's id")
                })?;
                Ok(Some(Self::Browser { to, id, command }))
            },
            (None, ..) if browser.is_some() => Err(misplaced()),
            (None, Some(_), _)
                if fields.raw("context").is_none() && !Pointer::sends(fields) =>
            {
                Err(fields.cx().key("to").fail(
                    "`to` goes with a `msg`, `context`, `pointer`, `wheel`, `drop` or browser `command`",
                ))
            },
            (None, _, Some(AnyCommand::App(command))) => Ok(Some(Self::Command(command))),
            (None, _, None) => Ok(None),
        }
    }
}

impl Extras {
    /// A control's `tooltip`, `context` (a message sent to `to` like `msg`)
    /// and `copy`.
    pub(super) fn from_fields(fields: &Fields) -> Result<Self, Error> {
        let context = fields
            .get::<Data>("context")?
            .map(|msg| -> Result<Action, Error> {
                Ok(Action::Send {
                    to: fields.get("to")?,
                    msg,
                })
            })
            .transpose()?;
        Ok(Self {
            tooltip: fields.get("tooltip")?,
            context,
            copy: fields.get("copy")?,
        })
    }
}

/// Where `float = "<at>"` pins an element: the edges it is held to.
const FLOAT_AT: &[(&str, [bool; 4])] = &[
    // top, right, bottom, left
    ("top-left", [true, false, false, true]),
    ("top-right", [true, true, false, false]),
    ("bottom-left", [false, false, true, true]),
    ("bottom-right", [false, true, true, false]),
    ("top", [true, true, false, true]),
    ("bottom", [false, true, true, true]),
    ("left", [true, false, true, true]),
    ("right", [true, true, true, false]),
    ("fill", [true, true, true, true]),
];

/// `float = { top = -4, right = -4 }`: pixels from the parent's edges. Or
/// pinned to an edge or corner, `float = "top-right"`, and set in from it,
/// `float = { at = "top-right", inset = 4 }`.
impl Read for Offsets {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        const RANGE: std::ops::RangeInclusive<f32> = -100_000.0..=100_000.0;
        if let Value::String(_) = value {
            return pinned(&String::read(value, cx)?, 0.0, cx);
        }
        let fields = Fields::read(value, cx, &[&[
            "top", "right", "bottom", "left", "at", "inset",
        ]])?;
        if let Some(at) = fields.get::<String>("at")? {
            let edges = ["top", "right", "bottom", "left"];
            if let Some(edge) = edges.iter().find(|edge| fields.raw(edge).is_some()) {
                return Err(cx
                    .key(edge)
                    .fail("give `at` and `inset`, or the edges, not both"));
            }
            let inset = fields.number("inset", RANGE)?.unwrap_or(0.0);
            return pinned(&at, inset, &cx.key("at"));
        }
        if fields.raw("inset").is_some() {
            return Err(cx.key("inset").fail("`inset` goes with `at`"));
        }
        let edge = |key| fields.number(key, RANGE);
        Ok(Self {
            top: edge("top")?,
            right: edge("right")?,
            bottom: edge("bottom")?,
            left: edge("left")?,
        })
    }
}

/// The offsets that hold an element `inset` pixels in from the edges `at`
/// names.
fn pinned(at: &str, inset: f32, cx: &Cx) -> Result<Offsets, Error> {
    let Some((_, [top, right, bottom, left])) = FLOAT_AT.iter().find(|(name, _)| *name == at)
    else {
        let names: Vec<&str> = FLOAT_AT.iter().map(|(name, _)| *name).collect();
        return Err(cx.fail(format!(
            "`{at}` is not a place; use {}, or {{ top, right, bottom, left }}",
            names.join(", ")
        )));
    };
    let edge = |held: &bool| held.then_some(inset);
    Ok(Offsets {
        top: edge(top),
        right: edge(right),
        bottom: edge(bottom),
        left: edge(left),
    })
}

/// `scroll = true` (along the axis), or `"x"`, `"y"`, `"both"`.
struct ScrollKey(Scroll);

impl Read for ScrollKey {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Boolean(true) => Ok(Self(Scroll::Along)),
            Value::Boolean(false) => Ok(Self(Scroll::No)),
            _ => Scroll::read(value, cx).map(Self),
        }
    }
}

/// A control's `command`: the app's own, or one for a browser.
enum AnyCommand {
    App(Command),
    Browser(BrowserCommand),
}

impl Read for AnyCommand {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let name = String::read(value, cx)?;
        Command::parse(&name)
            .map(Self::App)
            .or_else(|| BrowserCommand::parse(&name).map(Self::Browser))
            .ok_or_else(|| {
                cx.fail(format!(
                    "unknown `{name}`; expected one of {}, {}",
                    Command::NAMES.join(", "),
                    BrowserCommand::NAMES.join(", ")
                ))
            })
    }
}
