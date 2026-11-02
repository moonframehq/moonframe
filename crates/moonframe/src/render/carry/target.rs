//! Measure the actual target, outside its padding and scrolling children.

use {
    gpui_kit::*,
    std::{cell::RefCell, rc::Rc},
};

pub(super) struct Measured {
    child: Stateful<Div>,
    measured: Rc<RefCell<Option<Hitbox>>>,
}

impl Measured {
    pub(super) fn state() -> Rc<RefCell<Option<Hitbox>>> {
        Rc::new(RefCell::new(None))
    }

    pub(super) fn new(child: Stateful<Div>, measured: Rc<RefCell<Option<Hitbox>>>) -> Self {
        Self { child, measured }
    }
}

impl IntoElement for Measured {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Measured {
    type PrepaintState = Option<Hitbox>;
    type RequestLayoutState = <Stateful<Div> as Element>::RequestLayoutState;

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.child)
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.child.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.child.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Reuse the div's own hitbox: both its geometry and its eligibility
        // (clipping, occlusion, descendants) match native drop routing.
        let hitbox = self
            .child
            .prepaint(id, inspector, bounds, state, window, cx);
        *self.measured.borrow_mut() = hitbox.clone();
        hitbox
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child
            .paint(id, inspector, bounds, layout, prepaint, window, cx);
    }
}
