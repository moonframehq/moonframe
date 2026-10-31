//! The element tree a module's `view` returns, checked into types the app can
//! draw without running Lua. Styles are theme tokens, never colors, so every
//! module follows the app's theme; sizes are pixels only where a layout
//! needs them (widths, heights, pane sizes).

use crate::{Data, Look};

mod read;

pub(crate) use read::read;

tokens! {
    /// The element types `ui` builds; a table's `type` names one.
    Element {
        Row = "row",
        Column = "column",
        Panel = "panel",
        Hsplit = "hsplit",
        Vsplit = "vsplit",
        Pane = "pane",
        Text = "text",
        Button = "button",
        Item = "item",
        Badge = "badge",
        Icon = "icon",
        Terminal = "terminal",
        Divider = "divider",
        Spacer = "spacer",
        Region = "region",
        Mount = "mount",
        Popover = "popover",
    }
}

tokens! {
    /// A theme role for text and badges.
    Tone {
        Normal = "normal",
        Muted = "muted",
        Accent = "accent",
        Success = "success",
        Warning = "warning",
        Danger = "danger",
    }
}

tokens! {
    TextSize {
        Small = "small",
        Normal = "normal",
        Large = "large",
        Title = "title",
    }
}

tokens! {
    Weight {
        Normal = "normal",
        Medium = "medium",
        Bold = "bold",
    }
}

tokens! {
    /// A step on the app's spacing scale, for gaps and padding.
    Space {
        None = "none",
        Small = "small",
        Normal = "normal",
        Large = "large",
    }
}

impl Space {
    pub fn pixels(self) -> f32 {
        match self {
            Self::None => 0.,
            Self::Small => 4.,
            Self::Normal => 8.,
            Self::Large => 16.,
        }
    }
}

tokens! {
    /// Where children sit across a container's axis.
    Align {
        Stretch = "stretch",
        Start = "start",
        Center = "center",
        End = "end",
    }
}

tokens! {
    /// How children share a container's axis.
    Justify {
        Start = "start",
        Center = "center",
        End = "end",
        Between = "between",
    }
}

tokens! {
    /// A theme background.
    Surface {
        Background = "background",
        Sidebar = "sidebar",
        Muted = "muted",
        Secondary = "secondary",
    }
}

tokens! {
    /// The edges that get a border; `border = true` is all of them.
    Border {
        None = "none",
        All = "all",
        Top = "top",
        Bottom = "bottom",
        Left = "left",
        Right = "right",
    }
}

tokens! {
    Direction {
        Row = "row",
        Column = "column",
    }
}

tokens! {
    Variant {
        Normal = "normal",
        Primary = "primary",
        Ghost = "ghost",
        Outline = "outline",
        Danger = "danger",
    }
}

tokens! {
    /// Which side of its anchor a popover opens on.
    Side {
        Top = "top",
        Bottom = "bottom",
    }
}

tokens! {
    /// Which edge of its anchor a popover lines up with.
    Anchor {
        Start = "start",
        End = "end",
    }
}

tokens! {
    /// What the app does itself, rather than a module's `update`.
    Command {
        Quit = "quit",
        Reload = "reload",
        ToggleTheme = "toggle-theme",
        ShowThemes = "show-themes",
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Stack(Stack),
    Panel(Panel),
    Split(Split),
    Text(Text),
    Button(Button),
    Item(Item),
    Badge(Badge),
    Icon(Icon),
    Terminal(Terminal),
    Divider,
    Spacer,
    /// Every mod that names this region, in order.
    Region(Region),
    /// One mod's view, by name.
    Mount(String),
    Popover(Popover),
    /// A child that could not be read, drawn as a small marker in its
    /// place; the message says what is wrong.
    Problem(String),
}

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
    pub scroll: bool,
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
    /// What clicking anywhere in it does, as on a button.
    pub action: Option<Action>,
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
    pub frame: Frame,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub look: Look,
    pub text: String,
    pub tone: Option<Tone>,
    pub size: Option<TextSize>,
    pub weight: Option<Weight>,
    /// Cut to one line with an ellipsis rather than wrap.
    pub truncate: bool,
}

impl Text {
    /// A bare string in a container.
    pub(crate) fn plain(text: String) -> Self {
        Self {
            look: Look::default(),
            text,
            tone: None,
            size: None,
            weight: None,
            truncate: false,
        }
    }
}

/// What clicking a control does.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A message for the `update` of the mod named `to`, or of the mod that
    /// drew the control.
    Send {
        to: Option<String>,
        msg: Data,
    },
    Command(Command),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub look: Look,
    pub label: String,
    pub variant: Variant,
    pub small: bool,
    pub disabled: bool,
    pub action: Option<Action>,
}

/// A row in a list: a label, optional detail and badge, and a selection.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub look: Look,
    pub label: String,
    pub detail: Option<String>,
    pub badge: Option<String>,
    pub selected: bool,
    pub action: Option<Action>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Badge {
    pub look: Look,
    pub text: String,
    pub tone: Option<Tone>,
}

/// A symbol from the bundled set, drawn at the text's size and color.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub look: Look,
    pub name: IconName,
    pub tone: Option<Tone>,
    pub size: Option<TextSize>,
}

tokens! {
    /// The emulator a terminal runs on; both are built by default.
    TerminalEngine {
        /// `alacritty_terminal`, pure Rust.
        Alacritty = "alacritty",
        /// Ghostty's emulator, libghostty-vt.
        Ghostty = "ghostty",
    }
}

/// A shell the host runs on a pseudo-terminal. The host keeps it by the
/// mod and `id`, so it lives across reloads and while its pane is hidden;
/// a mod picks where it starts, never what runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Terminal {
    pub look: Look,
    pub id: String,
    pub cwd: Option<String>,
    pub engine: TerminalEngine,
}

/// One of [`ICONS`]; anything else is refused when the view is read, so a
/// typo is an error rather than a blank space.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconName(&'static str);

impl IconName {
    pub fn new(name: &str) -> Option<Self> {
        ICONS
            .binary_search(&name)
            .ok()
            .map(|index| Self(ICONS[index]))
    }

    pub fn name(&self) -> &'static str {
        self.0
    }

    /// Where the app's assets keep it.
    pub fn path(&self) -> String {
        format!("icons/{}.svg", self.0)
    }
}

/// The icons the app bundles (Lucide's, as gpui-component ships them), sorted.
pub const ICONS: &[&str] = &[
    "a-large-small",
    "arrow-down",
    "arrow-left",
    "arrow-right",
    "arrow-up",
    "asterisk",
    "ban",
    "battery",
    "battery-charging",
    "battery-full",
    "battery-low",
    "battery-medium",
    "battery-warning",
    "bell",
    "book-open",
    "bot",
    "building-2",
    "calendar",
    "case-sensitive",
    "chart-pie",
    "check",
    "chevron-down",
    "chevron-left",
    "chevron-right",
    "chevron-up",
    "chevrons-up-down",
    "circle-alert",
    "circle-check",
    "circle-user",
    "circle-x",
    "close",
    "copy",
    "cpu",
    "dash",
    "delete",
    "ellipsis",
    "ellipsis-vertical",
    "external-link",
    "eye",
    "eye-off",
    "file",
    "file-text",
    "folder",
    "folder-closed",
    "folder-open",
    "frame",
    "gallery-vertical-end",
    "github",
    "globe",
    "hard-drive",
    "heart",
    "heart-off",
    "inbox",
    "info",
    "inspector",
    "layout-dashboard",
    "loader",
    "loader-circle",
    "map",
    "maximize",
    "memory-stick",
    "menu",
    "mic",
    "minimize",
    "minus",
    "moon",
    "network",
    "palette",
    "panel-bottom",
    "panel-bottom-open",
    "panel-left",
    "panel-left-close",
    "panel-left-open",
    "panel-right",
    "panel-right-close",
    "panel-right-open",
    "pause",
    "play",
    "plus",
    "redo",
    "redo-2",
    "refresh-cw",
    "replace",
    "resize-corner",
    "rotate-cw",
    "search",
    "settings",
    "settings-2",
    "sort-ascending",
    "sort-descending",
    "square",
    "square-terminal",
    "star",
    "star-fill",
    "star-off",
    "sun",
    "thumbs-down",
    "thumbs-up",
    "triangle-alert",
    "undo",
    "undo-2",
    "user",
    "window-close",
    "window-maximize",
    "window-minimize",
    "window-restore",
];

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
    pub side: Side,
    pub align: Anchor,
    /// Pixels between the anchor and the content.
    pub gap: f32,
    pub width: Option<f32>,
    /// What it opens from, drawn in place.
    pub anchor: Vec<Node>,
    pub content: Box<Node>,
}
