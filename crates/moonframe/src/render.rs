//! Draws the checked node trees with GPUI and gpui-component. Every node
//! type implements [`Draw`]; the [`Painter`] holds what they share: the
//! workspace, mod slots, and the styles they resolve. Clicks go back to
//! [`MoonframeApp::dispatch`] with the module that drew the control, so a
//! message reaches its `update` without the tree holding any Lua.
//!
//! Each element's look is, lowest first: its default here, the stylesheet's
//! rules (`Stylesheet::resolve`), and its own keys such as `padding`.

mod elements;
mod gallery;
mod styling;
mod terminal;

use {
    crate::{
        app::MoonframeApp,
        inspect::{Crumb, Sink, Tracked},
        motion::{Motion, Shown},
        terminal::Terminals,
    },
    gpui_kit::{
        component::{ActiveTheme, Icon, Sizable as _, tooltip::Tooltip},
        *,
    },
    moonframe_core::{
        Action, Data, Direction, Look, Node, Part, Resolved, Rules, SHELL, Slot, Transition,
        Workspace,
    },
    std::{rc::Rc, sync::LazyLock, time::Instant},
};

static NO_RULES: LazyLock<Rules> = LazyLock::new(Rules::default);

/// Mounts deep enough for any real layout; deeper is a mod mounting itself.
const MAX_MOUNTS: usize = 16;
/// Elements one frame draws. Each view is bounded when it is read, but a mod
/// mounted many times is drawn many times, so the frame has its own bound.
const MAX_DRAWN: usize = 50_000;

type Cx<'a, 'b> = &'a Context<'b, MoonframeApp>;

/// Something in a view that knows how to draw itself.
trait Draw<'w> {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement;
}

/// Where a node is drawn: by which mod, at which path (the root of its
/// element ids), and inside a row or a column. While inspecting, also the
/// nodes it is inside.
#[derive(Clone)]
struct Place<'w> {
    owner: &'w str,
    path: String,
    parent: Direction,
    crumb: Option<Rc<Crumb>>,
}

impl<'w> Place<'w> {
    /// A mod's root.
    fn root(owner: &'w str) -> Self {
        Self {
            owner,
            path: owner.to_owned(),
            parent: Direction::Column,
            crumb: None,
        }
    }

    /// The `index`th child of a container laid out along `parent`.
    fn child(&self, index: usize, parent: Direction) -> Self {
        Self {
            path: format!("{}/{index}", self.path),
            parent,
            ..self.clone()
        }
    }

    /// A named part, such as a panel's header.
    fn part(&self, name: &str) -> Self {
        Self {
            path: format!("{}/{name}", self.path),
            ..self.clone()
        }
    }

    fn id(&self) -> ElementId {
        ElementId::Name(SharedString::from(self.path.clone()))
    }

    /// Sends `action` from this node's mod when clicked.
    fn click(
        &self,
        action: &Action,
        cx: Cx,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let owner = self.owner.to_owned();
        let action = action.clone();
        cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.dispatch(&owner, &action, window, cx)
        })
    }
}

pub(crate) struct Painter<'w, 'm> {
    workspace: &'w Workspace,
    /// The modules being drawn, outermost first, to catch mount cycles.
    mounted: Vec<&'w str>,
    motion: &'m mut Motion,
    /// The frame's time, the same for every pane in it.
    now: Instant,
    /// Whether something is still moving and needs another frame.
    animating: bool,
    /// Elements drawn so far this frame.
    drawn: usize,
    /// The open popover's mod and `dismiss` message: a click outside it
    /// sends this.
    backdrop: Option<(&'w str, Data)>,
    /// Where nodes record their bounds while inspecting.
    sink: Option<Sink>,
    /// The innermost node being drawn, while inspecting: what a mod mounted
    /// inside it is inside.
    outer: Option<Rc<Crumb>>,
    /// The shells terminals show.
    terminals: &'m mut Terminals,
    /// Drawing a preview (the component browser): nothing starts a shell.
    preview: bool,
}

impl<'w, 'm> Painter<'w, 'm> {
    pub(crate) fn new(
        workspace: &'w Workspace,
        motion: &'m mut Motion,
        terminals: &'m mut Terminals,
        sink: Option<Sink>,
    ) -> Self {
        Self {
            workspace,
            mounted: Vec::new(),
            motion,
            now: Instant::now(),
            animating: false,
            drawn: 0,
            backdrop: None,
            sink,
            outer: None,
            terminals,
            preview: false,
        }
    }

    /// For previews, which show elements without starting anything.
    pub(crate) fn previewing(mut self) -> Self {
        self.preview = true;
        self
    }

    pub(crate) fn animating(&self) -> bool {
        self.animating
    }

    /// The whole window: the shell, under the stylesheet's and providers'
    /// problems if any, and the backdrop of an open popover.
    pub(crate) fn shell(&mut self, cx: Cx) -> AnyElement {
        let shell = self.slot(SHELL, cx);
        // The stylesheet's problem and each provider's, above the shell.
        let banners: Vec<_> = self
            .workspace
            .style_error()
            .map(|error| format!("style: {error}"))
            .into_iter()
            .chain(
                self.workspace
                    .host_problems()
                    .map(|(name, error)| format!("{name}: {error}")),
            )
            .map(|message| problem(message, cx))
            .collect();
        // Over the shell's top edge rather than above it, so a broken
        // stylesheet or provider never moves the layout.
        let banners = (!banners.is_empty()).then(|| {
            deferred(
                div()
                    .absolute()
                    .top(px(4.))
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .children(banners),
            )
            .with_priority(1)
        });
        let backdrop = self.backdrop.take().map(|(owner, msg)| {
            let action = Action::Send { to: None, msg };
            let owner = owner.to_owned();
            // Under the popover's content, over everything else.
            deferred(
                div()
                    .id("popover-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            this.dispatch(&owner, &action, window, cx)
                        }),
                    ),
            )
            .with_priority(1)
        });
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(shell)
            .children(banners)
            .children(backdrop)
            .into_any_element()
    }

    /// A module's last good view, under its problem when it has one.
    fn slot(&mut self, name: &'w str, cx: Cx) -> AnyElement {
        if self.mounted.contains(&name) || self.mounted.len() == MAX_MOUNTS {
            return problem(format!("`{name}` is mounted inside itself"), cx);
        }
        let Some(slot) = self.workspace.slot(name) else {
            return problem(format!("no mod named `{name}`"), cx);
        };
        self.mounted.push(name);
        let mut root = Place::root(name);
        if self.sink.is_some() {
            root.crumb = Some(Crumb::module(name, self.outer.clone()));
        }
        let view = slot.view().map(|node| node.draw(self, &root, cx));
        self.mounted.pop();
        match (slot.error(), view) {
            (None, Some(view)) => view,
            // The last good view keeps its place, and the problem floats
            // over its corner: an edit that breaks never moves the layout.
            (Some(error), Some(view)) => div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(view)
                .child(
                    div()
                        .absolute()
                        .top(px(4.))
                        .right(px(4.))
                        .max_w(px(PROBLEM_WIDTH))
                        .child(problem(format!("{name}: {error}"), cx)),
                )
                .into_any_element(),
            (error, None) => problem(
                format!(
                    "{name}: {}",
                    error.map_or_else(|| "nothing to show".to_owned(), ToString::to_string)
                ),
                cx,
            ),
        }
    }

    /// The mods in a region, in order.
    fn region(&mut self, name: &str, cx: Cx) -> Vec<AnyElement> {
        self.workspace
            .region(name)
            .into_iter()
            .map(|member| self.slot(member, cx))
            .collect()
    }

    fn children(
        &mut self,
        nodes: &'w [Node],
        place: &Place<'w>,
        direction: Direction,
        cx: Cx,
    ) -> Vec<AnyElement> {
        nodes
            .iter()
            .enumerate()
            .map(|(index, node)| node.draw(self, &place.child(index, direction), cx))
            .collect()
    }

    /// How much of the pane at `place` shows this frame, asking for another
    /// frame while it moves.
    fn shown(&mut self, place: &Place, visible: bool, transition: Option<&Transition>) -> Shown {
        let shown = self.motion.pane(&place.path, visible, transition, self.now);
        if transition.is_some() {
            tracing::trace!(pane = %place.path, visible, ?shown, "pane motion");
        }
        self.animating |= matches!(shown, Shown::Moving(_));
        shown
    }

    /// A part's style from the stylesheet and the rules of the mod that drew it.
    fn look(&self, part: Part, look: &Look, selected: bool, place: &Place) -> Resolved {
        let local = self
            .workspace
            .slot(place.owner)
            .map(Slot::rules)
            .unwrap_or(&NO_RULES);
        self.workspace
            .stylesheet()
            .resolve(part, look, selected, local)
    }
}

impl<'w> Draw<'w> for Node {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        painter.drawn += 1;
        match painter.drawn.cmp(&MAX_DRAWN) {
            std::cmp::Ordering::Less => {},
            std::cmp::Ordering::Equal => {
                return problem(
                    format!(
                        "more than {MAX_DRAWN} elements in one frame; is a mod mounted in a loop?"
                    ),
                    cx,
                );
            },
            std::cmp::Ordering::Greater => return div().into_any_element(),
        }
        let Some(sink) = painter.sink.clone() else {
            return draw_node(self, painter, place, cx);
        };
        let crumb = Crumb::node(self, place.crumb.clone());
        let place = Place {
            crumb: Some(crumb.clone()),
            ..place.clone()
        };
        let outer = painter.outer.replace(crumb.clone());
        let child = draw_node(self, painter, &place, cx);
        painter.outer = outer;
        Tracked {
            child,
            path: place.path,
            crumb,
            sink,
        }
        .into_any_element()
    }
}

/// The node itself, without the inspector's tracking.
fn draw_node<'w>(
    node: &'w Node,
    painter: &mut Painter<'w, '_>,
    place: &Place<'w>,
    cx: Cx,
) -> AnyElement {
    match node {
        Node::Stack(stack) => stack.draw(painter, place, cx),
        Node::Panel(panel) => panel.draw(painter, place, cx),
        Node::Split(split) => split.draw(painter, place, cx),
        Node::Text(text) => text.draw(painter, place, cx),
        Node::Button(button) => button.draw(painter, place, cx),
        Node::Item(item) => item.draw(painter, place, cx),
        Node::Badge(badge) => badge.draw(painter, place, cx),
        Node::Icon(icon) => icon.draw(painter, place, cx),
        Node::Terminal(terminal) => terminal.draw(painter, place, cx),
        Node::Region(region) => region.draw(painter, place, cx),
        Node::Divider => elements::divider(painter, place, cx),
        Node::Spacer => div().flex_1().into_any_element(),
        Node::Mount(name) => painter.slot(name, cx),
        Node::Popover(popover) => popover.draw(painter, place, cx),
        Node::Problem(message) => broken(message, place, cx),
    }
}

/// The widest a floating problem gets, and the most of its text it shows;
/// `just check` and the log have all of it.
const PROBLEM_WIDTH: f32 = 360.;
const PROBLEM_TEXT: usize = 300;

/// A problem, kept small: a few lines at most, so a long message never
/// takes over the space it is drawn in.
fn problem(message: String, cx: Cx) -> AnyElement {
    let theme = cx.theme();
    let mut text: String = message.chars().take(PROBLEM_TEXT).collect();
    if message.chars().count() > PROBLEM_TEXT {
        text.push('…');
    }
    div()
        .flex_shrink_0()
        .m_1()
        .px_2()
        .py_1()
        .max_h(px(72.))
        .overflow_hidden()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.danger)
        .bg(theme.background)
        .text_color(theme.danger)
        .text_xs()
        .child(text)
        .into_any_element()
}

/// A child that could not be read: an icon-sized mark in its place, with
/// what is wrong on hover, so one mistake breaks one spot and nothing moves.
fn broken(message: &str, place: &Place, cx: Cx) -> AnyElement {
    let theme = cx.theme();
    let message = SharedString::from(message.to_owned());
    div()
        .id(place.id())
        .flex_shrink_0()
        .size(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .bg(theme.danger.opacity(0.12))
        .child(
            Icon::default()
                .path("icons/triangle-alert.svg")
                .xsmall()
                .text_color(theme.danger),
        )
        .tooltip(move |window, cx| Tooltip::new(message.clone()).build(window, cx))
        .into_any_element()
}
