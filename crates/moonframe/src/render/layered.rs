//! The painter's side of host views drawn in layers over the app (see
//! `crate::hosted::layers`): where a view goes in a layer and where in place,
//! and lifting what must stay above one.
//!
//! A layer is painted after all of the app's tree, so anything in the tree
//! painted after a layered view that could cover it is lifted to be painted
//! after the layers too: a float (`float` or `area`), and the problem marker
//! over a mod. A lifted element keeps its layout and what clips it; it is
//! painted with the window's deferred layers, below every popover and
//! overlay, as it was. What follows it in the tree and overlaps it is now
//! under it, where it was over it.

use {
    super::{Cx, Painter, Place, draw_node},
    gpui_kit::*,
    moonframe_core::Node,
};

/// What decides whether a host view goes in a layer, in a window that draws
/// them so.
#[derive(Default)]
pub(super) struct Layering {
    /// Host views put in layers so far this frame.
    layered: usize,
    /// Drawing what is painted after the window's tree: a popover's content
    /// or lifted anchor, a lifted float.
    lifted: usize,
    /// Drawing a keyed child moving in or out, which fades or is covered.
    moving: usize,
}

impl<'w> Painter<'w, '_> {
    /// For the main window, which draws host views in layers over the app.
    pub(crate) fn layering(mut self) -> Self {
        self.layering = Some(Layering::default());
        self
    }

    /// What goes where the host view `view` does: a placeholder it is drawn
    /// over, or the view itself where it cannot be.
    pub(super) fn host(&mut self, view: AnyView) -> AnyElement {
        let layered = self.layer == 0
            && !self.preview
            && self
                .layering
                .as_ref()
                .is_some_and(|layering| layering.lifted == 0 && layering.moving == 0);
        let (true, Some(views), Some(layering)) =
            (layered, self.views.as_deref(), self.layering.as_mut())
        else {
            return view.into_any_element();
        };
        layering.layered += 1;
        views.layers().place(view)
    }

    /// Draws with `draw` what is painted after the window's tree.
    pub(super) fn lifted<R>(&mut self, draw: impl FnOnce(&mut Self) -> R) -> R {
        self.layered_depth(true, draw, |layering| &mut layering.lifted)
    }

    /// Draws with `draw` a keyed child, `moving` in or out.
    pub(super) fn moving<R>(&mut self, moving: bool, draw: impl FnOnce(&mut Self) -> R) -> R {
        self.layered_depth(moving, draw, |layering| &mut layering.moving)
    }

    fn layered_depth<R>(
        &mut self,
        deeper: bool,
        draw: impl FnOnce(&mut Self) -> R,
        depth: fn(&mut Layering) -> &mut usize,
    ) -> R {
        let deeper = deeper && self.layering.is_some();
        if deeper && let Some(layering) = self.layering.as_mut() {
            *depth(layering) += 1;
        }
        let drawn = draw(self);
        if deeper && let Some(layering) = self.layering.as_mut() {
            *depth(layering) -= 1;
        }
        drawn
    }

    /// Whether what is drawn next in the window's tree could be covered by
    /// a host view in a layer, unless it is lifted.
    fn under_layers(&self) -> bool {
        self.layer == 0
            && self
                .layering
                .as_ref()
                .is_some_and(|layering| layering.layered > 0 && layering.lifted == 0)
    }

    /// `element`, lifted over the host views drawn so far if it could be
    /// under one.
    pub(super) fn over_layers(&self, element: AnyElement) -> AnyElement {
        if !self.under_layers() {
            return element;
        }
        Lifted {
            child: Some(element),
        }
        .into_any_element()
    }

    /// `node`, lifted over the host views drawn so far if it floats where it
    /// could be under one.
    pub(super) fn draw_lifted(&mut self, node: &'w Node, place: &Place<'w>, cx: Cx) -> AnyElement {
        if !(floats(node) && self.under_layers()) {
            return draw_node(node, self, place, cx);
        }
        let child = self.lifted(|painter| draw_node(node, painter, place, cx));
        Lifted { child: Some(child) }.into_any_element()
    }
}

/// Whether `node` is placed out of the flow, over what is around it.
fn floats(node: &Node) -> bool {
    let frame = match node {
        Node::Stack(stack) => &stack.frame,
        Node::Panel(panel) => &panel.frame,
        Node::List(list) => &list.frame,
        _ => return false,
    };
    frame.float.is_some() || frame.area.is_some()
}

/// `child` laid out where it is and painted with the window's deferred
/// layers, at the lowest priority, clipped by what clips it here.
struct Lifted {
    child: Option<AnyElement>,
}

impl IntoElement for Lifted {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Lifted {
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
        let layout = match self.child.as_mut() {
            Some(child) => child.request_layout(window, cx),
            None => window.request_layout(Style::default(), None, cx),
        };
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        if let Some(child) = self.child.take() {
            let mask = window.content_mask();
            window.defer_draw(child, window.element_offset(), 0, Some(mask));
        }
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

#[cfg(test)]
mod tests;
