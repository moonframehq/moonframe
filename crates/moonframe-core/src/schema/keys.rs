//! The type of each element key, by hand: by element and key where one
//! element reads a key its own way, else by key. A key not listed here is
//! [`KeyType::Any`]; the reader still checks it.

use {
    super::{KeyType, TokenSet},
    crate::{
        Align, Anchor, Arrow, Axis, Border, BrowserCommand, Case, Command, Cursor, Direction, Dots,
        Element, Fit, Font, Justify, MapProvider, Part, PinShape, Placement, Scroll, ShadowPreset,
        Side, Space, Surface, TerminalBackend, TerminalEngine, TextSize, TimelineLabels,
        TimelineSide, Token, Tone, TransitionKind, TravelMode, Variant, VideoFilter, Weight,
        style::State,
    },
};

/// The token set of a `tokens!` enum, named as the enum is.
macro_rules! token_set {
    ($ty:ident) => {
        TokenSet {
            name: stringify!($ty),
            names: $ty::NAMES,
        }
    };
}

/// The token sets the schema lists: those view keys and style rules take.
pub(super) fn token_sets() -> Vec<TokenSet> {
    vec![
        // keep-sorted start
        token_set!(Align),
        token_set!(Anchor),
        token_set!(Arrow),
        token_set!(Axis),
        token_set!(Border),
        token_set!(BrowserCommand),
        token_set!(Case),
        token_set!(Command),
        token_set!(Cursor),
        token_set!(Direction),
        token_set!(Dots),
        token_set!(Element),
        token_set!(Fit),
        token_set!(Font),
        token_set!(Justify),
        token_set!(MapProvider),
        token_set!(Part),
        token_set!(PinShape),
        token_set!(Placement),
        token_set!(Scroll),
        token_set!(ShadowPreset),
        token_set!(Side),
        token_set!(Space),
        token_set!(State),
        token_set!(Surface),
        token_set!(TerminalBackend),
        token_set!(TerminalEngine),
        token_set!(TextSize),
        token_set!(TimelineLabels),
        token_set!(TimelineSide),
        token_set!(Token),
        token_set!(Tone),
        token_set!(TransitionKind),
        token_set!(TravelMode),
        token_set!(Variant),
        token_set!(VideoFilter),
        token_set!(Weight),
        // keep-sorted end
    ]
}

/// Widths, heights and pane sizes, as the reader bounds them.
const PIXELS: KeyType = number(0.0, 100_000.0);

const fn number(min: f64, max: f64) -> KeyType {
    KeyType::Number {
        min: Some(min),
        max: Some(max),
    }
}

/// The type of `key` on the element `element`.
pub(super) fn key_type(element: &str, key: &str) -> KeyType {
    use KeyType::*;
    match (element, key) {
        ("hatch", "gap") => number(2.0, 64.0),
        ("popover", "gap") => number(0.0, 1000.0),
        ("popover", "align") => Tokens(token_set!(Anchor)),
        ("popover", "side") => Tokens(token_set!(Side)),
        ("revision_timeline", "side") => Tokens(token_set!(TimelineSide)),
        ("revision_timeline", "labels") => Tokens(token_set!(TimelineLabels)),
        ("revision_timeline", "selected") => String,
        ("revision_timeline", "spread") => number(0.0, 5.0),
        ("revision_timeline", "tick_width") => number(1.0, 128.0),
        ("revision_timeline", "tick_height") => number(0.5, 16.0),
        ("pane", "size" | "min" | "max") => PIXELS,
        ("line", "min" | "max" | "baseline") => number(-1.0e12, 1.0e12),
        ("line", "area") => Bool,
        ("line", "stroke") => number(0.5, 8.0),
        ("line", "points") => Any,
        ("text", "lines") => number(1.0, 100.0),
        ("image", "zoom") => number(0.001, 1000.0),
        ("map", "bearing") => number(-360.0, 360.0),
        ("map", "provider") => Tokens(token_set!(MapProvider)),
        ("markdown", "scroll") => Bool,
        ("terminal", "engine") => Tokens(token_set!(TerminalEngine)),
        ("terminal", "backend") => Tokens(token_set!(TerminalBackend)),
        ("video", "filter") => Tokens(token_set!(VideoFilter)),
        ("video", "black") => number(0.0, 65535.0),
        ("video", "white") => number(0.001, 65535.0),
        ("video", "gain") => number(0.0, 1000.0),
        ("video", "gamma") => number(0.1, 10.0),
        ("icon", "name") => Icon,
        ("list", "row") => Function,
        ("list", "row_height") => number(1.0, 10_000.0),
        ("list", "count") => number(0.0, 10_000_000.0),
        (_, key) => by_key(key),
    }
}

/// The type a key has wherever [`key_type`] does not say otherwise.
fn by_key(key: &str) -> KeyType {
    use KeyType::*;
    match key {
        "actions" | "overlay" => Elements,
        "align" | "self_align" => Tokens(token_set!(Align)),
        "anchor_clicks" | "backdrop" | "dashed" | "disabled" | "divider" | "drag" | "extended"
        | "fixed" | "focus" | "follow" | "glide" | "grow" | "hug" | "lazy" | "mono"
        | "multiline" | "open" | "prose" | "scrollbar" | "selected" | "shrink" | "small"
        | "spin" | "spread" | "strike" | "truncate" | "typewriter" | "visible" | "wrap" => Bool,
        "badge" | "browser" | "class" | "code" | "component" | "copy" | "cwd" | "detail" | "id"
        | "key" | "label" | "mod" | "name" | "placeholder" | "src" | "state" | "syntax"
        | "text" | "title" | "to" | "tooltip" | "url" | "value" => String,
        "blur" | "capture" | "carry" | "change" | "context" | "dismiss" | "drop" | "exit"
        | "force" | "found" | "hover" | "image" | "link" | "loading" | "measure" | "move"
        | "msg" | "navigate" | "outline" | "pointer" | "props" | "rotate" | "routed"
        | "scrolled" | "section" | "select" | "smart_zoom" | "submit" | "swipe" | "wheel" => Data,
        "border" => OneOf(vec![Bool, Tokens(token_set!(Border))]),
        "color" => Color,
        "command" => OneOf(vec![
            Tokens(token_set!(Command)),
            Tokens(token_set!(BrowserCommand)),
        ]),
        "content" => Element,
        "cursor" => Tokens(token_set!(Cursor)),
        "direction" => Tokens(token_set!(Direction)),
        "dots" => Tokens(token_set!(Dots)),
        "fit" => Tokens(token_set!(Fit)),
        "gap" | "padding" => Tokens(token_set!(Space)),
        "icon" => Icon,
        "justify" => Tokens(token_set!(Justify)),
        "lines" | "rows" | "max_rows" | "line" | "find_index" | "zoom" | "stroke" => Number {
            min: None,
            max: None,
        },
        "place" => Tokens(token_set!(Placement)),
        "scroll" => OneOf(vec![Bool, Tokens(token_set!(Scroll))]),
        "scroll_to" => Number {
            min: Some(1.0),
            max: None,
        },
        "size" => Tokens(token_set!(TextSize)),
        "style" => Style,
        "surface" => Tokens(token_set!(Surface)),
        "tone" => Tokens(token_set!(Tone)),
        "variant" => Tokens(token_set!(Variant)),
        "weight" => Tokens(token_set!(Weight)),
        "width" | "height" | "max_width" | "max_height" => PIXELS,
        _ => Any,
    }
}
