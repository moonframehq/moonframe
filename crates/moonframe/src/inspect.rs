//! Inspect: pick any element in the window, as in a browser's developer
//! tools, to tell an agent where to change or add something. While picking,
//! every drawn node is wrapped in [`Tracked`], which records where it landed
//! without adding a layout box; the pointer then finds the deepest node
//! under it. The pick names the mod, its file and line, and the elements
//! around it, and is saved where agents read it (`paths::selection`).

use {
    gpui_kit::*,
    moonframe_core::{Node, Workspace},
    std::{cell::RefCell, fmt::Write as _, path::Path, rc::Rc},
};

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
    pub(crate) parent: Option<Rc<Crumb>>,
}

impl Crumb {
    pub(crate) fn node(node: &Node, parent: Option<Rc<Crumb>>) -> Rc<Self> {
        Rc::new(Self {
            label: describe(node),
            line: line(node),
            owner: None,
            parent,
        })
    }

    pub(crate) fn module(name: &str, parent: Option<Rc<Crumb>>) -> Rc<Self> {
        Rc::new(Self {
            label: format!("mod {name}"),
            line: None,
            owner: Some(name.to_owned()),
            parent,
        })
    }

    /// Outermost first.
    fn trail(self: &Rc<Self>) -> Vec<Rc<Crumb>> {
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
    pub(crate) hovered: Option<Placed>,
    pub(crate) selected: Option<Placed>,
}

impl Inspector {
    /// Where this frame's nodes record themselves, when anything needs them.
    pub(crate) fn sink(&self) -> Option<Sink> {
        (self.picking || self.selected.is_some()).then(|| {
            self.sink.borrow_mut().clear();
            self.sink.clone()
        })
    }

    /// The same, for painting what was recorded.
    pub(crate) fn recorded(&self) -> Sink {
        self.sink.clone()
    }

    /// The deepest node under `point` in the last frame.
    pub(crate) fn at(&self, point: Point<Pixels>) -> Option<Placed> {
        self.sink
            .borrow()
            .iter()
            .rev()
            .find(|placed| placed.bounds.contains(&point))
            .cloned()
    }
}

/// A pick, written for an agent to act on.
pub(crate) fn report(placed: &Placed, workspace: &Workspace) -> String {
    let trail = placed.crumb.trail();
    // The innermost mod, and the innermost line within it.
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
    let source = workspace.slot(owner).and_then(|slot| slot.source());
    let mut out = String::from("Selected in Moonframe (View > Inspect Element):\n");
    let _ = writeln!(out, "- element: {}", placed.crumb.label);
    let _ = writeln!(out, "- mod: {owner}, drawn by {}", location(source, line));
    let path: Vec<&str> = trail.iter().map(|crumb| crumb.label.as_str()).collect();
    let _ = writeln!(out, "- inside: {}", path.join(" > "));
    out
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
        Node::Divider => "divider".to_owned(),
        Node::Spacer => "spacer".to_owned(),
        Node::Region(region) => format!("region {}", quote(&region.name)),
        Node::Mount(name) => format!("mount {}", quote(name)),
        Node::Popover(popover) => format!("popover{}", classes(&popover.look)),
        Node::Overlay(overlay) => format!(
            "overlay{} ({})",
            classes(&overlay.look),
            overlay.place.name()
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
        Node::Terminal(terminal) => terminal.look.line,
        Node::Browser(browser) => browser.look.line,
        Node::Input(input) => input.look.line,
        Node::Popover(popover) => popover.look.line,
        Node::Overlay(overlay) => overlay.look.line,
        Node::Split(_)
        | Node::Divider
        | Node::Spacer
        | Node::Region(_)
        | Node::Mount(_)
        | Node::Problem(_) => None,
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
