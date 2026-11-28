//! The GPUI view: draws a [`Diagram`] with its [`Layout`] in the theme's
//! colors and fonts, pans and zooms, and in [`Mode::Edit`] selects, drags,
//! resizes, connects and creates. It never changes its diagram: each edit
//! is told to its owner as a [`DiagramEvent`], and the owner hands back the
//! diagram it made of it with [`DiagramView::set_diagram`]. Until then the
//! view keeps showing the edit, so nothing jumps back.
//!
//! Layouts of big diagrams run on a background thread; the view keeps the
//! last one meanwhile, then eases nodes to their new places.

// keep-sorted start
pub mod camera;
mod hit;
mod input;
mod paint;
mod scene;
mod snap;
pub mod style;
// keep-sorted end

pub use camera::Camera;
use {
    crate::{
        geometry::{Point, Rect},
        layout::{self, Layout, Options, Widths},
        model::{Diagram, DiagramKind, NodeKind},
    },
    gpui_kit::{
        component::input::{Input, InputEvent, InputState},
        *,
    },
    std::{
        cell::Cell,
        collections::{HashMap, HashSet},
        rc::Rc,
        sync::Arc,
        time::{Duration, Instant},
    },
};

/// Diagrams with up to this many nodes are laid out at once, on the UI
/// thread; bigger ones in the background.
const LAY_OUT_AT_ONCE: usize = 150;
/// How long nodes take to ease to a new layout.
const EASE: Duration = Duration::from_millis(240);

/// What the user may do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    // keep-sorted start
    /// Change it: everything [`Tool`] offers too.
    Edit,
    /// Look: select, pan, zoom, open a node with a double click.
    #[default]
    View,
    // keep-sorted end
}

/// What a press on the canvas does in [`Mode::Edit`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    // keep-sorted start
    Connect,
    Create(NodeKind),
    Hand,
    #[default]
    Select,
    // keep-sorted end
}

/// An edit or a request, for the view's owner to act on.
#[derive(Clone, Debug, PartialEq)]
pub enum DiagramEvent {
    // keep-sorted start
    Connect {
        from: String,
        to: String,
    },
    /// A new node of `kind` drawn at `rect`.
    Create {
        kind: NodeKind,
        rect: Rect,
    },
    /// The chosen nodes and edges, to delete.
    Delete(Vec<String>),
    /// A label edited in place.
    EditLabel {
        id: String,
        label: String,
    },
    /// Nodes dragged to these boxes.
    Move(Vec<(String, Rect)>),
    /// A node double-clicked in [`Mode::View`].
    Open(String),
    Resize {
        id: String,
        rect: Rect,
    },
    /// The chosen nodes and edges, by id.
    Select(Vec<String>),
    /// The tool changed by itself (back to select after a new node).
    Tool(Tool),
    /// The zoom changed, as a fraction (1.0 is 100 %).
    Zoom(f32),
    // keep-sorted end
}

/// Nodes easing from where they were to the new layout's places.
struct Easing {
    from: HashMap<String, Rect>,
    start: Instant,
}

pub struct DiagramView {
    pub(crate) diagram: Arc<Diagram>,
    pub(crate) layout: Arc<Layout>,
    options: Options,
    /// Each label's width as the theme's font draws it.
    widths: HashMap<String, f32>,
    pub(crate) camera: Camera,
    /// The canvas's box in the window, as last drawn.
    pub(crate) bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(crate) mode: Mode,
    pub(crate) tool: Tool,
    pub(crate) selected: HashSet<String>,
    selected_order: Vec<String>,
    pub(crate) highlight: HashSet<String>,
    pub(crate) gesture: Option<input::Gesture>,
    /// Boxes of nodes moved or resized, kept until the owner's diagram
    /// says where they are.
    pub(crate) pending: HashMap<String, Rect>,
    easing: Option<Easing>,
    pub(crate) hover: Option<String>,
    /// The pointer's shape over what it hovers.
    pub(crate) cursor_now: CursorStyle,
    pub(crate) editing: Option<(String, Entity<InputState>, Subscription)>,
    pub(crate) focus: FocusHandle,
    /// Fit the diagram into the view once it has a size and a layout.
    fit_pending: bool,
    /// Scrolling pans only once the view has focus (inside a page that
    /// scrolls, as a chat does).
    pub(crate) scroll_needs_focus: bool,
    pub(crate) told_zoom: f32,
    layout_task: Option<Task<()>>,
    /// A layout is under way on a background thread; a fit waits for it.
    laying_out: bool,
}

impl EventEmitter<DiagramEvent> for DiagramView {}

impl Focusable for DiagramView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl DiagramView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            diagram: Arc::new(Diagram::default()),
            layout: Arc::new(Layout::default()),
            options: Options::default(),
            widths: HashMap::new(),
            camera: Camera::default(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            mode: Mode::View,
            tool: Tool::Select,
            selected: HashSet::new(),
            selected_order: Vec::new(),
            highlight: HashSet::new(),
            gesture: None,
            pending: HashMap::new(),
            easing: None,
            hover: None,
            cursor_now: CursorStyle::Arrow,
            editing: None,
            focus: cx.focus_handle(),
            fit_pending: true,
            scroll_needs_focus: false,
            told_zoom: 1.0,
            layout_task: None,
            laying_out: false,
        }
    }

    pub fn diagram(&self) -> &Diagram {
        &self.diagram
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn camera(&self) -> Camera {
        self.camera
    }

    /// Shows `diagram`, laying it out again from the current layout, so
    /// what stayed keeps its place.
    pub fn set_diagram(
        &mut self,
        mut diagram: Diagram,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A participant only a message names is drawn, chosen and hit as
        // the listed ones are.
        if diagram.kind == DiagramKind::Sequence {
            diagram.nodes = diagram.participants();
        }
        self.measure(&diagram, window, cx);
        let widths = Widths(self.widths.clone());
        let options = self.options.clone();
        let previous = self.layout.clone();
        let diagram = Arc::new(diagram);
        self.diagram = diagram.clone();
        if diagram.nodes.len() <= LAY_OUT_AT_ONCE {
            let out = layout::layout(&diagram, &widths, &options, Some(&previous));
            self.layout_task = None;
            self.apply(out, cx);
            return;
        }
        self.laying_out = true;
        let work = cx.background_spawn(async move {
            layout::layout(&diagram, &widths, &options, Some(&previous))
        });
        self.layout_task = Some(cx.spawn(async move |this, cx| {
            let out = work.await;
            let _ = this.update(cx, |this, cx| this.apply(out, cx));
        }));
    }

    /// A new layout: nodes that were shown ease from where they were (in
    /// a diagram laid out for its author; placed ones jump), edits the
    /// owner has now taken are forgotten.
    fn apply(&mut self, out: Layout, cx: &mut Context<Self>) {
        let laid_out = !self.diagram.nodes.iter().any(|n| n.at.is_some());
        if laid_out && !self.layout.nodes.is_empty() && !self.fit_pending {
            let from: HashMap<String, Rect> = self.layout.nodes.clone().into_iter().collect();
            self.easing = Some(Easing {
                from,
                start: Instant::now(),
            });
        }
        self.laying_out = false;
        // The owner's diagram now says where moved nodes are; an edit it
        // turned down would wait for ever, so none is kept.
        self.pending.clear();
        self.layout = Arc::new(out);
        let known: HashSet<&str> = self
            .diagram
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .chain(self.diagram.edges.iter().map(|e| e.id.as_str()))
            .collect();
        self.selected.retain(|id| known.contains(id.as_str()));
        self.selected_order.retain(|id| known.contains(id.as_str()));
        cx.notify();
    }

    /// Shapes each label once with the theme's font, for the layout.
    fn measure(&mut self, diagram: &Diagram, window: &mut Window, cx: &mut Context<Self>) {
        let size = self.options.font_size;
        let texts = diagram
            .nodes
            .iter()
            .flat_map(|n| std::iter::once(&n.label).chain(n.sublabel.as_ref()))
            .chain(diagram.messages.iter().map(|m| &m.label))
            .chain(diagram.notes.iter().map(|n| &n.text));
        for text in texts {
            if !self.widths.contains_key(text) {
                let width = paint::text_width(text, size, window, cx);
                self.widths.insert(text.clone(), width);
            }
        }
    }

    /// Changing the mode ends the gesture under way; the same mode again,
    /// as an owner's props repeat it, leaves a drag alone.
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if mode != self.mode {
            self.mode = mode;
            self.gesture = None;
            cx.notify();
        }
    }

    pub fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.tool = tool;
        cx.notify();
    }

    /// Chooses these nodes and edges, as the owner keeps the selection.
    pub fn set_selected(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        self.selected = ids.iter().cloned().collect();
        self.selected_order = ids;
        cx.notify();
    }

    pub fn selected(&self) -> &[String] {
        &self.selected_order
    }

    /// Nodes and edges drawn in the accent, as an agent points at them.
    pub fn set_highlight(&mut self, ids: impl IntoIterator<Item = String>, cx: &mut Context<Self>) {
        self.highlight = ids.into_iter().collect();
        cx.notify();
    }

    pub fn set_scroll_needs_focus(&mut self, needs: bool) {
        self.scroll_needs_focus = needs;
    }

    pub fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        let size = self.bounds.get().size;
        let center = Point::new(f32::from(size.width) / 2.0, f32::from(size.height) / 2.0);
        self.camera.zoom_at(self.camera.zoom * factor, center);
        self.tell_zoom(cx);
        cx.notify();
    }

    pub fn zoom_to(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom_by(zoom / self.camera.zoom, cx);
    }

    /// Shows the whole diagram: the one being laid out, if a layout is
    /// under way, once it is done.
    pub fn fit(&mut self, cx: &mut Context<Self>) {
        let size = self.bounds.get().size;
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        if w <= 0.0 || self.layout.nodes.is_empty() || self.laying_out {
            self.fit_pending = true;
            return;
        }
        self.camera = Camera::fit(self.layout.bounds, w, h, 40.0);
        self.fit_pending = false;
        self.tell_zoom(cx);
        cx.notify();
    }

    /// Holds `ids` dragged by (dx, dy), as a press and a move would: for
    /// measuring frames while dragging, with no pointer to drive.
    #[doc(hidden)]
    pub fn hold_drag(&mut self, ids: Vec<String>, dx: f32, dy: f32, cx: &mut Context<Self>) {
        let pressed = ids.first().cloned().unwrap_or_default();
        self.gesture = Some(input::Gesture::Move {
            ids,
            start: Point::default(),
            delta: Point::new(dx, dy),
            guides: Vec::new(),
            travelled: true,
            pressed,
            shift: false,
        });
        cx.notify();
    }

    pub(crate) fn tell_zoom(&mut self, cx: &mut Context<Self>) {
        let zoom = (self.camera.zoom * 100.0).round() / 100.0;
        if zoom != self.told_zoom {
            self.told_zoom = zoom;
            cx.emit(DiagramEvent::Zoom(zoom));
        }
    }

    pub(crate) fn select(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        if ids != self.selected_order {
            self.selected = ids.iter().cloned().collect();
            self.selected_order = ids.clone();
            cx.emit(DiagramEvent::Select(ids));
        }
    }

    /// Where each node is drawn this frame, and how far through easing in.
    pub(crate) fn eased(&mut self) -> (HashMap<String, Rect>, f32) {
        let mut rects: HashMap<String, Rect> = self
            .layout
            .nodes
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        let mut t = 1.0;
        if let Some(easing) = &self.easing {
            let raw = easing.start.elapsed().as_secs_f32() / EASE.as_secs_f32();
            if raw >= 1.0 {
                self.easing = None;
            } else {
                t = 1.0 - (1.0 - raw).powi(3);
                for (id, rect) in &mut rects {
                    if let Some(from) = easing.from.get(id) {
                        *rect = Rect::new(
                            from.x + (rect.x - from.x) * t,
                            from.y + (rect.y - from.y) * t,
                            from.w + (rect.w - from.w) * t,
                            from.h + (rect.h - from.h) * t,
                        );
                    }
                }
            }
        }
        for (id, rect) in &self.pending {
            rects.insert(id.clone(), *rect);
        }
        (rects, t)
    }

    /// Starts editing `id`'s label in place.
    pub(crate) fn edit_label(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(node) = self.diagram.node(&id) else {
            return;
        };
        let label = node.label.clone();
        let state = cx.new(|cx| InputState::new(window, cx));
        state.update(cx, |state, cx| {
            state.set_value(label, window, cx);
            state.focus(window, cx);
        });
        let editing = id.clone();
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |this, state, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    let label = state.read(cx).value().to_string();
                    this.editing = None;
                    this.focus.focus(window, cx);
                    cx.emit(DiagramEvent::EditLabel {
                        id: editing.clone(),
                        label,
                    });
                    cx.notify();
                },
                _ => {},
            },
        );
        self.editing = Some((id, state, subscription));
        cx.notify();
    }
}

impl Render for DiagramView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.fit_pending
            && !self.laying_out
            && f32::from(self.bounds.get().size.width) > 0.0
            && !self.layout.nodes.is_empty()
        {
            self.fit(cx);
        }
        let scene = scene::Scene::build(self);
        if scene.animating {
            window.request_animation_frame();
        }
        let palette = style::Palette::of(cx);
        let view = cx.entity();
        let bounds = self.bounds.clone();
        let camera = self.camera;
        let shown = paint::Shown::of(self);
        let canvas = canvas(
            move |b, _, _| bounds.set(b),
            move |b, (), window, cx| {
                paint::paint(&scene, &shown, camera, b, palette, window, cx);
                input::listen_while_dragging(&view, window);
            },
        )
        .size_full();
        let editor = self.editing.as_ref().and_then(|(id, state, _)| {
            let rect = self.camera.rect_to_screen(*self.layout.nodes.get(id)?);
            let height = 30.0;
            Some(
                div()
                    .absolute()
                    .left(px(rect.x))
                    .top(px(rect.y + rect.h / 2.0 - height / 2.0))
                    .w(px(rect.w.max(120.0)))
                    .child(Input::new(state).focus_bordered(false).h(px(height))),
            )
        });
        div()
            .id("diagram")
            .key_context("Diagram")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(palette.background)
            .cursor(input::cursor(self))
            .on_mouse_down(MouseButton::Left, cx.listener(input::down))
            .on_mouse_down(MouseButton::Middle, cx.listener(input::down))
            .on_mouse_move(cx.listener(input::hover))
            .on_scroll(cx.listener(input::scroll))
            .on_pinch(cx.listener(input::pinch))
            .on_key_down(cx.listener(input::key))
            .child(canvas)
            .children(editor)
    }
}
