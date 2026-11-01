//! Stylesheets: `app/style.luau` restyles elements on top of the layout the
//! modules describe. Rules match an element type (`panel`), a class
//! (`.card`), and optionally a state (`item:selected`, `button:hover`).
//! Elements opt into classes with `class` and add their own `style`.
//!
//! ```lua
//! return style {
//!   radius = 8,
//!   colors = {
//!     accent = { light = "#4f46e5", dark = "#818cf8" }, -- re-themes the app
//!     card = { light = "#ffffff", dark = "#1f1f23" },   -- a new name
//!   },
//!   rules = {
//!     panel = { radius = 10, margin = "small", background = "card", shadow = "small" },
//!     ["item:selected"] = { background = "accent/20", color = "accent" },
//!     [".danger"] = { color = "danger" },
//!   },
//! }
//! ```
//!
//! A mod can bring rules of its own under `styles`; they apply only to what
//! that mod draws. At each step the stylesheet's rule outranks the mod's, so
//! the app can always restyle an extension.
//!
//! A component brings default rules the same way, under `styles`; they
//! apply wherever it is used, under the mod's and the stylesheet's, so
//! `style.luau` restyles any component by its classes.
//!
//! Precedence, lowest first: type rules (the components', the mod's, then
//! the stylesheet's), class rules in the order the element lists them (each
//! the components', the mod's, then the stylesheet's), the element's
//! `style`, then its own keys such as `padding`.

mod read;

use {
    crate::{Border, TextSize, Weight},
    std::{collections::BTreeMap, time::Duration},
};

pub(crate) use read::{Edges, valid_name};

tokens! {
    /// A color the theme defines, so it follows light and dark mode.
    Token {
        Background = "background",
        Foreground = "foreground",
        Muted = "muted",
        MutedForeground = "muted-foreground",
        Sidebar = "sidebar",
        /// What panels and popovers are drawn on.
        Card = "card",
        Secondary = "secondary",
        Border = "border",
        Accent = "accent",
        AccentForeground = "accent-foreground",
        Success = "success",
        Warning = "warning",
        Danger = "danger",
        ListHover = "list-hover",
        ListActive = "list-active",
    }
}

tokens! {
    /// The theme's ready-made shadows; `none` removes one set by a rule below.
    ShadowPreset {
        None = "none",
        Small = "small",
        Medium = "medium",
        Large = "large",
    }
}

tokens! {
    /// What a rule can match besides classes. `panel-header` is the title
    /// bar of a panel, which has no element of its own.
    Part {
        Row = "row",
        Column = "column",
        Panel = "panel",
        PanelHeader = "panel-header",
        Pane = "pane",
        Text = "text",
        Button = "button",
        Item = "item",
        Badge = "badge",
        Icon = "icon",
        Image = "image",
        Line = "line",
        Hatch = "hatch",
        Terminal = "terminal",
        Browser = "browser",
        View = "view",
        Input = "input",
        /// An input's placeholder; its `color` colors it.
        InputPlaceholder = "input-placeholder",
        /// Selected text in an input; its `background` highlights it.
        InputSelection = "input-selection",
        /// An input's text cursor; its `background` colors it.
        InputCaret = "input-caret",
        Divider = "divider",
        Popover = "popover",
        Tooltip = "tooltip",
        Overlay = "overlay",
        /// What dims the window behind an overlay with `backdrop = true`.
        OverlayBackdrop = "overlay-backdrop",
        List = "list",
    }
}

tokens! {
    /// Letter case a text is shown in, whatever its words.
    Case {
        Upper = "upper",
        Lower = "lower",
        None = "none",
    }
}

tokens! {
    /// A typeface role: the interface font or the fixed-width one terminals
    /// and code use.
    Font {
        Sans = "sans",
        Mono = "mono",
    }
}

tokens! {
    State {
        Hover = "hover",
        Selected = "selected",
        /// It has keyboard focus: an input.
        Focus = "focus",
        /// It takes no input: an input or a button.
        Disabled = "disabled",
    }
}

/// A box shadow: a preset, or layers as in CSS.
#[derive(Clone, Debug, PartialEq)]
pub enum Shadow {
    Preset(ShadowPreset),
    Layers(Vec<ShadowLayer>),
}

/// One layer of a CSS `box-shadow`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowLayer {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    /// Drawn inside the element's edge instead of outside it.
    pub inset: bool,
}

tokens! {
    /// How a pane shows and hides when its `visible` changes.
    TransitionKind {
        /// Its size grows from or shrinks to nothing.
        Slide = "slide",
        /// It fades in or out, then takes or gives back its space.
        Fade = "fade",
        SlideFade = "slide-fade",
        None = "none",
    }
}

/// An animation for a change of state: what moves, for how long, and how
/// its speed changes along the way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration: Duration,
    pub curve: Curve,
    /// How long a change of state waits before it starts moving, as CSS's
    /// `transition-delay`: the second duration in `"fade 150ms 40ms"`.
    /// Staggered items each wait a little longer (see `Style::delay`).
    pub delay: Duration,
}

/// An easing curve, as in CSS: progress over time, both from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Linear,
    /// A cubic Bézier from (0, 0) to (1, 1) through these control points.
    CubicBezier(f32, f32, f32, f32),
}

impl Curve {
    pub const EASE: Self = Self::CubicBezier(0.25, 0.1, 0.25, 1.0);
    pub const EASE_IN: Self = Self::CubicBezier(0.42, 0.0, 1.0, 1.0);
    pub const EASE_IN_OUT: Self = Self::CubicBezier(0.42, 0.0, 0.58, 1.0);
    pub const EASE_OUT: Self = Self::CubicBezier(0.0, 0.0, 0.58, 1.0);

    /// Progress at `time`, both from 0 to 1.
    pub fn at(&self, time: f32) -> f32 {
        let time = time.clamp(0.0, 1.0);
        match *self {
            Self::Linear => time,
            Self::CubicBezier(x1, y1, x2, y2) => {
                let bezier = |a: f32, b: f32, t: f32| {
                    3.0 * a * t * (1.0 - t).powi(2) + 3.0 * b * t.powi(2) * (1.0 - t) + t.powi(3)
                };
                // The curve is given by x (time); find its parameter, which
                // grows with x, by bisection, then read y (progress).
                let (mut low, mut high) = (0.0_f32, 1.0_f32);
                for _ in 0..32 {
                    let middle = (low + high) / 2.0;
                    if bezier(x1, x2, middle) < time {
                        low = middle;
                    } else {
                        high = middle;
                    }
                }
                bezier(y1, y2, (low + high) / 2.0)
            },
        }
    }
}

/// An `0xRRGGBBAA` color for each mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shade {
    pub light: u32,
    pub dark: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    Token(Token),
    Fixed(Shade),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub paint: Paint,
    /// From 0 to 1, multiplied into the paint's own alpha: `accent/20`.
    pub alpha: f32,
}

/// A linear gradient from one color to another, painted over the
/// background: `"180deg, #ffa53a, #ff6200"`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    /// The direction in degrees, as in CSS: 0 runs up, 90 right, 180 down.
    pub angle: f32,
    pub from: Color,
    pub to: Color,
}

/// Visual properties; each one unset unless a rule or the element sets it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub background: Option<Color>,
    /// A linear gradient instead of the flat background.
    pub gradient: Option<Gradient>,
    /// Text color; children inherit it.
    pub color: Option<Color>,
    pub border: Option<Border>,
    pub border_color: Option<Color>,
    /// Corner rounding: one radius, or each corner's.
    pub radius: Option<Corners>,
    pub padding: Sides,
    pub gap: Option<f32>,
    pub margin: Sides,
    pub shadow: Option<Shadow>,
    /// Text size; children inherit it.
    pub size: Option<TextSize>,
    pub weight: Option<Weight>,
    /// Typeface; children inherit it.
    pub font: Option<Font>,
    /// Letter case of a `ui.text`: section headings in capitals.
    pub case: Option<Case>,
    pub opacity: Option<f32>,
    /// Width and height in pixels, under the element's own `width` and
    /// `height`: what a named state can change, as QML states do.
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// How a pane, popover or overlay animates when it shows or hides, how
    /// a keyed row or column arrives and goes, and how an element with a
    /// `state` moves from one state's look to the next.
    pub transition: Option<Transition>,
    /// Replaces the `transition`'s delay: what a component sets on each of
    /// a list's items, from its index, so they move one after another while
    /// the rules keep the transition itself.
    pub delay: Option<Duration>,
    /// Hides what overflows its box instead of drawing it outside: a
    /// sidebar whose width eases down to a rail clips its labels.
    pub clip: Option<bool>,
}

impl Style {
    /// Takes every property `over` sets.
    pub fn layer(&mut self, over: &Style) {
        macro_rules! take {
            ($($field:ident),+) => {
                $(if over.$field.is_some() {
                    self.$field = over.$field.clone();
                })+
            };
        }
        take!(
            background,
            gradient,
            color,
            border,
            border_color,
            radius,
            gap,
            shadow,
            size,
            weight,
            font,
            case,
            opacity,
            width,
            height,
            transition,
            delay,
            clip
        );
        self.padding.layer(&over.padding);
        self.margin.layer(&over.margin);
    }
}

/// Pixels for each edge. An unset edge falls through to the rule below, so
/// a class can change `top` alone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sides {
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
}

impl Sides {
    pub fn all(value: f32) -> Self {
        Self {
            top: Some(value),
            right: Some(value),
            bottom: Some(value),
            left: Some(value),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn layer(&mut self, over: &Sides) {
        for (side, over) in [
            (&mut self.top, over.top),
            (&mut self.right, over.right),
            (&mut self.bottom, over.bottom),
            (&mut self.left, over.left),
        ] {
            if over.is_some() {
                *side = over;
            }
        }
    }
}

/// A radius for each corner, clockwise from the top left as in CSS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    pub fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    /// The radius when every corner has the same.
    pub fn uniform(&self) -> Option<f32> {
        let Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        } = *self;
        (top_right == top_left && bottom_right == top_left && bottom_left == top_left)
            .then_some(top_left)
    }
}

/// The classes an element opts into and the style it sets itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Look {
    pub class: Vec<String>,
    pub style: Style,
    /// The state of the view's own naming it is in, which `:<state>` rules
    /// match: `state = "open"` takes `.drawer:open`. With a `transition`,
    /// a change of state moves between the two looks.
    pub state: Option<String>,
    /// The line of the module's source that built the element, for the
    /// inspector to point at.
    pub line: Option<u32>,
}

/// The style for an element at rest, and what changes under the pointer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resolved {
    pub base: Style,
    pub hover: Style,
}

/// The states an element is in when its style is resolved: which of the
/// `:selected`, `:focus` and `:disabled` rules apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct States {
    /// A list row that is the current choice (`ui.item`).
    pub selected: bool,
    /// It has keyboard focus (`ui.input`).
    pub focus: bool,
    /// It takes no input (`ui.input`, `ui.button`).
    pub disabled: bool,
}

/// Color names a stylesheet defines, beside the theme's own.
pub(crate) type Palette = BTreeMap<String, Shade>;

/// The speeds every stylesheet has, which `speeds` can redefine.
const DEFAULT_SPEEDS: &[(&str, u64)] = &[("fast", 120), ("normal", 220), ("slow", 400)];

/// What a stylesheet names for values to refer to: colors, animation
/// speeds, and how fast every animation runs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Names {
    pub(crate) colors: Palette,
    pub(crate) speeds: BTreeMap<String, Duration>,
    /// Divides every duration: 2 is twice as fast, 0 turns animations off.
    pub(crate) animation_speed: f32,
}

impl Names {
    /// Before any stylesheet: no colors of its own, the default speeds.
    pub(crate) const NONE: Self = Self {
        colors: BTreeMap::new(),
        speeds: BTreeMap::new(),
        animation_speed: 1.0,
    };

    /// A named speed, the stylesheet's or a default one.
    pub(crate) fn speed(&self, name: &str) -> Option<Duration> {
        self.speeds.get(name).copied().or_else(|| {
            DEFAULT_SPEEDS
                .iter()
                .find(|(default, _)| *default == name)
                .map(|(_, milliseconds)| Duration::from_millis(*milliseconds))
        })
    }

    /// A duration as the app runs it, after `animation_speed`.
    pub(crate) fn scaled(&self, duration: Duration) -> Duration {
        if self.animation_speed == 0.0 {
            Duration::ZERO
        } else {
            duration.div_f32(self.animation_speed)
        }
    }
}

impl Default for Names {
    fn default() -> Self {
        Self::NONE
    }
}

/// What a rule matches: an element type or a class, maybe in a state.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Selector {
    base: Base,
    state: Option<Pseudo>,
}

/// A selector's state: one the app knows (`hover`), or one a view names
/// and puts an element in with `state`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Pseudo {
    Known(State),
    Named(String),
}

impl Pseudo {
    fn name(&self) -> &str {
        match self {
            Self::Known(state) => state.name(),
            Self::Named(name) => name,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Base {
    Part(Part),
    Class(String),
}

/// Styles by selector: the stylesheet's `rules`, or a mod's `styles`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rules(BTreeMap<Selector, Style>);

impl Rules {
    /// Each selector, as a stylesheet writes it: `.tab:selected`.
    pub fn selectors(&self) -> Vec<String> {
        self.0
            .keys()
            .map(|selector| {
                let base = match &selector.base {
                    Base::Part(part) => part.name().to_owned(),
                    Base::Class(class) => format!(".{class}"),
                };
                match &selector.state {
                    Some(state) => format!("{base}:{}", state.name()),
                    None => base,
                }
            })
            .collect()
    }

    /// Takes every rule `over` has, each laid over the same selector's here.
    pub(crate) fn layer(&mut self, over: &Rules) {
        for (selector, style) in &over.0 {
            self.0.entry(selector.clone()).or_default().layer(style);
        }
    }

    fn get(&self, base: &Base, state: Option<State>) -> Option<&Style> {
        self.0.get(&Selector {
            base: base.clone(),
            state: state.map(Pseudo::Known),
        })
    }

    fn get_named(&self, base: &Base, state: &str) -> Option<&Style> {
        self.0.get(&Selector {
            base: base.clone(),
            state: Some(Pseudo::Named(state.to_owned())),
        })
    }
}

/// `mono = { ... }` in the stylesheet: the fixed-width font. What is left
/// out keeps the app's default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonoFont {
    pub family: Option<String>,
    pub size: Option<f32>,
    /// From 100 (thin) to 900 (black); 400 is regular.
    pub weight: Option<f32>,
    /// Terminal line height as a multiple of the size.
    pub line_height: Option<f32>,
    /// Fonts to look in, in order, for characters the family lacks, such
    /// as a Nerd Font's icons. `None` keeps the default list.
    pub fallbacks: Option<Vec<String>>,
    /// Whether ligatures such as `->` join; `None` keeps the font's own.
    pub ligatures: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stylesheet {
    pub font_size: Option<f32>,
    pub font_family: Option<String>,
    /// The fixed-width font terminals and `font = "mono"` text use.
    pub mono: MonoFont,
    /// The theme's corner radius, which buttons and other controls use.
    pub radius: Option<f32>,
    /// Theme colors replaced for the whole app.
    pub overrides: Vec<(Token, Shade)>,
    /// Which look each kind of component takes, by kind: `looks = { tabs =
    /// "cards" }`. Views see it as `app.looks`; a component's own prop wins.
    pub looks: BTreeMap<String, String>,
    names: Names,
    rules: Rules,
    /// Every component's `styles`, under each mod's and the stylesheet's.
    pub(crate) components: Rules,
}

impl Stylesheet {
    pub(crate) fn names(&self) -> &Names {
        &self.names
    }

    /// A duration as the app runs it, after `animation_speed`: zero when
    /// animations are off.
    pub fn scaled(&self, duration: Duration) -> Duration {
        self.names.scaled(duration)
    }

    /// The style for a `part` with `look`, `selected` or not, drawn by a mod
    /// whose own rules are `local`.
    pub fn resolve(&self, part: Part, look: &Look, selected: bool, local: &Rules) -> Resolved {
        let states = States {
            selected,
            ..States::default()
        };
        self.resolve_in(part, look, states, local)
    }

    /// The style for a `part` with `look` in `states`. At each selector the
    /// states layer over its resting rule (`selected`, then the look's own
    /// named `state`, then `focus`, then `disabled`); a disabled element has
    /// no hover.
    pub fn resolve_in(&self, part: Part, look: &Look, states: States, local: &Rules) -> Resolved {
        let mut resolved = Resolved::default();
        let bases =
            std::iter::once(Base::Part(part)).chain(look.class.iter().cloned().map(Base::Class));
        let active = [
            (states.selected, State::Selected),
            (states.focus, State::Focus),
            (states.disabled, State::Disabled),
        ];
        for base in bases {
            for rules in [&self.components, local, &self.rules] {
                if let Some(style) = rules.get(&base, None) {
                    resolved.base.layer(style);
                }
                for (index, (on, state)) in active.into_iter().enumerate() {
                    if on && let Some(style) = rules.get(&base, Some(state)) {
                        resolved.base.layer(style);
                    }
                    // A named state goes after `selected`.
                    if index == 0
                        && let Some(named) = &look.state
                        && let Some(style) = rules.get_named(&base, named)
                    {
                        resolved.base.layer(style);
                    }
                }
                if !states.disabled
                    && let Some(style) = rules.get(&base, Some(State::Hover))
                {
                    resolved.hover.layer(style);
                }
            }
        }
        resolved.base.layer(&look.style);
        resolved
    }
}
