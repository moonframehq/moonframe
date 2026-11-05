//! Draws the checked node trees with GPUI and gpui-component. Every node
//! type implements [`Draw`]; the [`Painter`] holds what they share: the
//! workspace, mod slots, and the styles they resolve. Clicks go back to
//! [`MoonframeApp::dispatch`] with the module that drew the control, so a
//! message reaches its `update` without the tree holding any Lua.
//!
//! Each element's look is, lowest first: its default here, the stylesheet's
//! rules (`Stylesheet::resolve`), and its own keys such as `padding`.

mod browser;
pub(crate) mod cached;
mod carry;
mod elements;
mod floating;
mod gallery;
pub(crate) mod images;
mod input;
mod keyed;
mod list;
#[cfg_attr(not(feature = "map"), path = "render/map_left_out.rs")]
mod map;
#[cfg_attr(not(feature = "markdown"), path = "render/markdown_left_out.rs")]
mod markdown;
mod place;
mod plot;
mod pointer;
mod popover;
mod styling;
#[cfg_attr(not(feature = "syntax"), path = "render/syntax_left_out.rs")]
mod syntax;
#[cfg_attr(not(feature = "terminal"), path = "render/terminal_left_out.rs")]
mod terminal;
mod video;
mod view;
mod viewer;

use {
    crate::{
        app::MoonframeApp,
        browser::Browsers,
        color::ThemeColor,
        hosted::Views,
        input::Inputs,
        inspect::{Crumb, Sink, Tracked},
        motion::{Keyed, Motion, Shown, Snapshot},
    },
    gpui_kit::{
        component::{ActiveTheme, Icon, Sizable as _, Theme, tooltip::Tooltip},
        *,
    },
    moonframe_core::{
        Action, Color, Data, Direction, Keys, ListWindow, Look, Node, Paint, Part, Resolved, Rules,
        SHELL, Shade, Slot, States, Transition, Workspace,
    },
    std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        path::Path,
        rc::Rc,
        sync::LazyLock,
        time::{Duration, Instant},
    },
};

#[cfg(feature = "terminal")]
use crate::terminal::Terminals;

static NO_RULES: LazyLock<Rules> = LazyLock::new(Rules::default);

/// One turn of a spinning icon, before `animation_speed`.
const SPIN_PERIOD: Duration = Duration::from_secs(1);
/// When spinning icons started turning: all of them turn together.
static SPIN_EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Mounts deep enough for any real layout; deeper is a mod mounting itself.
const MAX_MOUNTS: usize = 16;
/// Elements one frame draws. Each view is bounded when it is read, but a mod
/// mounted many times is drawn many times, so the frame has its own bound.
const MAX_DRAWN: usize = 50_000;

type Cx<'a, 'b> = &'a Context<'b, MoonframeApp>;

/// Where floating layers paint, lowest first. A popover's backdrop and
/// content sit just above the layer it is drawn in: the window's (0), or an
/// overlay's. Each open overlay takes the next step up, in tree order, so a
/// later one is above an earlier one and everything in it.
const POPOVER_BACKDROP: usize = 1;
const POPOVER_CONTENT: usize = 2;
const OVERLAY_BASE: usize = 10;
const OVERLAY_STEP: usize = 10;
/// Above every layer the painter hands out: each overlay is a node, so a
/// frame opens fewer than `MAX_DRAWN`, and its popovers sit within its step.
const APP_LAYERS: usize = overlay_layer(MAX_DRAWN);
/// The toast, over every overlay and popover.
pub(crate) const TOAST_LAYER: usize = APP_LAYERS;
/// The inspector's lowest layer (its others are just above), over the toast
/// too, so it can pick inside an open dialog.
pub(crate) const INSPECTOR_LAYER: usize = APP_LAYERS + 1;

/// The paint priority of the `index`th overlay open in a frame.
const fn overlay_layer(index: usize) -> usize {
    OVERLAY_BASE + OVERLAY_STEP * index
}

// Checked when the crate builds: a popover sits within its layer's step, and
// one in the last overlay a frame can open is still under the toast, which
// is under the inspector, so picking reaches inside any open dialog.
const _: () = {
    assert!(POPOVER_BACKDROP < POPOVER_CONTENT && POPOVER_CONTENT < OVERLAY_STEP);
    assert!(overlay_layer(MAX_DRAWN - 1) + POPOVER_CONTENT < TOAST_LAYER);
    assert!(TOAST_LAYER < INSPECTOR_LAYER);
};

/// An open popover or overlay that may take keystrokes: the topmost one (by
/// `priority`, then the later in the tree) gets the first say over each key.
#[derive(Clone, Debug)]
pub(crate) struct KeyLayer {
    pub(crate) priority: usize,
    /// The mod that drew it, whose `update` gets the messages.
    pub(crate) owner: String,
    /// The mod its messages go to, if not the owner.
    pub(crate) to: Option<String>,
    pub(crate) keys: Keys,
    /// Also what plain Escape sends, unless `keys` takes Escape.
    pub(crate) dismiss: Option<Data>,
    /// It blocks the window behind (a backdrop), so a terminal there gets
    /// no keys while it is open.
    pub(crate) modal: bool,
}

/// What a click outside an open popover sends, and the layer it is in.
struct Backdrop<'w> {
    priority: usize,
    owner: &'w str,
    to: Option<String>,
    msg: Data,
}

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
            // The innermost control takes the click: a tab's close button
            // does not also select the tab.
            cx.stop_propagation();
            this.dispatch(&owner, &action, window, cx)
        })
    }
}

impl Place<'_> {
    /// Sends `action` from this node's mod on a right-click.
    fn context(
        &self,
        action: &Action,
        cx: Cx,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        let owner = self.owner.to_owned();
        let action = action.clone();
        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            this.dispatch(&owner, &action, window, cx)
        })
    }
}

/// What a drawn frame leaves for the app to do.
pub(crate) struct Finished {
    /// Something still moves: draw again next frame.
    pub(crate) animating: bool,
    /// The live views that move on by themselves, the rest of the window
    /// keeping still.
    pub(crate) live_again: Vec<EntityId>,
    /// The rows lists asked for.
    pub(crate) list_requests: Vec<ListRequest>,
    /// The open popovers and overlays it drew, for their keys.
    pub(crate) key_layers: Vec<KeyLayer>,
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
    /// The open popovers' click-outside layers.
    backdrops: Vec<Backdrop<'w>>,
    /// Open overlays' layers, in tree order. A slot is taken before the
    /// overlay's content is drawn, so an overlay inside it comes after.
    overlays: Vec<Option<AnyElement>>,
    /// The paint priority of the layer being drawn: 0 for the window, an
    /// overlay's while drawing its content.
    layer: usize,
    /// Open popovers and overlays, for the keys they take.
    key_layers: Vec<KeyLayer>,
    /// Where nodes record their bounds while inspecting.
    sink: Option<Sink>,
    /// The innermost node being drawn, while inspecting: what a mod mounted
    /// inside it is inside.
    outer: Option<Rc<Crumb>>,
    /// Each element's corner radius from its style, by path, while
    /// inspecting: the highlight follows its corners.
    radii: RefCell<HashMap<String, moonframe_core::Corners>>,
    /// The shells terminals show.
    #[cfg(feature = "terminal")]
    terminals: &'m mut Terminals,
    /// The text input fields edit.
    inputs: Inputs,
    /// Drawing a preview (the component browser): nothing starts a shell,
    /// and fields can't be edited.
    preview: bool,
    /// Drawing inside a `drag = true` stack: controls cover it, so a press
    /// on one is a click and not the start of a window move.
    in_drag: bool,
    /// The pages browsers show; none in previews.
    browsers: Option<&'m mut Browsers>,
    /// The app's own views; none in previews.
    views: Option<&'m mut Views>,
    /// What keyed containers drew last frame, for children that arrive or
    /// go; none in previews, which draw them as they are.
    last_keyed: Option<&'w Keyed>,
    /// The theme, for the colors a state's look eases between.
    theme: Theme,
    /// Whether an element is moving between named states: styles resolve
    /// through a shared borrow, so this is apart from `animating`.
    tweening: Cell<bool>,
    /// The rows lists need that their views did not build, as `(owner,
    /// list id, rows)`: asked for once the frame is drawn.
    list_requests: Vec<(String, String, ListWindow)>,
    /// The window it draws, for what is kept per window (a viewer's box);
    /// `None` for the components window.
    window: Option<WindowId>,
    /// How many frames that window has drawn, this one included: what was
    /// not drawn in the frame before this one was gone in between.
    frame: u64,
    /// What this window keeps from frame to frame; none where nothing is.
    cached: Option<&'m mut cached::Cached>,
    /// The next node is the root of a kept view.
    root: Option<cached::Root>,
    /// Drawing inside a kept view.
    in_kept: bool,
    /// The live views that asked for another frame.
    live_again: Vec<EntityId>,
}

/// A list's rows a mod should build, which the app asks the workspace for
/// after the frame.
pub(crate) type ListRequest = (String, String, ListWindow);

impl<'w, 'm> Painter<'w, 'm> {
    pub(crate) fn new(
        workspace: &'w Workspace,
        motion: &'m mut Motion,
        #[cfg(feature = "terminal")] terminals: &'m mut Terminals,
        inputs: &Inputs,
        sink: Option<Sink>,
        theme: &Theme,
    ) -> Self {
        Self {
            theme: theme.clone(),
            tweening: Cell::new(false),
            list_requests: Vec::new(),
            workspace,
            mounted: Vec::new(),
            motion,
            now: crate::motion::now(),
            animating: false,
            drawn: 0,
            backdrops: Vec::new(),
            overlays: Vec::new(),
            layer: 0,
            key_layers: Vec::new(),
            sink,
            outer: None,
            radii: RefCell::default(),
            #[cfg(feature = "terminal")]
            terminals,
            inputs: inputs.clone(),
            preview: false,
            in_drag: false,
            browsers: None,
            views: None,
            last_keyed: None,
            window: None,
            frame: 0,
            cached: None,
            root: None,
            in_kept: false,
            live_again: Vec::new(),
        }
    }

    /// For the window `window`: what is kept per window stays apart.
    pub(crate) fn in_window(mut self, window: WindowId) -> Self {
        self.window = Some(window);
        self.frame = viewer::begin_frame(window);
        crate::gestures::begin_frame(window);
        self
    }

    /// Plays keyed children in and out against what the last frame drew
    /// (`Motion::begin_frame`).
    pub(crate) fn with_keyed(mut self, last: &'w Keyed) -> Self {
        self.last_keyed = Some(last);
        self
    }

    /// For the main window, whose browsers show their pages.
    pub(crate) fn with_browsers(mut self, browsers: &'m mut Browsers) -> Self {
        self.browsers = Some(browsers);
        self
    }

    /// For the main window, which keeps what does not move.
    pub(crate) fn with_cached(mut self, cached: &'m mut cached::Cached) -> Self {
        self.cached = Some(cached);
        self
    }

    /// For a kept view drawn by itself in `window`: unlike
    /// [`Self::in_window`], not a frame of that window.
    pub(crate) fn kept_in(mut self, window: WindowId) -> Self {
        self.window = Some(window);
        self
    }

    /// For windows that show the app's own views.
    pub(crate) fn with_views(mut self, views: &'m mut Views) -> Self {
        self.views = Some(views);
        self
    }

    /// For previews, which show elements without starting anything.
    pub(crate) fn previewing(mut self) -> Self {
        self.preview = true;
        self
    }

    /// The frame is drawn: what it leaves for the app to do.
    pub(crate) fn finish(self) -> Finished {
        Finished {
            animating: self.animating || self.tweening.get(),
            live_again: self.live_again,
            list_requests: self.list_requests,
            key_layers: self.key_layers,
        }
    }

    /// The whole window: the shell, the backdrops of open popovers, and
    /// open overlays. The app draws the stylesheet's and providers'
    /// problems over it.
    pub(crate) fn shell(&mut self, cx: Cx) -> AnyElement {
        self.root(SHELL, cx)
    }

    /// A window of its own for the mod `name`, with its popovers and
    /// overlays, as the shell has.
    pub(crate) fn window(&mut self, name: &'w str, cx: Cx) -> AnyElement {
        self.root(name, cx)
    }

    /// The mod `name` filling a window, with the backdrops of open
    /// popovers and the open overlays.
    fn root(&mut self, name: &'w str, cx: Cx) -> AnyElement {
        let shell = self.slot(name, None, cx);
        let backdrops: Vec<_> = std::mem::take(&mut self.backdrops)
            .into_iter()
            .enumerate()
            .map(|(index, backdrop)| {
                let action = Action::Send {
                    to: backdrop.to,
                    msg: backdrop.msg,
                };
                let owner = backdrop.owner.to_owned();
                // Under the popover's content, over the rest of its layer.
                deferred(
                    div()
                        .id(("popover-backdrop", index))
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
                .with_priority(backdrop.priority)
            })
            .collect();
        let overlays = std::mem::take(&mut self.overlays).into_iter().flatten();
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(shell)
            .children(backdrops)
            .children(overlays)
            .into_any_element()
    }

    /// A module's last good view, under its problem when it has one.
    fn slot(&mut self, name: &'w str, at: Option<&Place<'w>>, cx: Cx) -> AnyElement {
        if self.mounted.contains(&name) || self.mounted.len() == MAX_MOUNTS {
            return problem(format!("`{name}` is mounted inside itself"), cx);
        }
        let Some(slot) = self.workspace.slot(name) else {
            return problem(format!("no mod named `{name}`"), cx);
        };
        self.mounted.push(name);
        let mut root = at.map_or_else(|| Place::root(name), |at| at.mounted(name));
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
    fn region(&mut self, name: &str, place: &Place<'w>, cx: Cx) -> Vec<AnyElement> {
        self.workspace
            .region(name)
            .into_iter()
            .map(|member| self.slot(member, Some(place), cx))
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
            .map(|(index, node)| self.kept_child(node, &place.child(index, direction), cx))
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

    /// How far round a spinning icon is this frame, from 0 to 1, asking for
    /// another frame; `None` while `animation_speed` turns animations off.
    fn spin(&mut self) -> Option<f32> {
        self.phase(SPIN_PERIOD)
    }

    /// How far through a repeating animation of `period` (before
    /// `animation_speed`) this frame is, from 0 to 1, asking for another
    /// frame; `None` while animations are off. Every caller with the same
    /// period moves in step.
    fn phase(&mut self, period: Duration) -> Option<f32> {
        let phase = self.quiet_phase(period);
        self.animating |= phase.is_some();
        phase
    }

    /// As [`Self::phase`], for what paces its own frames.
    fn quiet_phase(&self, period: Duration) -> Option<f32> {
        let period = self.workspace.stylesheet().scaled(period);
        if period.is_zero() {
            return None;
        }
        let elapsed = self.now.saturating_duration_since(*SPIN_EPOCH);
        Some((elapsed.as_secs_f64() / period.as_secs_f64()).fract() as f32)
    }

    /// The folder of the mod (or component, in the browser) `owner`, which
    /// relative paths such as an image's `src` start from.
    fn folder(&self, owner: &str) -> Option<&'w Path> {
        let source = match self.workspace.slot(owner) {
            Some(slot) => slot.source(),
            None => self
                .workspace
                .components()
                .iter()
                .find(|component| component.name == owner)
                .map(|component| component.path.as_path()),
        };
        source.and_then(Path::parent)
    }

    /// A part's style from the stylesheet and the rules of the mod that drew it.
    fn look(&self, part: Part, look: &Look, selected: bool, place: &Place) -> Resolved {
        let states = States {
            selected,
            ..States::default()
        };
        self.look_in(part, look, states, place)
    }

    /// The same, for a part in `states` (focused, disabled).
    fn look_in(&self, part: Part, look: &Look, states: States, place: &Place) -> Resolved {
        let local = self
            .workspace
            .slot(place.owner)
            .map(Slot::rules)
            .unwrap_or(&NO_RULES);
        let mut resolved = self
            .workspace
            .stylesheet()
            .resolve_in(part, look, states, local);
        // Its own part's radius, which comes first, not a part inside it.
        if self.sink.is_some()
            && let Some(radius) = resolved.base.radius
        {
            self.radii
                .borrow_mut()
                .entry(place.path.clone())
                .or_insert(radius);
        }
        if let Some(state) = &look.state {
            self.tween(&mut resolved, state, place);
        }
        resolved
    }

    /// Eases `resolved`, the look of `state`, from the look of the state
    /// the element at `place` was in before, as its `transition` says.
    fn tween(&self, resolved: &mut Resolved, state: &str, place: &Place) {
        let theme = &self.theme;
        let style = &resolved.base;
        let color = |color: Option<Color>| color.map(|color| color.hsla(theme));
        let target = Snapshot {
            background: color(style.background),
            color: color(style.color),
            border_color: color(style.border_color),
            opacity: style.opacity,
            width: style.width,
            height: style.height,
        };
        let sheet = self.workspace.stylesheet();
        let transition = style.transition.map(|transition| Transition {
            duration: sheet.scaled(transition.duration),
            // An item's own `delay` (its place in a staggered list) wins
            // over the transition's.
            delay: sheet.scaled(style.delay.unwrap_or(transition.delay)),
            ..transition
        });
        let (now, moving) = self.motion.tweens.borrow_mut().step(
            &place.path,
            Some(state),
            target,
            transition.as_ref(),
            self.now,
        );
        if !moving {
            return;
        }
        self.tweening.set(true);
        let fixed = |hsla: Option<Hsla>| hsla.map(fixed_color);
        let style = &mut resolved.base;
        style.background = fixed(now.background).or(style.background);
        style.color = fixed(now.color).or(style.color);
        style.border_color = fixed(now.border_color).or(style.border_color);
        style.opacity = now.opacity;
        style.width = now.width;
        style.height = now.height;
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
        let crumb = Crumb::node(self, &place.path, place.crumb.clone());
        let place = Place {
            crumb: Some(crumb.clone()),
            ..place.clone()
        };
        let outer = painter.outer.replace(crumb.clone());
        let child = draw_node(self, painter, &place, cx);
        painter.outer = outer;
        let styled = painter.radii.borrow_mut().remove(&place.path);
        Tracked {
            child,
            corners: crate::inspect::corners(self, styled, painter.theme.radius),
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
        Node::Image(image) => image.draw(painter, place, cx),
        Node::Line(line) => line.draw(painter, place, cx),
        Node::Paths(paths) => paths.draw(painter, place, cx),
        Node::Hatch(hatch) => hatch.draw(painter, place, cx),
        Node::Map(map) => map.draw(painter, place, cx),
        Node::Terminal(terminal) => terminal.draw(painter, place, cx),
        Node::Browser(browser) => browser.draw(painter, place, cx),
        Node::Video(video) => video.draw(painter, place, cx),
        Node::View(view) => view.draw(painter, place, cx),
        Node::Input(input) => input.draw(painter, place, cx),
        Node::Markdown(markdown) => markdown.draw(painter, place, cx),
        Node::Region(region) => region.draw(painter, place, cx),
        Node::Divider => elements::divider(painter, place, cx),
        Node::Spacer => div().flex_1().into_any_element(),
        Node::Mount(name) => painter.slot(name, Some(place), cx),
        // Drawn where it is, from the state the app keeps for it.
        Node::Instance(instance) => match painter.workspace.instance(place.owner, instance) {
            Some((key, _)) => painter.slot(key, Some(place), cx),
            None => problem(
                format!("`{}` is not running", instance.key(place.owner)),
                cx,
            ),
        },
        Node::Popover(popover) => popover.draw(painter, place, cx),
        Node::Overlay(overlay) => overlay.draw(painter, place, cx),
        Node::List(list) => list.draw(painter, place, cx),
        Node::Problem(message) => broken(message, place, cx),
    }
}

/// A color mid-move between two states', as the same in light and dark.
fn fixed_color(hsla: Hsla) -> Color {
    let rgba = Rgba::from(hsla);
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u32;
    let packed = byte(rgba.r) << 24 | byte(rgba.g) << 16 | byte(rgba.b) << 8 | byte(rgba.a);
    Color {
        paint: Paint::Fixed(Shade {
            light: packed,
            dark: packed,
        }),
        alpha: 1.0,
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
