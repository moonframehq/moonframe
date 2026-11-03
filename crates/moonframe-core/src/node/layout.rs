//! Containers and what places things: frames, stacks, panels, splits
//! and panes, regions, popovers, overlays and lists.

use {super::*, crate::Keys};

/// The box around a container's children. Unset values fall back to the
/// stylesheet, then to the element's default.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub gap: Option<Space>,
    pub padding: Option<Space>,
    /// Take the space left along the parent's axis.
    pub grow: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub surface: Option<Surface>,
    pub border: Option<Border>,
    /// Scroll along the container's axis when its children overflow.
    pub scroll: Scroll,
    /// While scrolled to its end, stay there as the content grows: a chat
    /// or a log. Scrolled back, it stays where the user put it.
    pub follow: bool,
    /// Give up space when the parent is short of it (the default); `false`
    /// keeps the content's size, so a strip is never squeezed.
    pub shrink: bool,
    /// Where it sits across the parent's axis, over the parent's `align`:
    /// `start` keeps a control at its own width in a column.
    pub self_align: Option<Align>,
    /// Keep its own width in a column, which otherwise stretches it, and
    /// follow a row's `align` as usual: pills, badges, dots.
    pub hug: bool,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    /// Out of the flow, over the parent at these offsets in pixels (each
    /// edge optional, negative overhangs): a count bubble on an icon's corner.
    /// `float = "top-right"` pins it to an edge or corner.
    pub float: Option<Offsets>,
    /// Out of the flow, over the parent at fractions of its size: a crop
    /// frame on a picture, a marker on a track.
    pub area: Option<Area>,
}

tokens! {
    /// Which ways a container scrolls when its children overflow.
    Scroll {
        No = "no",
        /// Along its own axis: what `scroll = true` means.
        Along = "along",
        X = "x",
        Y = "y",
        Both = "both",
    }
}

/// Distances from a parent's edges, in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offsets {
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    pub look: Look,
    pub direction: Direction,
    pub frame: Frame,
    pub align: Align,
    pub justify: Justify,
    /// Move children that do not fit onto another line.
    pub wrap: bool,
    /// Matches `:selected` rules, as a selected item does: a tab, a
    /// segment, a row.
    pub selected: bool,
    /// Reveal this direct child (one-based) when it changes; scrolling
    /// by hand remains free between changes.
    pub scroll_to: Option<usize>,
    /// What clicking anywhere in it does, as on a button.
    pub action: Option<Action>,
    pub extras: Extras,
    /// Dragging it moves the window and a double click zooms it, as a
    /// title bar does; controls inside it still take their clicks.
    pub drag: bool,
    /// Names it among its siblings, so it stays itself as they come and
    /// go: with a `transition` rule it grows in when it appears and shrinks
    /// away when it goes, as tabs open and close.
    pub key: Option<String>,
    /// Presses, drags, the wheel, dropped files and the cursor; boxed, as
    /// most stacks have none.
    pub pointer: Option<Box<Pointer>>,
    pub children: Vec<Node>,
}

/// A titled section: a header with optional actions over a column body.
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub look: Look,
    pub title: Option<String>,
    pub actions: Vec<Node>,
    pub frame: Frame,
    pub children: Vec<Node>,
}

/// Panes the user can resize by dragging the edges between them.
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    pub direction: Direction,
    /// Keeps the user's sizes across renders; defaults to the split's path.
    pub id: Option<String>,
    /// Draws a hairline between panes; without it the panes meet with no
    /// line, and only a hovered or dragged handle shows.
    pub divider: bool,
    pub panes: Vec<Pane>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pane {
    pub look: Look,
    /// The starting size in pixels; without one the pane shares the rest.
    pub size: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    /// Keep its size when the window resizes, as a sidebar does.
    pub fixed: bool,
    pub visible: bool,
    /// Start what it mounts only once it first shows, and keep it, asleep,
    /// while it is hidden: neither its view nor its drawing runs.
    pub lazy: bool,
    pub frame: Frame,
    pub children: Vec<Node>,
}

impl Pane {
    /// Hidden and `lazy`: what it mounts sleeps.
    pub fn asleep(&self) -> bool {
        self.lazy && !self.visible
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub name: String,
    pub direction: Direction,
    pub gap: Space,
    pub justify: Justify,
}

/// Content that floats over everything else beside its anchor, such as a
/// sheet opened from a status bar item. While it is open, a click anywhere
/// outside it sends `dismiss`.
#[derive(Clone, Debug, PartialEq)]
pub struct Popover {
    /// The content's look; a `popover` rule styles and animates it.
    pub look: Look,
    pub open: bool,
    /// A message for the drawing mod's `update` when a click lands outside.
    pub dismiss: Option<Data>,
    /// The mod `dismiss` and `keys` messages go to, if not the drawing one.
    pub to: Option<String>,
    pub side: Side,
    pub align: Anchor,
    /// Pixels between the anchor and the content.
    pub gap: f32,
    pub width: Option<f32>,
    /// Keystrokes it takes while open and on top.
    pub keys: Keys,
    /// While open, the anchor stays above the click-outside layer, so a
    /// click on it reaches the anchor instead of sending `dismiss`: a
    /// field whose suggestions show under it while the user types.
    pub anchor_clicks: bool,
    /// What it opens from, drawn in place.
    pub anchor: Vec<Node>,
    pub content: Box<Node>,
}

/// Content floating over the whole window rather than beside an anchor: a
/// dialog, a command palette, a sheet along an edge, a toast in a corner.
/// Open overlays stack in tree order above popovers; the topmost one takes
/// its `keys`. A click on the backdrop, or anywhere outside the content,
/// sends `dismiss`; so does Escape, unless `keys` takes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    /// The content's look; an `overlay` rule styles and animates it.
    pub look: Look,
    pub open: bool,
    pub dismiss: Option<Data>,
    /// The mod `dismiss` and `keys` messages go to, if not the drawing one.
    pub to: Option<String>,
    pub place: Placement,
    /// Dims the window behind it (the `overlay-backdrop` part).
    pub backdrop: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub keys: Keys,
    pub content: Box<Node>,
}

/// Rows of the same height, of which the host draws only those that show,
/// so a list of any length costs what one screenful does. The view gives a
/// `row` function rather than the rows; it is called for the rows around
/// the ones showing, and the host asks again as the list scrolls.
#[derive(Clone, Debug, PartialEq)]
pub struct List {
    pub look: Look,
    pub frame: Frame,
    /// Names it among the mod's lists, so its place survives re-renders.
    pub id: String,
    /// How many rows there are in all.
    pub count: usize,
    /// Each row's height in pixels.
    pub row_height: f32,
    /// The position of `rows[0]`, from 0.
    pub start: usize,
    /// The rows from `start` the view built.
    pub rows: Vec<Node>,
}

/// The rows of a list a view builds: from `start`, `len` of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListWindow {
    pub start: usize,
    pub len: usize,
}

impl ListWindow {
    /// Rows built before the host says which show: a tall window's worth.
    pub const FIRST: Self = Self { start: 0, len: 64 };
    /// The most rows one window builds.
    pub const MAX: usize = 1000;

    /// The positions in it, of `count` rows.
    pub fn range(self, count: usize) -> std::ops::Range<usize> {
        let start = self.start.min(count);
        start..(start + self.len.min(Self::MAX)).min(count)
    }
}
