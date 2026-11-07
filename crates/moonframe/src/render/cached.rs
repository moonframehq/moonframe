//! Parts of a window kept from one frame to the next, so that an element
//! moving by itself pays for itself and not for what surrounds it.
//!
//! An element that moves on its own, a map, draws in a view of its own: a
//! *live* one. While a window shows one, each run of plain nodes around it
//! (see [`nodes`]) also draws in a view, and the frame is then one of two
//! kinds:
//!
//! - A whole frame draws everything, as a window without a live view always
//!   does, and notes how big each kept view came out.
//! - A live frame is one nothing but a live view asked for. GPUI replays the
//!   kept views as they were painted, which needs them laid out without
//!   reading their content: each takes its own style at the size it measured.
//!
//! Anything else that happens makes the next frame a whole one: the app being
//! notified, a kept view notified (a hover, an image arriving), the window
//! changing size. What GPUI draws again by itself, as a refresh does, is drawn
//! from the node the view keeps, and a view that does not land where it was
//! is never pinned again.

use {
    super::{Cx, Draw, Painter, Place},
    crate::app::MoonframeApp,
    gpui_kit::*,
    moonframe_core::{Direction, Look, Node, Scroll, Stack},
    std::{
        cell::{Cell, RefCell},
        collections::{HashMap, HashSet},
        rc::Rc,
    },
};

/// Views kept ready while a window has a live one: the painter only borrows
/// the app, so it cannot make them. What finds none draws as it always did.
const SPARE: usize = 32;
/// The fewest nodes worth a view of their own.
const FEWEST_NODES: usize = 3;

/// What a window's frames asked for since the last one.
#[derive(Default)]
struct Asked {
    /// A live view wants drawing.
    live: Cell<bool>,
    /// Something else does: the next frame is a whole one.
    whole: Cell<bool>,
}

/// The kept views of a window.
#[derive(Default)]
pub(crate) struct Cached {
    asked: Rc<Asked>,
    /// Hears the app being notified, once made.
    app_watch: Option<Subscription>,
    spare: Vec<Entry>,
    entries: HashMap<String, Entry>,
    /// The paths drawn in this whole frame: the rest have gone.
    drawn: HashSet<String>,
    viewport: Option<Size<Pixels>>,
    /// The frame being drawn is a live one.
    live_frame: bool,
    /// Plain nodes draw in views this frame.
    keeping: bool,
    /// The last frame drew a live view, and this one has so far.
    had_live: bool,
    has_live: bool,
}

impl Cached {
    /// Readies views for the frame about to be drawn.
    pub(crate) fn prepare(&mut self, cx: &mut Context<MoonframeApp>) {
        let asked = self.asked.clone();
        self.app_watch
            .get_or_insert_with(|| cx.observe_self(move |_, _| asked.whole.set(true)));
        let wanted = if self.had_live {
            SPARE
        } else {
            1
        };
        while self.spare.len() < wanted {
            self.spare.push(Entry::new(&self.asked, cx));
        }
    }

    /// Decides what kind of frame this is.
    pub(crate) fn begin(&mut self, viewport: Size<Pixels>, inspecting: bool) {
        let live = self.asked.live.take();
        let whole = self.asked.whole.take();
        let same_size = self.viewport.replace(viewport) == Some(viewport);
        self.keeping = self.had_live && !inspecting;
        self.live_frame = live && !whole && same_size && self.keeping;
        self.has_live = false;
        self.drawn.clear();
    }

    /// The frame is built: what a whole frame did not draw is gone.
    pub(crate) fn finish(&mut self) {
        self.had_live = self.has_live;
        if !self.live_frame {
            let drawn = &self.drawn;
            self.entries.retain(|path, _| drawn.contains(path));
        }
    }

    /// The view of `path`, taking a spare one the first time.
    fn entry(&mut self, path: &str) -> Option<Entry> {
        self.drawn.insert(path.to_owned());
        if let Some(entry) = self.entries.get(path) {
            return Some(entry.clone());
        }
        let entry = self.spare.pop()?;
        self.entries.insert(path.to_owned(), entry.clone());
        Some(entry)
    }
}

/// A view and what is shared with it.
#[derive(Clone)]
struct Entry {
    view: Entity<CachedView>,
    kept: Rc<Kept>,
    _watch: Rc<Subscription>,
}

impl Entry {
    fn new(asked: &Rc<Asked>, cx: &mut Context<MoonframeApp>) -> Self {
        let kept = Rc::new(Kept::default());
        let app = cx.entity().downgrade();
        let view = {
            let kept = kept.clone();
            cx.new(|_| CachedView { kept, app })
        };
        let watch = {
            let (kept, asked) = (kept.clone(), asked.clone());
            cx.observe(&view, move |_, _, _| {
                let asked_for = if kept.live.get() {
                    &asked.live
                } else {
                    &asked.whole
                };
                asked_for.set(true);
            })
        };
        Self {
            view,
            kept,
            _watch: Rc::new(watch),
        }
    }
}

/// What a kept view keeps.
#[derive(Default)]
pub(crate) struct Kept {
    /// What the frame drew for it, until it renders.
    element: RefCell<Option<AnyElement>>,
    /// What to draw again from.
    source: RefCell<Option<Rc<Source>>>,
    /// Where it was laid out.
    bounds: Cell<Option<Bounds<Pixels>>>,
    /// The style of the element at its root.
    style: RefCell<Option<StyleRefinement>>,
    /// GPUI holds a drawing of it at its pinned size.
    warm: Cell<bool>,
    /// It did not land where it was when pinned: it never is again.
    loose: Cell<bool>,
    /// It moves on its own, and is drawn in every frame.
    live: Cell<bool>,
}

/// A node, and where in the window it goes.
pub(crate) struct Source {
    node: Node,
    owner: String,
    path: String,
    parent: Direction,
    /// It is inside a `drag = true` stack.
    in_drag: bool,
}

/// A view around what the frame drew; see the module's comment.
pub(crate) struct CachedView {
    kept: Rc<Kept>,
    app: WeakEntity<MoonframeApp>,
}

impl Render for CachedView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(element) = self.kept.element.take() {
            return element;
        }
        // GPUI is drawing it without a frame having asked: the size it is
        // pinned to may be out of date, which a whole frame puts right.
        let source = self.kept.source.borrow().clone();
        let size = self.kept.bounds.get().map(|bounds| bounds.size);
        let app = self.app.clone();
        draw_whole_next(app.clone(), window);
        let drawn = source.and_then(|source| {
            app.update(cx, |app, cx| {
                app.redraw_kept(&source, size, &self.kept, window, cx)
            })
            .ok()
        });
        drawn.unwrap_or_else(|| Empty.into_any_element())
    }
}

/// Makes the next frame a whole one. During a frame only: notifying then
/// would not reach the app.
fn draw_whole_next(app: WeakEntity<MoonframeApp>, window: &Window) {
    window.on_next_frame(move |_, cx| {
        let _ = app.update(cx, |_, cx| cx.notify());
    });
}

/// `child`, whose bounds are noted as it is laid out. It adds no box of its
/// own. `pinned` is the app to tell when a pinned view has moved.
struct Measured {
    child: AnyElement,
    kept: Rc<Kept>,
    pinned: Option<WeakEntity<MoonframeApp>>,
}

impl IntoElement for Measured {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Measured {
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
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let before = self.kept.bounds.replace(Some(bounds));
        if let Some(app) = self.pinned.take()
            && before != Some(bounds)
        {
            self.kept.loose.set(true);
            draw_whole_next(app, window);
        }
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

/// The node whose element is the root of a kept view, about to be drawn.
pub(super) struct Root {
    path: String,
    kept: Rc<Kept>,
    /// The size it is pinned to, if it is.
    size: Option<Size<Pixels>>,
}

/// A style to edit through `Styled`.
struct Shape(StyleRefinement);

impl Styled for Shape {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.0
    }
}

/// `style` at exactly `size`, whatever its content and its siblings.
fn pinned(style: StyleRefinement, size: Size<Pixels>) -> StyleRefinement {
    let mut shape = Shape(style).w(size.width).h(size.height).flex_none();
    let style = shape.style();
    style.flex_basis = None;
    style.min_size = SizeRefinement::default();
    style.max_size = SizeRefinement::default();
    shape.0
}

impl<'w> Painter<'w, '_> {
    /// For windows that keep what does not move.
    /// `node`, in a view of its own if it is worth one.
    pub(super) fn kept_child(&mut self, node: &'w Node, place: &Place<'w>, cx: Cx) -> AnyElement {
        let entry = match &mut self.cached {
            Some(cached)
                if cached.keeping
                    && self.root.is_none()
                    && !self.in_kept
                    && rooted(node)
                    && nodes(node).is_some_and(|nodes| nodes >= FEWEST_NODES) =>
            {
                cached.entry(&place.path)
            },
            _ => None,
        };
        let Some(entry) = entry else {
            return node.draw(self, place, cx);
        };
        let live_frame = self.cached.as_ref().is_some_and(|cached| cached.live_frame);
        let kept = &entry.kept;
        let same = kept
            .source
            .borrow()
            .as_ref()
            .is_some_and(|source| source.in_drag == self.in_drag && source.node == *node);
        let app = cx.entity().downgrade();
        let shape = kept.style.borrow().clone();
        if let (true, true, false, Some(bounds), Some(shape)) =
            (live_frame, same, kept.loose.get(), kept.bounds.get(), shape)
        {
            if !kept.warm.replace(true) {
                let element = self.draw_root(node, place, kept, Some(bounds.size), cx);
                *kept.element.borrow_mut() = Some(element);
            }
            let child = entry.view.clone().cached(pinned(shape, bounds.size));
            return Measured {
                child: child.into_any_element(),
                kept: kept.clone(),
                pinned: Some(app),
            }
            .into_any_element();
        }
        kept.warm.set(false);
        let element = self.draw_root(node, place, kept, None, cx);
        *kept.element.borrow_mut() = Some(element);
        if !same {
            *kept.source.borrow_mut() = Some(Rc::new(Source {
                node: node.clone(),
                owner: place.owner.to_owned(),
                path: place.path.clone(),
                parent: place.parent,
                in_drag: self.in_drag,
            }));
        }
        Measured {
            child: entry.view.clone().into_any_element(),
            kept: kept.clone(),
            pinned: None,
        }
        .into_any_element()
    }

    /// Draws the root of a kept view, and nothing inside it in views.
    fn draw_root(
        &mut self,
        node: &'w Node,
        place: &Place<'w>,
        kept: &Rc<Kept>,
        size: Option<Size<Pixels>>,
        cx: Cx,
    ) -> AnyElement {
        self.root = Some(Root {
            path: place.path.clone(),
            kept: kept.clone(),
            size,
        });
        let in_kept = std::mem::replace(&mut self.in_kept, true);
        let element = node.draw(self, place, cx);
        self.in_kept = in_kept;
        self.root = None;
        element
    }

    /// `source` again, for GPUI drawing it by itself.
    pub(crate) fn redraw(
        &mut self,
        source: &'w Source,
        size: Option<Size<Pixels>>,
        kept: &Rc<Kept>,
        cx: Cx,
    ) -> AnyElement {
        let place = Place {
            owner: &source.owner,
            path: source.path.clone(),
            parent: source.parent,
            crumb: None,
        };
        self.in_drag = source.in_drag;
        self.draw_root(&source.node, &place, kept, size, cx)
    }

    /// Notes the style of the element at the root of a kept view and, pinned,
    /// has it fill the view exactly: where it sits is the view's to say.
    pub(super) fn at_root<E: Styled>(&mut self, mut element: E, place: &Place) -> E {
        if self
            .root
            .as_ref()
            .is_none_or(|root| root.path != place.path)
        {
            return element;
        }
        let Some(root) = self.root.take() else {
            return element;
        };
        let Some(size) = root.size else {
            *root.kept.style.borrow_mut() = Some(element.style().clone());
            return element;
        };
        let style = element.style();
        *style = pinned(std::mem::take(style), size);
        style.position = Some(Position::Relative);
        style.inset = EdgesRefinement::default();
        style.margin = EdgesRefinement::default();
        element
    }

    /// The live view of the element at `place`, in a window that has them.
    #[cfg_attr(not(feature = "map"), expect(dead_code))]
    pub(super) fn live_view(&mut self, place: &Place) -> Option<EntityId> {
        let cached = self.cached.as_mut()?;
        let entry = cached.entry(&place.path)?;
        cached.has_live = true;
        entry.kept.live.set(true);
        Some(entry.view.entity_id())
    }

    /// `element`, what the element at `place` drew, in its live view.
    #[cfg_attr(not(feature = "map"), expect(dead_code))]
    pub(super) fn in_live_view(&mut self, element: AnyElement, place: &Place) -> AnyElement {
        let Some(entry) = self
            .cached
            .as_ref()
            .and_then(|cached| cached.entries.get(&place.path))
        else {
            return element;
        };
        *entry.kept.element.borrow_mut() = Some(element);
        entry.view.clone().into_any_element()
    }

    /// Asks for another frame of the live view `view`.
    #[cfg_attr(not(feature = "map"), expect(dead_code))]
    pub(super) fn live_again(&mut self, view: EntityId) {
        self.live_again.push(view);
    }
}

/// How many nodes `node` is, if nothing in it needs drawing every frame to
/// stay right: what it shows comes from the node alone. Drawing does more for
/// the rest: a state eases, a scroll position is held, an input, a terminal or
/// a browser marks itself as still there.
fn nodes(node: &Node) -> Option<usize> {
    fn steady(look: &Look) -> Option<usize> {
        look.state.is_none().then_some(1)
    }
    fn stack(stack: &Stack) -> Option<usize> {
        let plain = stack.key.is_none()
            && stack.pointer.is_none()
            && stack.scroll_to.is_none()
            && !stack.drag
            && stack.frame.scroll == Scroll::No;
        if !plain {
            return None;
        }
        let mut total = steady(&stack.look)?;
        for child in &stack.children {
            total += nodes(child)?;
        }
        Some(total)
    }
    match node {
        Node::Stack(node) => stack(node),
        Node::Text(node) => steady(&node.look),
        Node::Button(node) => steady(&node.look),
        Node::Item(node) => steady(&node.look),
        Node::Badge(node) => steady(&node.look),
        Node::Icon(node) if !node.spin => steady(&node.look),
        Node::Image(node) if !super::viewer::wanted(node) => steady(&node.look),
        Node::Line(node) => steady(&node.look),
        Node::Divider | Node::Spacer => Some(1),
        _ => None,
    }
}

/// Whether `node` can be the root of a kept view: its element takes the style
/// a kept view needs from it.
fn rooted(node: &Node) -> bool {
    matches!(node, Node::Stack(_))
}

#[cfg(test)]
mod tests {
    use {
        super::{Cached, nodes, pinned},
        gpui_kit::{Position, Size, StyleRefinement, px, size},
        moonframe_core::{
            Align, Data, Direction, Fit, Frame, Image, Justify, Look, Node, Scroll, Stack,
        },
    };

    fn stack(children: Vec<Node>) -> Stack {
        Stack {
            look: Look::default(),
            direction: Direction::Column,
            frame: Frame {
                gap: None,
                padding: None,
                grow: false,
                width: None,
                height: None,
                surface: None,
                border: None,
                scroll: Scroll::No,
                follow: false,
                shrink: true,
                self_align: None,
                hug: false,
                max_width: None,
                max_height: None,
                float: None,
                area: None,
            },
            align: Align::Stretch,
            justify: Justify::Start,
            wrap: false,
            spread: false,
            selected: false,
            scroll_to: None,
            action: None,
            extras: Default::default(),
            drag: false,
            key: None,
            pointer: None,
            children,
        }
    }

    fn window() -> Size<gpui_kit::Pixels> {
        size(px(800.), px(600.))
    }

    /// A window that has drawn a live view at its size.
    fn drawn() -> Cached {
        let mut cached = Cached::default();
        cached.begin(window(), false);
        cached.has_live = true;
        cached.finish();
        cached
    }

    #[test]
    fn only_a_live_view_asking_makes_a_live_frame() {
        let mut cached = drawn();
        cached.asked.live.set(true);
        cached.begin(window(), false);
        assert!(cached.live_frame);
    }

    #[test]
    fn a_frame_nobody_asked_for_is_whole() {
        // A refresh, or a view notified that is not a kept one.
        let mut cached = drawn();
        cached.begin(window(), false);
        assert!(!cached.live_frame);
        assert!(cached.keeping);
    }

    #[test]
    fn anything_else_asking_makes_the_frame_whole() {
        let mut cached = drawn();
        cached.asked.live.set(true);
        cached.asked.whole.set(true);
        cached.begin(window(), false);
        assert!(!cached.live_frame);
    }

    #[test]
    fn a_new_window_size_makes_the_frame_whole() {
        let mut cached = drawn();
        cached.asked.live.set(true);
        cached.begin(size(px(801.), px(600.)), false);
        assert!(!cached.live_frame);
    }

    #[test]
    fn inspecting_keeps_nothing() {
        let mut cached = drawn();
        cached.asked.live.set(true);
        cached.begin(window(), true);
        assert!(!cached.live_frame);
        assert!(!cached.keeping);
    }

    #[test]
    fn a_window_without_a_live_view_keeps_nothing() {
        let mut cached = Cached::default();
        cached.begin(window(), false);
        cached.finish();
        cached.asked.live.set(true);
        cached.begin(window(), false);
        assert!(!cached.keeping);
        assert!(!cached.live_frame);
    }

    #[test]
    fn plain_nodes_are_counted() {
        let inner = Node::Stack(stack(vec![Node::Divider, Node::Spacer]));
        assert_eq!(nodes(&Node::Stack(stack(vec![inner]))), Some(4));
    }

    #[test]
    fn what_needs_drawing_every_frame_is_not_kept() {
        let mut keyed = stack(Vec::new());
        keyed.key = Some("tab".to_owned());
        let mut scrolled = stack(Vec::new());
        scrolled.frame.scroll = Scroll::Along;
        let mut easing = stack(Vec::new());
        easing.look.state = Some("open".to_owned());
        for node in [keyed, scrolled, easing] {
            assert_eq!(nodes(&Node::Stack(node)), None);
        }
        let holding = stack(vec![Node::Problem("no".to_owned())]);
        assert_eq!(nodes(&Node::Stack(holding)), None);
    }

    #[test]
    fn image_viewers_keep_their_per_frame_bookkeeping() {
        let image = Image {
            look: Look::default(),
            src: "photo.png".to_owned(),
            width: Some(100.),
            height: Some(100.),
            fit: Fit::Contain,
            zoom: None,
            center: None,
            measure: None,
            to: None,
            overlay: Vec::new(),
            extras: Default::default(),
        };
        let container = |image| Node::Stack(stack(vec![Node::Image(image), Node::Divider]));
        assert_eq!(nodes(&container(image.clone())), Some(3));
        for viewer in [
            Image {
                zoom: Some(1.),
                ..image.clone()
            },
            Image {
                measure: Some(Data::from("measured")),
                ..image.clone()
            },
            Image {
                overlay: vec![Node::Divider],
                ..image
            },
        ] {
            assert_eq!(nodes(&container(viewer)), None);
        }
    }

    #[test]
    fn pinning_keeps_where_it_sits_and_drops_how_it_grows() {
        let style = StyleRefinement {
            position: Some(Position::Absolute),
            flex_grow: Some(1.),
            ..StyleRefinement::default()
        };
        let pin = pinned(style, size(px(40.), px(20.)));
        assert_eq!(pin.position, Some(Position::Absolute));
        assert_eq!(pin.flex_grow, Some(0.));
        assert_eq!(pin.size.width, Some(px(40.).into()));
    }
}
