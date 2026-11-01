//! The element tree a module's `view` returns, checked into types the app can
//! draw without running Lua. Styles are theme tokens, never colors, so every
//! module follows the app's theme; sizes are pixels only where a layout
//! needs them (widths, heights, pane sizes).

use {
    crate::{Data, Look},
    std::path::{Path, PathBuf},
};

mod embed;
mod icon;
mod input;
mod instance;
mod layout;
mod read;
mod tokens;

pub use {embed::*, icon::*, input::*, instance::*, layout::*, tokens::*};

pub(crate) use read::read;

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

/// Markdown the host renders: headings, emphasis, lists, task lists,
/// tables, quotes and highlighted code. Links never open by themselves:
/// a click sends `link` with the address as `url`, for the mod to decide.
/// Pictures are not loaded, so a document cannot make the app fetch
/// anything.
#[derive(Clone, Debug, PartialEq)]
pub struct Markdown {
    pub look: Look,
    pub text: String,
    /// Sent when a link is clicked, shaped as [`Markdown::message`].
    pub link: Option<Data>,
    /// The mod `link` goes to; the drawing mod when unset.
    pub to: Option<String>,
    /// Take the free space along the parent's axis.
    pub grow: bool,
    /// Scroll its own content (give it `grow` or a `height`).
    pub scroll: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub max_width: Option<f32>,
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
