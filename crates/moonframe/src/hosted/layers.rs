//! Host views drawn over the app rather than inside it, so that one drawing
//! again by itself, as a terminal printing does, costs what it costs alone.
//!
//! GPUI draws a view again when it is notified, and every view it was drawn
//! inside with it. Drawn inside the app, a host view drawing again would draw
//! the whole window. Instead the app draws a [`Placeholder`] where a
//! `ui.view` goes, which notes where it lands, and the window's own view
//! ([`crate::app::MainWindow`]) draws the app as a cached view and then
//! [`HostLayers`], which lays each host view out over its placeholder. A host
//! view drawing again then draws only itself, the window's view and GPUI's
//! root: the app and the other host views are replayed as they were painted.
//!
//! Each frame that draws the app notes where every host view goes before the
//! layers are laid out, so a host view follows its place (a new layout, a
//! resized window) in the same frame. A frame that replays the app keeps the
//! places it noted: nothing in the app moved.
//!
//! A host view is painted after all of the app, so what the app puts over it
//! must be painted later still: popovers and overlays are, and the painter
//! lifts the floats drawn after one (see `crate::render::layered`). Where that
//! cannot hold, the view is drawn in place, inside the app, as before: in a
//! popover or an overlay, in a keyed child moving in or out (which fades or
//! covers it), and in windows other than the main one.

use {
    gpui_kit::*,
    std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        rc::Rc,
    },
};

/// The host views the main window draws in layers.
#[derive(Default)]
pub(crate) struct Layers {
    /// What the app's last drawn frame showed, in tree order.
    slots: RefCell<Vec<Slot>>,
    /// The window's view draws the app as a cached view this frame.
    replayed: Cell<bool>,
    /// How many frames drew the app, for tests to see one replayed.
    #[cfg(test)]
    frames: Cell<usize>,
}

/// A host view, and where its placeholder landed.
#[derive(Clone)]
struct Slot {
    view: AnyView,
    spot: Rc<RefCell<Option<Spot>>>,
}

/// Where a placeholder landed, and what it would have drawn its view with.
#[derive(Clone)]
struct Spot {
    bounds: Bounds<Pixels>,
    /// What clips it: the boxes it is in.
    mask: ContentMask<Pixels>,
    text: TextStyle,
    rem: Pixels,
}

impl Layers {
    /// The app starts drawing a frame: what it shows is noted afresh.
    pub(crate) fn begin(&self) {
        self.slots.borrow_mut().clear();
        #[cfg(test)]
        self.frames.set(self.frames.get() + 1);
    }

    /// Whether the app's last frame showed a host view.
    pub(crate) fn any(&self) -> bool {
        !self.slots.borrow().is_empty()
    }

    /// Notes whether the window's view draws the app as a cached view.
    pub(crate) fn set_replayed(&self, replayed: bool) {
        self.replayed.set(replayed);
    }

    /// Whether this frame draws the app as a cached view: GPUI then draws
    /// everything inside it again whenever it draws the app, so keeping parts
    /// of it in views of their own (`crate::render::cached`) gains nothing.
    pub(crate) fn replayed(&self) -> bool {
        self.replayed.get()
    }

    /// How many frames drew the app.
    #[cfg(test)]
    pub(crate) fn frames(&self) -> usize {
        self.frames.get()
    }

    /// What the app draws where `view` goes: it takes the room the view
    /// fills, and the view is drawn over it.
    pub(crate) fn place(&self, view: AnyView) -> AnyElement {
        let spot = Rc::new(RefCell::new(None));
        self.slots.borrow_mut().push(Slot {
            view,
            spot: spot.clone(),
        });
        Placeholder { spot }.into_any_element()
    }

    /// The layers, for the window's view to draw after the app.
    pub(crate) fn element(self: &Rc<Self>) -> HostLayers {
        HostLayers {
            layers: self.clone(),
        }
    }
}

/// Fills its parent's content box, as a host view drawn there does, and notes
/// where it landed.
struct Placeholder {
    spot: Rc<RefCell<Option<Spot>>>,
}

impl IntoElement for Placeholder {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Placeholder {
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
        let style = Style {
            flex_grow: 1.,
            flex_shrink: 1.,
            flex_basis: relative(0.).into(),
            min_size: size(px(0.).into(), px(0.).into()),
            ..Style::default()
        };
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        *self.spot.borrow_mut() = Some(Spot {
            bounds,
            mask: window.content_mask(),
            text: window.text_style(),
            rem: window.rem_size(),
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

/// The host views the app showed, each laid out over its placeholder. It
/// takes no room of its own; it goes after the app.
pub(crate) struct HostLayers {
    layers: Rc<Layers>,
}

/// A host view laid out, and where.
pub(crate) struct Drawn {
    element: AnyElement,
    spot: Spot,
}

impl IntoElement for HostLayers {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for HostLayers {
    type PrepaintState = Vec<Drawn>;
    type RequestLayoutState = ();

    fn id(&self) -> Option<ElementId> {
        Some("host-layers".into())
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
        let style = Style {
            position: Position::Absolute,
            ..Style::default()
        };
        (window.request_layout(style, None, cx), ())
    }

    /// After the app's prepaint, which noted where each view goes this frame
    /// (or kept where it went, replayed).
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<Drawn> {
        let slots = self.layers.slots.borrow().clone();
        let mut seen: HashMap<EntityId, u64> = HashMap::new();
        let mut drawn = Vec::with_capacity(slots.len());
        for slot in slots {
            // Not laid out: in a list's rows out of sight, say.
            let Some(spot) = slot.spot.borrow().clone() else {
                continue;
            };
            let entity = slot.view.entity_id();
            // Named by the view, so it keeps its element state as it moves
            // from one place to another; a view shown twice, by its turn.
            let turn = seen.entry(entity).or_default();
            let id = ElementId::NamedInteger(format!("host-{turn}").into(), entity.as_u64());
            *turn += 1;
            let mut element = div()
                .id(id)
                .size_full()
                .child(slot.view.cached(full()))
                .into_any_element();
            within(&spot, window, |window| {
                let room = spot.bounds.size.map(AvailableSpace::Definite);
                element.layout_as_root(room, window, cx);
                element.prepaint_at(spot.bounds.origin, window, cx);
            });
            drawn.push(Drawn { element, spot });
        }
        drawn
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        drawn: &mut Vec<Drawn>,
        window: &mut Window,
        cx: &mut App,
    ) {
        for Drawn { element, spot } in drawn {
            within(spot, window, |window| element.paint(window, cx));
        }
    }
}

/// Runs `f` as if at the placeholder of `spot`: clipped by what clips it,
/// with its text style and rem size, which the view would have inherited.
fn within<R>(spot: &Spot, window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
    window.with_rem_size(Some(spot.rem), |window| {
        window.with_text_style(Some(refinement(&spot.text)), |window| {
            window.with_content_mask(Some(spot.mask), f)
        })
    })
}

/// A cached view's style: all of its box.
fn full() -> StyleRefinement {
    let mut style = StyleRefinement::default();
    style.size.width = Some(relative(1.).into());
    style.size.height = Some(relative(1.).into());
    style
}

/// `style` as a refinement that sets every field, so that it is the text
/// style whatever is set around it.
fn refinement(style: &TextStyle) -> TextStyleRefinement {
    let style = style.clone();
    TextStyleRefinement {
        color: Some(style.color),
        font_family: Some(style.font_family),
        font_features: Some(style.font_features),
        font_fallbacks: style.font_fallbacks,
        font_size: Some(style.font_size),
        line_height: Some(style.line_height),
        font_weight: Some(style.font_weight),
        font_style: Some(style.font_style),
        background_color: style.background_color,
        underline: style.underline,
        strikethrough: style.strikethrough,
        white_space: Some(style.white_space),
        text_overflow: style.text_overflow,
        text_align: Some(style.text_align),
        line_clamp: style.line_clamp,
    }
}

#[cfg(test)]
mod tests;
