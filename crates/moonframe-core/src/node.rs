//! The element tree a module's `view` returns, checked into types the app can
//! draw without running Lua. Styles are theme tokens, never colors, so every
//! module follows the app's theme; sizes are pixels only where a layout
//! needs them (widths, heights, pane sizes).

use {
    crate::{Data, Keys, Look},
    std::path::{Path, PathBuf},
};

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
        Image = "image",
        Line = "line",
        Hatch = "hatch",
        Terminal = "terminal",
        Browser = "browser",
        View = "view",
        Input = "input",
        Divider = "divider",
        Spacer = "spacer",
        Region = "region",
        Mount = "mount",
        Instance = "instance",
        Popover = "popover",
        Overlay = "overlay",
        List = "list",
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
        Left = "left",
        Right = "right",
    }
}

tokens! {
    /// Which edge of its anchor a popover lines up with: the left or right
    /// one above or below it, the top or bottom one beside it.
    Anchor {
        Start = "start",
        End = "end",
    }
}

tokens! {
    /// Where an overlay sits in the window.
    Placement {
        /// In the middle: a dialog or a command palette.
        Center = "center",
        /// Centered across, hanging from near the top: a palette.
        Top = "top",
        /// Sheets along an edge, as wide or as tall as the window.
        Bottom = "bottom",
        Left = "left",
        Right = "right",
        /// In a corner: toasts and notices.
        TopStart = "top-start",
        TopEnd = "top-end",
        BottomStart = "bottom-start",
        BottomEnd = "bottom-end",
    }
}

tokens! {
    /// What the app does itself, rather than a module's `update`.
    Command {
        Quit = "quit",
        Reload = "reload",
        ToggleTheme = "toggle-theme",
        /// Minimizes the window, for a title bar the app draws itself.
        Minimize = "minimize",
        /// Zooms (macOS) or maximizes the window, or restores it.
        Maximize = "maximize",
        CloseWindow = "close-window",
        ShowThemes = "show-themes",
        /// Starts Inspect Element for the mod that asked: the pick, or
        /// Escape, comes back to it on the topic `app/inspect`.
        Inspect = "inspect",
    }
}

tokens! {
    /// What a control can ask of a `ui.browser`, named by its `browser` key.
    BrowserCommand {
        Back = "browser-back",
        Forward = "browser-forward",
        Reload = "browser-reload",
        Stop = "browser-stop",
        /// Opens the page's address in the system browser.
        Open = "browser-open",
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
    Image(Image),
    Line(Line),
    Hatch(Hatch),
    Terminal(Terminal),
    Browser(Browser),
    /// A view the app writes in Rust, by name.
    View(View),
    Input(Input),
    Divider,
    Spacer,
    /// Every mod that names this region, in order.
    Region(Region),
    /// One mod's view, by name.
    Mount(String),
    /// A mod or component with state of its own, kept by its key.
    Instance(Instance),
    Popover(Popover),
    Overlay(Overlay),
    /// Rows the host draws only while they show.
    List(List),
    /// A child that could not be read, drawn as a small marker in its
    /// place; the message says what is wrong.
    Problem(String),
}

impl Node {
    /// Calls `found` with each instance in the tree, outermost first. It does
    /// not look inside instances: each draws its own view.
    pub fn instances(&self, found: &mut dyn FnMut(&Instance)) {
        let all = |nodes: &[Node], found: &mut dyn FnMut(&Instance)| {
            for node in nodes {
                node.instances(found);
            }
        };
        match self {
            Self::Instance(instance) => found(instance),
            Self::Stack(stack) => all(&stack.children, found),
            Self::Panel(panel) => {
                all(&panel.actions, found);
                all(&panel.children, found);
            },
            Self::Split(split) => {
                for pane in split.panes.iter().filter(|pane| !pane.asleep()) {
                    all(&pane.children, found);
                }
            },
            Self::Popover(popover) => {
                all(&popover.anchor, found);
                popover.content.instances(found);
            },
            Self::Overlay(overlay) => overlay.content.instances(found),
            Self::List(list) => all(&list.rows, found),
            _ => {},
        }
    }

    /// Whether the tree holds what the stylesheet animates as it shows and
    /// hides: a popover, an overlay or a split's panes.
    pub fn moves(&self) -> bool {
        let any = |nodes: &[Node]| nodes.iter().any(Node::moves);
        match self {
            Self::Popover(_) | Self::Overlay(_) | Self::Split(_) => true,
            Self::Stack(stack) => any(&stack.children),
            Self::Panel(panel) => any(&panel.actions) || any(&panel.children),
            Self::List(list) => any(&list.rows),
            _ => false,
        }
    }

    /// Calls `found` with each instance inside a hidden `lazy` pane: they
    /// are kept as they are, neither started nor drawn, until it shows.
    pub fn asleep(&self, found: &mut dyn FnMut(&Instance)) {
        let all = |nodes: &[Node], found: &mut dyn FnMut(&Instance)| {
            for node in nodes {
                node.asleep(found);
            }
        };
        match self {
            Self::Stack(stack) => all(&stack.children, found),
            Self::Panel(panel) => {
                all(&panel.actions, found);
                all(&panel.children, found);
            },
            Self::Split(split) => {
                for pane in &split.panes {
                    if pane.asleep() {
                        for node in &pane.children {
                            node.instances(found);
                            node.asleep(found);
                        }
                    } else {
                        all(&pane.children, found);
                    }
                }
            },
            Self::Popover(popover) => {
                all(&popover.anchor, found);
                popover.content.asleep(found);
            },
            Self::Overlay(overlay) => overlay.content.asleep(found),
            Self::List(list) => all(&list.rows, found),
            _ => {},
        }
    }
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
    pub scroll: Scroll,
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
pub struct Text {
    pub look: Look,
    pub text: String,
    pub tone: Option<Tone>,
    pub size: Option<TextSize>,
    pub weight: Option<Weight>,
    /// Cut to one line with an ellipsis rather than wrap.
    pub truncate: bool,
    /// Wrap to at most this many lines, then an ellipsis.
    pub lines: Option<u8>,
    /// Wrap long lines (the default); `false` keeps each line whole, for
    /// code in a container that scrolls sideways.
    pub wrap: bool,
    /// A line through it: a task done, a price struck.
    pub strike: bool,
    pub extras: Extras,
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
            lines: None,
            wrap: true,
            strike: false,
            extras: Extras::default(),
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
    /// Goes back, forward, reloads or stops the `ui.browser` `id` of the mod
    /// named `to`, or of the mod that drew the control.
    Browser {
        to: Option<String>,
        id: String,
        command: BrowserCommand,
    },
    /// Opens the mod named this in a window of its own, or raises it: a
    /// tool that stays open beside the main window.
    Window(String),
}

/// What an element does besides its click: on stacks, buttons and items,
/// and on texts, badges, icons and images, which take no click.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extras {
    /// Text shown under the pointer after it rests on the control.
    pub tooltip: Option<String>,
    /// What a right-click (or a two-finger click) does: open a context menu.
    pub context: Option<Action>,
    /// Text a click puts on the clipboard, beside any `msg`.
    pub copy: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub look: Look,
    /// Empty for an icon-only button.
    pub label: String,
    /// Drawn before the label.
    pub icon: Option<IconName>,
    pub variant: Variant,
    pub small: bool,
    pub disabled: bool,
    pub action: Option<Action>,
    pub extras: Extras,
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
    pub extras: Extras,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Badge {
    pub look: Look,
    pub text: String,
    pub tone: Option<Tone>,
    pub extras: Extras,
}

/// A symbol from the bundled set, drawn at the text's size and color.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub look: Look,
    pub name: IconName,
    pub tone: Option<Tone>,
    pub size: Option<TextSize>,
    /// Turns without end, for a loader; `animation_speed` paces it.
    pub spin: bool,
    pub extras: Extras,
}

tokens! {
    /// How a picture fills its box, as CSS `object-fit`.
    Fit {
        /// All of it, as large as fits, keeping its shape.
        Contain = "contain",
        /// The whole box, keeping its shape and cropping what overflows.
        Cover = "cover",
        /// The whole box, stretched.
        Fill = "fill",
        /// As `contain`, but never larger than it is.
        ScaleDown = "scale-down",
        /// At its own size, centered and cropped.
        None = "none",
    }
}

/// A picture from a file. A missing or unreadable one leaves an empty box
/// of its size rather than an error, since files come and go.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub look: Look,
    /// As written: relative to the drawing mod's folder, `~/...`, or
    /// absolute, inside the app's folders or the image cache. See
    /// [`Image::resolve`].
    pub src: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub fit: Fit,
    pub extras: Extras,
}

/// A line through values, evenly spaced across its box: a trend, a curve
/// in a table, a forecast. Its look's `color` strokes it, its `background`
/// fills the area between it and the `baseline`, and its `border_color`
/// draws the baseline, dashed.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub look: Look,
    pub points: Vec<f32>,
    /// The values at the bottom and top of the box; the points' (and the
    /// baseline's) least and most when unset.
    pub min: Option<f32>,
    pub max: Option<f32>,
    /// A value drawn across as a dashed line, with the area filled to it.
    pub baseline: Option<f32>,
    /// Fill the area between the line and the baseline (or the bottom).
    pub area: bool,
    /// Mark the points: `last`, `all`, or none.
    pub dots: Option<Dots>,
    /// The stroke's width in pixels.
    pub stroke: f32,
    /// Draw the line itself dashed.
    pub dashed: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

tokens! {
    /// Which points of a `ui.line` get a dot.
    Dots {
        Last = "last",
        All = "all",
    }
}

/// A box filled with diagonal stripes in its look's `color` over its
/// `background`: what is held back, pending, or not there yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Hatch {
    pub look: Look,
    /// Pixels between stripes.
    pub gap: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub grow: bool,
}

/// Largest picture file `ui.image` opens.
pub const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

impl Image {
    /// The file `src` names, drawn by a mod in `folder`; `home` expands `~`.
    /// Only a regular file of at most [`MAX_IMAGE_BYTES`] under one of
    /// `allowed` (the app's folders and the cache providers download to),
    /// after following links and `..`, so a view cannot show any file on
    /// the machine; anything else is `None`, an empty box.
    pub fn resolve(
        &self,
        folder: &Path,
        home: Option<&Path>,
        allowed: &[PathBuf],
    ) -> Option<PathBuf> {
        let src = Path::new(&self.src);
        let path = match (home, src.strip_prefix("~")) {
            (Some(home), Ok(rest)) => home.join(rest),
            _ if src.is_absolute() => src.to_owned(),
            _ => folder.join(src),
        };
        let real = path.canonicalize().ok()?;
        let metadata = real.metadata().ok()?;
        if !metadata.is_file() || metadata.len() > MAX_IMAGE_BYTES {
            return None;
        }
        allowed
            .iter()
            .filter_map(|root| root.canonicalize().ok())
            .any(|root| real.starts_with(root))
            .then_some(real)
    }
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

tokens! {
    /// What keeps a terminal's shell running.
    TerminalBackend {
        /// The best this machine has: herdr-core, then tmux, then a pty.
        Auto = "auto",
        /// herdr-core (`hc`): the shell outlives the app.
        HerdrCore = "herdr-core",
        /// A tmux session on the app's own server: the shell outlives the app.
        Tmux = "tmux",
        /// A local pseudo-terminal: the shell ends with the app.
        Pty = "pty",
    }
}

/// A shell the host runs. The host keeps it by the mod and `id`, so it
/// lives across reloads and while its pane is hidden, and with herdr-core
/// or tmux across launches; a mod picks where it starts, never what runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Terminal {
    pub look: Look,
    pub id: String,
    pub cwd: Option<String>,
    pub engine: TerminalEngine,
    pub backend: TerminalBackend,
}

/// A web page the host shows in a native web view (WebKit on macOS,
/// WebView2 on Windows), kept by the mod and `id` like a terminal. The mod
/// gives the address; it never runs scripts in the page.
#[derive(Clone, Debug, PartialEq)]
pub struct Browser {
    pub look: Look,
    pub id: String,
    /// Where to go: loaded when it differs from the last one the mod gave,
    /// so the user's own navigation stands until the mod asks again.
    pub url: Option<String>,
    /// The page's zoom factor; 1 is its own size.
    pub zoom: f32,
    /// Sent to the mod as the page goes somewhere or its title changes,
    /// with `url` and `title` added.
    pub navigate: Option<Data>,
    /// Sent as a load starts and finishes, with `loading` and `url` added.
    pub loading: Option<Data>,
}

/// A view the app on Moonframe writes in Rust and registers by `name`
/// (`Moonframe::view`), kept by the mod and `id` like a terminal; the host
/// hands it `props` and tells it when they change. Only the app's own
/// views: Luau never names a Rust type.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub look: Look,
    pub name: String,
    pub id: String,
    pub props: Data,
}

/// Whether `url` is an `http` or `https` address: what may go to the system
/// browser or open from a page's popup. A `file` address handed to the
/// system opens whatever program the file is, so it never goes there.
pub fn is_remote_url(url: &str) -> bool {
    is_web_url(url)
        && url.split_once("://").is_some_and(|(scheme, _)| {
            matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
        })
}

/// Whether `url` is a `file` address.
pub fn is_file_url(url: &str) -> bool {
    url.split_once("://")
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
}

/// Whether a browser may show `url`: an `http`, `https` or `file` address,
/// one line without spaces, as a mod writes it or a page navigates.
pub fn is_web_url(url: &str) -> bool {
    const MAX: usize = 8192;
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    let host_given = match scheme.as_str() {
        "http" | "https" => rest
            .split(['/', '?', '#'])
            .next()
            .is_some_and(|host| !host.is_empty()),
        // `file:///path`, or `file://localhost/path`.
        "file" => rest.starts_with('/') || rest.starts_with("localhost/"),
        _ => false,
    };
    host_given && url.len() <= MAX && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// Where text typed in a browser's address field goes, as a browser does
/// it: an address as written, a host (`example.com`, `localhost:3000`)
/// over https (http for the machine itself), and anything else as a web
/// search. None for blank text.
pub fn typed_address(text: &str) -> Option<String> {
    const SEARCH: &str = "https://duckduckgo.com/?q=";
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.contains("://") {
        return is_web_url(text).then(|| text.to_owned());
    }
    if !text.chars().any(char::is_whitespace) {
        let host = text.split(['/', '?', '#']).next().unwrap_or_default();
        let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
        let local =
            matches!(name, "localhost" | "127.0.0.1" | "[::1]") || name.ends_with(".localhost");
        if local || name.contains('.') && !name.starts_with('.') && !name.ends_with('.') {
            let scheme = if local {
                "http"
            } else {
                "https"
            };
            let url = format!("{scheme}://{text}");
            if is_web_url(&url) {
                return Some(url);
            }
        }
    }
    let mut query = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                query.push(byte as char);
            },
            b' ' => query.push('+'),
            _ => query.push_str(&format!("%{byte:02X}")),
        }
    }
    Some(format!("{SEARCH}{query}")).filter(|url| is_web_url(url))
}

/// A text field the host edits: single-line, or several lines with
/// `multiline`. The host keeps what the user typed by the drawing mod and
/// `id` (or the field's place in the tree), so it survives re-renders and
/// reloads; `value` replaces it only when the mod passes a new one.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub look: Look,
    /// Keeps the text across renders; without one, the field's path.
    pub id: Option<String>,
    /// The text to show, applied when it differs from the last value the
    /// mod passed, so a mod can set or clear it; otherwise the user's text
    /// stands.
    pub value: Option<String>,
    pub placeholder: Option<String>,
    /// The mod the messages go to; the drawing mod when unset.
    pub to: Option<String>,
    /// Sent on each edit, shaped by [`Input::message`].
    pub change: Option<Data>,
    /// Sent on Enter (single-line), or ⌘/Ctrl-Enter (multiline).
    pub submit: Option<Data>,
    /// Keys that send a message instead of editing while the field has focus.
    pub keys: Vec<(KeyChord, Data)>,
    pub multiline: bool,
    /// Visible lines of a multiline field; it scrolls past them.
    pub rows: u32,
    pub disabled: bool,
    /// Take keyboard focus when it appears, or when this turns true.
    pub focus: bool,
    /// Use the monospace font.
    pub mono: bool,
    /// The `ui.browser` this field is the address bar of: Enter loads the
    /// typed address there (see [`typed_address`]), focus selects the
    /// text, and Escape puts back the page's address.
    pub browser: Option<String>,
    pub width: Option<f32>,
    pub grow: bool,
}

impl Input {
    /// Lines a multiline field shows without `rows`.
    pub const DEFAULT_ROWS: u32 = 3;
    /// Most `rows` a field can ask for.
    pub const MAX_ROWS: u32 = 200;

    /// What the field sends for `msg` when its text is `value`: a table
    /// `msg` is copied with `value` set; anything else becomes
    /// `{ msg = msg, value = value }`.
    pub fn message(msg: &Data, value: &str) -> Data {
        let value = Data::String(value.to_owned());
        match msg {
            Data::Map(entries) => {
                let mut entries = entries.clone();
                entries.insert("value".to_owned(), value);
                Data::Map(entries)
            },
            other => Data::Map(
                [
                    ("msg".to_owned(), other.clone()),
                    ("value".to_owned(), value),
                ]
                .into(),
            ),
        }
    }
}

/// A key and the modifiers held with it, named as GPUI names keystrokes:
/// `up`, `escape`, `shift-tab`, `ctrl-n`, `secondary-enter`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyChord {
    /// GPUI's name for the key: a lowercase letter, digit or punctuation
    /// mark, or one of [`KEY_NAMES`].
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// ⌘ on macOS, the Windows or Super key elsewhere.
    pub cmd: bool,
    /// ⌘ on macOS, Ctrl elsewhere: the one to use for shortcuts that work
    /// on every platform.
    pub secondary: bool,
}

/// The modifiers a [`KeyChord`] can name.
pub const KEY_MODIFIERS: &[&str] = &["ctrl", "alt", "shift", "cmd", "secondary"];

/// Keys with a name rather than a character.
pub const KEY_NAMES: &[&str] = &[
    "up",
    "down",
    "left",
    "right",
    "escape",
    "tab",
    "enter",
    "backspace",
    "delete",
    "insert",
    "home",
    "end",
    "pageup",
    "pagedown",
    "space",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

impl KeyChord {
    /// Reads `ctrl-shift-n`: modifiers, each at most once, then the key.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut chord = Self {
            key: String::new(),
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
            secondary: false,
        };
        // A trailing `-` after a separator is the minus key (`ctrl--`).
        let (modifiers, key) = match text.strip_suffix("--") {
            Some(rest) => (Some(rest), "-"),
            None => match text.rsplit_once('-') {
                Some((modifiers, key)) if !modifiers.is_empty() => (Some(modifiers), key),
                _ => (None, text),
            },
        };
        for modifier in modifiers
            .into_iter()
            .flat_map(|modifiers| modifiers.split('-'))
        {
            let held = match modifier {
                "ctrl" => &mut chord.ctrl,
                "alt" => &mut chord.alt,
                "shift" => &mut chord.shift,
                "cmd" => &mut chord.cmd,
                "secondary" => &mut chord.secondary,
                _ => {
                    return Err(format!(
                        "unknown modifier `{modifier}`; use {}",
                        KEY_MODIFIERS.join(", ")
                    ));
                },
            };
            if *held {
                return Err(format!("`{modifier}` twice"));
            }
            *held = true;
        }
        let mut chars = key.chars();
        let character = match (chars.next(), chars.next()) {
            (Some(c), None) => c.is_ascii_graphic() && !c.is_ascii_uppercase(),
            _ => false,
        };
        if !character && !KEY_NAMES.contains(&key) {
            return Err(format!(
                "unknown key `{key}`; use a lowercase letter, digit or mark, or one of {}",
                KEY_NAMES.join(", ")
            ));
        }
        chord.key = key.to_owned();
        Ok(chord)
    }
}

/// What a component or mod offers a tab bar: the `+` menu lists it, and a
/// new tab of it starts with `props`.
#[derive(Clone, Debug, PartialEq)]
pub struct TabKind {
    pub title: Option<String>,
    pub icon: Option<IconName>,
    pub props: Data,
    /// The prop a tab hands it as a message for reports, such as a browser's
    /// `navigate`: what comes back (`url`, `title`) is kept in the tab.
    pub report: Option<String>,
}

/// What an instance runs: a mod, as `ui.mount { "explorer", id = ... }`
/// mounts it, or a component that keeps state, as `use` mounts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstanceOf {
    Mod(String),
    Component(String),
}

/// A mod or component instance in a view: its own state, kept by the app
/// under [`Instance::key`], and drawn where this node is.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub of: InstanceOf,
    pub id: String,
    /// What it is drawn with, as its `init`, `update` and view see it.
    pub props: Data,
}

impl Instance {
    /// Where the app keeps it. A mod instance is the app's own (`explorer#2`,
    /// the same wherever it is mounted); a component's belongs to the module
    /// that drew it (`editor/tabbed#main`), so two mods' tab bars differ.
    pub fn key(&self, owner: &str) -> String {
        match &self.of {
            // A preview's instances stay inside it: never a real one's state.
            InstanceOf::Mod(name) if owner.starts_with(crate::workspace::DEMO) => {
                format!("{owner}/{name}#{}", self.id)
            },
            InstanceOf::Mod(name) => format!("{name}#{}", self.id),
            InstanceOf::Component(name) => format!("{owner}/{name}#{}", self.id),
        }
    }
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

/// The icons the app bundles, sorted: gpui-kit's Lucide set, and the app's
/// own in `assets/icons` (more of Lucide, and `agent-*` and `brand-*` logos).
pub const ICONS: &[&str] = &[
    "a-large-small",
    "activity",
    "agent-amp",
    "agent-claude",
    "agent-codex",
    "agent-copilot",
    "agent-cursor",
    "agent-droid",
    "agent-gemini",
    "agent-grok",
    "agent-kiro",
    "agent-opencode",
    "agent-qwen",
    "arrow-down",
    "arrow-left",
    "arrow-right",
    "arrow-up",
    "arrow-up-down",
    "asterisk",
    "at-sign",
    "ban",
    "battery",
    "battery-charging",
    "battery-full",
    "battery-low",
    "battery-medium",
    "battery-warning",
    "bell",
    "bell-off",
    "book-open",
    "bookmark",
    "bot",
    "brand-anthropic",
    "brand-github",
    "brand-gitlab",
    "brand-mistral",
    "brand-ollama",
    "brand-openai",
    "bug",
    "building-2",
    "calendar",
    "case-sensitive",
    "chart-column",
    "chart-pie",
    "check",
    "chevron-down",
    "chevron-left",
    "chevron-right",
    "chevron-up",
    "chevrons-left",
    "chevrons-right",
    "chevrons-up-down",
    "circle",
    "circle-alert",
    "circle-check",
    "circle-dashed",
    "circle-dot",
    "circle-help",
    "circle-pause",
    "circle-play",
    "circle-question-mark",
    "circle-stop",
    "circle-user",
    "circle-x",
    "clock",
    "close",
    "cloud",
    "code",
    "coffee",
    "columns-2",
    "command",
    "copy",
    "cpu",
    "dash",
    "delete",
    "dot",
    "download",
    "ellipsis",
    "ellipsis-vertical",
    "external-link",
    "eye",
    "eye-off",
    "file",
    "file-text",
    "filter",
    "flag",
    "folder",
    "folder-closed",
    "folder-git-2",
    "folder-open",
    "frame",
    "funnel",
    "gallery-vertical-end",
    "git-branch",
    "git-commit-horizontal",
    "git-compare",
    "git-merge",
    "git-pull-request",
    "git-pull-request-closed",
    "git-pull-request-draft",
    "github",
    "gitlab",
    "globe",
    "grip-vertical",
    "hard-drive",
    "hash",
    "heart",
    "heart-off",
    "history",
    "house",
    "inbox",
    "info",
    "inspector",
    "key-round",
    "keyboard",
    "laptop",
    "layers",
    "layout-dashboard",
    "link",
    "list",
    "list-checks",
    "list-tree",
    "loader",
    "loader-circle",
    "lock",
    "lock-open",
    "log-out",
    "mail",
    "map",
    "maximize",
    "maximize-2",
    "memory-stick",
    "menu",
    "message-square",
    "mic",
    "minimize",
    "minimize-2",
    "minus",
    "monitor",
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
    "pencil",
    "pin",
    "pin-off",
    "play",
    "plug",
    "plus",
    "power",
    "redo",
    "redo-2",
    "refresh-ccw",
    "refresh-cw",
    "replace",
    "resize-corner",
    "rocket",
    "rotate-ccw-clock",
    "rotate-cw",
    "rows-2",
    "search",
    "send",
    "server",
    "settings",
    "settings-2",
    "shield",
    "sort-ascending",
    "sort-descending",
    "sparkles",
    "split-square-horizontal",
    "split-square-vertical",
    "square",
    "square-check",
    "square-split-horizontal",
    "square-split-vertical",
    "square-terminal",
    "star",
    "star-fill",
    "star-off",
    "sun",
    "tag",
    "terminal",
    "thumbs-down",
    "thumbs-up",
    "trash",
    "trash-2",
    "triangle-alert",
    "undo",
    "undo-2",
    "unlock",
    "upload",
    "user",
    "wifi",
    "wifi-off",
    "window-close",
    "window-maximize",
    "window-minimize",
    "window-restore",
    "workflow",
    "zap",
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
    /// The mod `dismiss` and `keys` messages go to, if not the drawing one.
    pub to: Option<String>,
    pub side: Side,
    pub align: Anchor,
    /// Pixels between the anchor and the content.
    pub gap: f32,
    pub width: Option<f32>,
    /// Keystrokes it takes while open and on top.
    pub keys: Keys,
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
