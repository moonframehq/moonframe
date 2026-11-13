//! Inspect: pick any element in the window, as in a browser's developer
//! tools, to tell an agent where to change or add something. While picking,
//! every drawn node is wrapped in [`Tracked`], which records where it landed
//! without adding a layout box; the pointer then finds the deepest node
//! under it, and ↑ and ↓ step out to the nodes around it and back
//! ([`aim`]). The pick names the mod, its file and line, and the elements
//! around it, with the live values the mod drew it from ([`context`]), and
//! is saved where agents read it (`paths::selection`), kept current while
//! it stays selected.

use {
    crate::paths,
    gpui_kit::*,
    moonframe_core::{Node, Workspace},
    std::{cell::RefCell, fmt::Write as _, path::Path, rc::Rc},
};

mod aim;
mod context;

pub(crate) use aim::Target;

/// Longest text quoted in an element's label.
const QUOTE: usize = 40;

/// One node on the way down to an element: what it is and where it was
/// built. A mod's root is marked with the mod, so a pick knows whose file
/// its line is in.
#[derive(Debug)]
pub(crate) struct Crumb {
    pub(crate) label: String,
    pub(crate) line: Option<u32>,
    pub(crate) owner: Option<String>,
    /// The node's element path, as its [`Placed`] records it; none for a
    /// mod's root marker, which is not drawn.
    pub(crate) path: Option<String>,
    pub(crate) parent: Option<Rc<Crumb>>,
    /// What clicking, or typing in, the node sends, when it does.
    pub(crate) does: Option<String>,
}

impl Crumb {
    pub(crate) fn node(node: &Node, path: &str, parent: Option<Rc<Crumb>>) -> Rc<Self> {
        Rc::new(Self {
            label: describe(node),
            line: line(node),
            owner: None,
            path: Some(path.to_owned()),
            parent,
            does: context::does(node),
        })
    }

    pub(crate) fn module(name: &str, parent: Option<Rc<Crumb>>) -> Rc<Self> {
        Rc::new(Self {
            label: format!("mod {name}"),
            line: None,
            owner: Some(name.to_owned()),
            path: None,
            parent,
            does: None,
        })
    }

    /// Outermost first.
    pub(crate) fn trail(self: &Rc<Self>) -> Vec<Rc<Crumb>> {
        let mut trail = vec![self.clone()];
        while let Some(parent) = trail.last().and_then(|crumb| crumb.parent.clone()) {
            trail.push(parent);
        }
        trail.reverse();
        trail
    }
}

/// Where a tracked node was drawn this frame.
#[derive(Clone)]
pub(crate) struct Placed {
    pub(crate) bounds: Bounds<Pixels>,
    /// The part of it not clipped away by a box it is in (a scrolled
    /// list, a pane): what the pointer can reach and the outline follows.
    pub(crate) visible: Bounds<Pixels>,
    /// Its corner radii, as drawn, for the highlight to follow.
    pub(crate) corners: Corners<Pixels>,
    /// The node's element path, the same from frame to frame.
    pub(crate) path: String,
    pub(crate) crumb: Rc<Crumb>,
}

/// What tracked nodes record into, in the order they are prepainted:
/// parents before their children, popovers last.
pub(crate) type Sink = Rc<RefCell<Vec<Placed>>>;

#[derive(Default)]
pub(crate) struct Inspector {
    /// Picking: the pointer highlights, a click selects.
    pub(crate) picking: bool,
    sink: Sink,
    /// The last whole frame's record: `sink` is empty while a frame is
    /// built, and the card names the sizes of what it lists then.
    drawn: RefCell<Vec<Placed>>,
    /// What the pointer, and ↑ and ↓, aim at while picking.
    aim: aim::Aim,
    pub(crate) hovered: Option<Target>,
    pub(crate) selected: Option<Placed>,
    /// The deepest node the selection's card offers in its trail: what
    /// was under the pointer, when ↑ picked a node around it.
    pub(crate) leaf: Option<Placed>,
    /// The element path of the card's row under the pointer, which the
    /// window outlines to show where that row is.
    pub(crate) previewed: Option<String>,
    /// The report last saved where agents read the selection, so it is
    /// written again only when what it says changes.
    pub(crate) saved: Option<String>,
    /// Where the card's list of elements is scrolled.
    pub(crate) ladder: ScrollHandle,
    /// The mod that started this pick (`command = "inspect"`): it hears
    /// the pick on `app/inspect` once the card's choice is confirmed.
    pub(crate) asker: Option<String>,
    /// Where the highlight was last drawn, to animate it from there.
    pub(crate) highlight: Rc<RefCell<crate::overlay::Highlight>>,
    /// How it measures, and what that needs.
    pub(crate) measuring: crate::overlay::Measuring,
}

impl Inspector {
    /// Where this frame's nodes record themselves, when anything needs them.
    pub(crate) fn sink(&self) -> Option<Sink> {
        (self.picking || self.selected.is_some()).then(|| {
            // Asked more than once a frame: only the first finds a record.
            let mut sink = self.sink.borrow_mut();
            if !sink.is_empty() {
                *self.drawn.borrow_mut() = std::mem::take(&mut *sink);
            }
            self.sink.clone()
        })
    }

    /// The same, for painting what was recorded.
    pub(crate) fn recorded(&self) -> Sink {
        self.sink.clone()
    }
}

/// A pick, for a mod that asked for one.
pub(crate) struct Pick {
    /// The element, as the card names it: `text.status-text "⎇ main"`.
    pub(crate) element: String,
    /// The innermost mod that drew it.
    pub(crate) owner: String,
    /// The file and line that built it.
    pub(crate) at: String,
    /// The whole report, as `report` writes it.
    pub(crate) report: String,
}

/// A pick for `asker`, the mod it is handed to, or the user's own (none):
/// a mod reads the live values only as [`Workspace::view_context_for`]
/// allows.
pub(crate) fn pick(placed: &Placed, workspace: &Workspace, asker: Option<&str>) -> Pick {
    let (owner, at) = origin(placed, workspace);
    Pick {
        element: placed.crumb.label.clone(),
        owner,
        at,
        report: report_for(placed, workspace, asker),
    }
}

/// The innermost mod that drew `placed`, and the innermost line within it.
fn origin(placed: &Placed, workspace: &Workspace) -> (String, String) {
    let (owner, source, line) = source_of(placed, workspace);
    let at = location(source.as_deref(), line);
    (owner, at)
}

/// The innermost mod that drew `placed`, its file, and the innermost line
/// within it that built the element.
pub(crate) fn source_of(
    placed: &Placed,
    workspace: &Workspace,
) -> (String, Option<std::path::PathBuf>, Option<u32>) {
    let trail = placed.crumb.trail();
    let owner = trail
        .iter()
        .rev()
        .find_map(|crumb| crumb.owner.as_deref())
        .unwrap_or(moonframe_core::SHELL);
    let line = trail
        .iter()
        .rev()
        .take_while(|crumb| crumb.owner.is_none())
        .find_map(|crumb| crumb.line);
    let source = workspace
        .slot(owner)
        .and_then(|slot| slot.source())
        .map(Path::to_path_buf);
    (owner.to_owned(), source, line)
}

/// A pick, written for the user's agent to act on: the selection file
/// and the clipboard.
pub(crate) fn report(placed: &Placed, workspace: &Workspace) -> String {
    report_for(placed, workspace, None)
}

/// The same, for `asker` (see [`pick`]).
fn report_for(placed: &Placed, workspace: &Workspace, asker: Option<&str>) -> String {
    let trail = placed.crumb.trail();
    let (owner, at) = origin(placed, workspace);
    let mut out = String::from("Selected in Moonframe (View > Inspect Element):\n");
    let _ = writeln!(out, "- element: {}", placed.crumb.label);
    let _ = writeln!(out, "- mod: {owner}, drawn by {at}");
    let path: Vec<&str> = trail.iter().map(|crumb| crumb.label.as_str()).collect();
    let _ = writeln!(out, "- inside: {}", path.join(" > "));
    let (corner, size) = (placed.bounds.origin, placed.bounds.size);
    let _ = writeln!(
        out,
        "- drawn at {:.0},{:.0}, {:.0}×{:.0} px",
        f32::from(corner.x),
        f32::from(corner.y),
        f32::from(size.width),
        f32::from(size.height)
    );
    // What it sends can carry its mod's state, so it is read as that is.
    if let Some(does) = &placed.crumb.does
        && workspace.may_read_context(&owner, asker)
    {
        let _ = writeln!(out, "- {does}");
    }
    if let Some(copy) = copy_to(workspace.slot(&owner).and_then(|slot| slot.source())) {
        let _ = writeln!(out, "{copy}");
    }
    if let Some(context) = workspace.view_context_for(&owner, asker) {
        out.push_str(&context.report(&owner));
    }
    out
}

/// What to do with `source` when it ships in the installed app: it is
/// signed, so the change goes in a copy, which wins over it.
fn copy_to(source: Option<&Path>) -> Option<String> {
    let folder = &moonframe_providers::installed()?.folder;
    let path = paths::overlay(source?)?;
    Some(read_only(&folder.join(path)))
}

fn read_only(copy: &Path) -> String {
    format!(
        "- read-only: that file ships in the signed app. Copy it to {} and change the copy, \
         which replaces it at once",
        copy.display()
    )
}

fn location(source: Option<&Path>, line: Option<u32>) -> String {
    match (source, line) {
        (Some(source), Some(line)) => format!("{}:{line}", source.display()),
        (Some(source), None) => source.display().to_string(),
        (None, _) => "an unknown file".to_owned(),
    }
}

/// A node as the inspector names it: its type, classes, and text.
fn describe(node: &Node) -> String {
    let classes = |look: &moonframe_core::Look| {
        look.class
            .iter()
            .map(|class| format!(".{class}"))
            .collect::<String>()
    };
    match node {
        Node::Stack(stack) => {
            let kind = match stack.direction {
                moonframe_core::Direction::Row => "row",
                moonframe_core::Direction::Column => "column",
            };
            format!("{kind}{}", classes(&stack.look))
        },
        Node::Panel(panel) => match &panel.title {
            Some(title) => format!("panel{} {}", classes(&panel.look), quote(title)),
            None => format!("panel{}", classes(&panel.look)),
        },
        Node::Split(split) => match split.direction {
            moonframe_core::Direction::Row => "hsplit".to_owned(),
            moonframe_core::Direction::Column => "vsplit".to_owned(),
        },
        Node::Text(text) => format!("text{} {}", classes(&text.look), quote(&text.text)),
        Node::Button(button) => format!("button{} {}", classes(&button.look), quote(&button.label)),
        Node::Item(item) => format!("item{} {}", classes(&item.look), quote(&item.label)),
        Node::Badge(badge) => format!("badge{} {}", classes(&badge.look), quote(&badge.text)),
        Node::Icon(icon) => format!("icon{} {}", classes(&icon.look), icon.name.name()),
        Node::Image(image) => format!("image{} {}", classes(&image.look), quote(&image.src)),
        Node::Line(line) => format!("line{} ({} points)", classes(&line.look), line.points.len()),
        Node::Paths(paths) => format!("paths{} ({})", classes(&paths.look), paths.paths.len()),
        Node::RevisionTimeline(timeline) => format!(
            "revision_timeline{} ({})",
            classes(&timeline.look),
            timeline.entries.len()
        ),
        Node::Hatch(hatch) => format!("hatch{}", classes(&hatch.look)),
        Node::Map(map) => format!("map{} {}", classes(&map.look), map.provider.name()),
        Node::Terminal(terminal) => format!(
            "terminal{} {} ({})",
            classes(&terminal.look),
            quote(&terminal.id),
            terminal.engine.name()
        ),
        Node::Browser(browser) => format!(
            "browser{} {} {}",
            classes(&browser.look),
            quote(&browser.id),
            browser
                .url
                .as_deref()
                .map_or_else(|| "about:blank".to_owned(), quote)
        ),
        Node::Video(video) => format!(
            "video{} {} {}",
            classes(&video.look),
            quote(&video.id),
            quote(&video.source)
        ),
        Node::View(view) => format!(
            "view{} {} {}",
            classes(&view.look),
            quote(&view.name),
            quote(&view.id)
        ),
        Node::Input(input) => {
            let mut name = format!("input{}", classes(&input.look));
            if let Some(id) = &input.id {
                name = format!("{name} {}", quote(id));
            }
            if let Some(placeholder) = &input.placeholder {
                name = format!("{name} {}", quote(placeholder));
            }
            name
        },
        Node::Markdown(markdown) => {
            let first = markdown.text.lines().find(|line| !line.trim().is_empty());
            let mut name = format!("markdown{}", classes(&markdown.look));
            if let Some(first) = first {
                name = format!("{name} {}", quote(first));
            }
            name
        },
        Node::Divider => "divider".to_owned(),
        Node::Spacer => "spacer".to_owned(),
        Node::Region(region) => format!("region {}", quote(&region.name)),
        Node::Mount(name) => format!("mount {}", quote(name)),
        Node::Instance(instance) => match &instance.of {
            moonframe_core::InstanceOf::Mod(name) => format!("mod {name} #{}", instance.id),
            moonframe_core::InstanceOf::Component(name) => format!("{name} #{}", instance.id),
        },
        Node::Popover(popover) => format!("popover{}", classes(&popover.look)),
        Node::Overlay(overlay) => format!(
            "overlay{} ({})",
            classes(&overlay.look),
            overlay.place.name()
        ),
        Node::List(list) => format!(
            "list{} {} ({} rows)",
            classes(&list.look),
            quote(&list.id),
            list.count
        ),
        Node::Problem(message) => format!("broken element ({})", quote(message)),
    }
}

fn line(node: &Node) -> Option<u32> {
    match node {
        Node::Stack(stack) => stack.look.line,
        Node::Panel(panel) => panel.look.line,
        Node::Text(text) => text.look.line,
        Node::Button(button) => button.look.line,
        Node::Item(item) => item.look.line,
        Node::Badge(badge) => badge.look.line,
        Node::Icon(icon) => icon.look.line,
        Node::Image(image) => image.look.line,
        Node::Line(line) => line.look.line,
        Node::Paths(paths) => paths.look.line,
        Node::RevisionTimeline(timeline) => timeline.look.line,
        Node::Hatch(hatch) => hatch.look.line,
        Node::Map(map) => map.look.line,
        Node::Terminal(terminal) => terminal.look.line,
        Node::Browser(browser) => browser.look.line,
        Node::Video(video) => video.look.line,
        Node::View(view) => view.look.line,
        Node::Input(input) => input.look.line,
        Node::Markdown(markdown) => markdown.look.line,
        Node::Popover(popover) => popover.look.line,
        Node::Overlay(overlay) => overlay.look.line,
        Node::List(list) => list.look.line,
        Node::Split(_)
        | Node::Divider
        | Node::Spacer
        | Node::Region(_)
        | Node::Mount(_)
        | Node::Instance(_)
        | Node::Problem(_) => None,
    }
}

/// How round a node's corners are drawn: its style's radius, else the
/// element's own (buttons and items take the theme's, badges are pills).
/// A pill's radius is clamped to half its height when painted.
pub(crate) fn corners(
    node: &Node,
    styled: Option<moonframe_core::Corners>,
    theme: Pixels,
) -> Corners<Pixels> {
    if let Some(styled) = styled {
        // A button takes one radius: its top left's.
        if matches!(node, Node::Button(_)) {
            return Corners::all(px(styled.top_left));
        }
        return Corners {
            top_left: px(styled.top_left),
            top_right: px(styled.top_right),
            bottom_right: px(styled.bottom_right),
            bottom_left: px(styled.bottom_left),
        };
    }
    match node {
        Node::Button(_) | Node::Item(_) => Corners::all(theme),
        Node::Badge(_) => Corners::all(px(f32::MAX / 4.)),
        _ => Corners::default(),
    }
}

fn quote(text: &str) -> String {
    let mut short: String = text.chars().take(QUOTE).collect();
    if text.chars().count() > QUOTE {
        short.push('…');
    }
    format!("{short:?}")
}

/// A drawn node that records its bounds as it is prepainted. It hands its
/// child's layout straight through, so the layout is the same as without it.
pub(crate) struct Tracked {
    pub(crate) child: AnyElement,
    pub(crate) corners: Corners<Pixels>,
    pub(crate) path: String,
    pub(crate) crumb: Rc<Crumb>,
    pub(crate) sink: Sink,
}

impl IntoElement for Tracked {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Tracked {
    type PrepaintState = ();
    type RequestLayoutState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.sink.borrow_mut().push(Placed {
            bounds,
            visible: bounds.intersect(&window.content_mask().bounds),
            corners: self.corners,
            path: self.path.clone(),
            crumb: self.crumb.clone(),
        });
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
