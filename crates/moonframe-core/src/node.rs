//! The element tree a module's `view` returns, checked into types the app can
//! draw without running Lua. Styles are theme tokens, never colors, so every
//! module follows the app's theme; sizes are pixels only where a layout
//! needs them (widths, heights, pane sizes).

use {
    crate::{Data, Look},
    std::path::{Path, PathBuf},
};

// keep-sorted start
mod embed;
mod gesture;
mod icon;
mod input;
mod instance;
mod layout;
mod map;
mod pointer;
mod read;
mod revision_timeline;
mod snapshot;
mod tokens;
mod video;
// keep-sorted end

pub use {
    embed::*, gesture::*, icon::*, input::*, instance::*, layout::*, map::*, pointer::*,
    revision_timeline::*, snapshot::*, tokens::*, video::*,
};

pub(crate) use read::{element_keys, read};

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
    Paths(Paths),
    RevisionTimeline(RevisionTimeline),
    Hatch(Hatch),
    /// A street map from a tile provider, with pins and routes.
    Map(Map),
    Terminal(Terminal),
    Browser(Browser),
    /// Frames from another process, from a ring in shared memory.
    Video(Video),
    /// A view the app writes in Rust, by name.
    View(View),
    Input(Input),
    Markdown(Markdown),
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
            Self::Image(image) => all(&image.overlay, found),
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
            Self::Image(image) => all(&image.overlay, found),
            _ => {},
        }
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
    /// Colored as code in this language, a line at a time.
    pub syntax: Option<String>,
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
            syntax: None,
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
    /// Screen pixels per picture pixel (1 is 100 %), panned to `center`:
    /// a viewer's zoom. Unset, the picture fits its box as `fit` says.
    pub zoom: Option<f32>,
    /// The point of the picture at the middle of the box while zoomed, as
    /// fractions across and down it; the middle when unset.
    pub center: Option<[f32; 2]>,
    /// Sent with the picture's own size and the zoom that fits it (`{
    /// width, height, fit }`) when the picture or its box changes.
    pub measure: Option<Data>,
    /// The mod `measure` and `context` go to; the drawing mod when unset.
    pub to: Option<String>,
    /// Drawn over the picture itself, wherever it is shown in its box, so
    /// an `area` there is in fractions of the picture: a crop frame, marks.
    pub overlay: Vec<Node>,
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

/// Lines through points the view places itself, in pixels from the box's
/// top-left corner: arrows between dependent bars on a timeline, the
/// connectors of a diagram. Each is stroked in its own `color`, else the
/// look's `color`, else the theme's border color.
#[derive(Clone, Debug, PartialEq)]
pub struct Paths {
    pub look: Look,
    pub paths: Vec<Polyline>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

/// One line of a `ui.paths`.
#[derive(Clone, Debug, PartialEq)]
pub struct Polyline {
    /// Its corners in order, `{ x, y }` in pixels.
    pub points: Vec<[f32; 2]>,
    pub color: Option<crate::Color>,
    /// The stroke's width in pixels.
    pub stroke: f32,
    pub dashed: bool,
    /// Rounds each corner by this many pixels, as an elbow connector does.
    pub radius: f32,
    /// Which ends get an arrowhead.
    pub arrow: Option<Arrow>,
}

tokens! {
    /// Which ends of a `ui.paths` line get an arrowhead.
    Arrow {
        End = "end",
        Start = "start",
        Both = "both",
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

/// Markdown the host renders: headings, emphasis, lists, task lists,
/// tables, quotes, alerts, footnotes and highlighted code. Links never open
/// by themselves: a click sends `link` with the address as `url`, for the
/// mod to decide, and a `#heading` link scrolls to its heading. Pictures
/// show only with `images`, and then only files a `ui.image` could, so a
/// document cannot make the app fetch anything.
#[derive(Clone, Debug, PartialEq)]
pub struct Markdown {
    pub look: Look,
    pub text: String,
    /// Sent when a link is clicked, shaped as [`Markdown::message`].
    pub link: Option<Data>,
    /// The mod its messages go to; the drawing mod when unset.
    pub to: Option<String>,
    /// Take the free space along the parent's axis.
    pub grow: bool,
    /// Scroll its own content (give it `grow` or a `height`).
    pub scroll: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub max_width: Option<f32>,
    /// Show at most this many lines of body text (not with `scroll`).
    pub lines: Option<usize>,
    /// Show pictures from files, as `ui.image` would.
    pub images: bool,
    /// Sent when a picture on a line of its own is clicked, with `src`,
    /// its address, and `alt`.
    pub image: Option<Data>,
    /// Sent with `headings` each time the document's headings change.
    pub outline: Option<Data>,
    /// Sent with `heading`, the index of the one being read, as a
    /// scrolling document passes it.
    pub section: Option<Data>,
    /// Where to scroll to; a new one scrolls.
    pub reveal: Option<Reveal>,
    /// Text to highlight wherever it shows.
    pub find: Option<String>,
    /// The match to mark as current and scroll to, from 1.
    pub find_index: Option<usize>,
    /// Sent with `count` each time the number of matches changes.
    pub found: Option<Data>,
    /// Fade in what is added to `text`, as an answer streams in.
    pub stream: bool,
}

/// A place in a document to scroll to: bytes into its text, as `headings`
/// gives them. `seq` tells two requests for one place apart, as the same
/// heading can be picked again after scrolling away.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reveal {
    pub at: usize,
    pub seq: i64,
}

impl Markdown {
    /// What a click on the link to `url` sends for `msg`: a table `msg`
    /// is copied with `url` set; anything else becomes `{ msg = msg, url = url }`.
    pub fn message(msg: &Data, url: &str) -> Data {
        let url = Data::String(url.to_owned());
        match msg {
            Data::Map(entries) => {
                let mut entries = entries.clone();
                entries.insert("url".to_owned(), url);
                Data::Map(entries)
            },
            other => Data::Map([("msg".to_owned(), other.clone()), ("url".to_owned(), url)].into()),
        }
    }

    /// `msg` with `fields` added, in the shape of [`Markdown::message`].
    pub fn note<const N: usize>(msg: &Data, fields: [(&str, Data); N]) -> Data {
        let mut entries = match msg {
            Data::Map(entries) => entries.clone(),
            other => [("msg".to_owned(), other.clone())].into(),
        };
        entries.extend(fields.map(|(key, value)| (key.to_owned(), value)));
        Data::Map(entries)
    }
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
        resolve_file(&self.src, folder, home, allowed, MAX_IMAGE_BYTES)
    }
}

/// The file `src` names for a mod in `folder` (relative to it, `~/...`
/// with `home`, or absolute), if it is a regular file of at most
/// `max_bytes` under one of `allowed` after following links and `..`: so
/// a mod reaches only the app's own files.
pub fn resolve_file(
    src: &str,
    folder: &Path,
    home: Option<&Path>,
    allowed: &[PathBuf],
    max_bytes: u64,
) -> Option<PathBuf> {
    let src = Path::new(src);
    let path = match (home, src.strip_prefix("~")) {
        (Some(home), Ok(rest)) => home.join(rest),
        _ if src.is_absolute() => src.to_owned(),
        _ => folder.join(src),
    };
    let real = path.canonicalize().ok()?;
    let metadata = real.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return None;
    }
    allowed
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| real.starts_with(root))
        .then_some(real)
}
