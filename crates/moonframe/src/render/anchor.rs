//! A column that scrolls keeps what is being read in place: while it is
//! scrolled down, its first keyed child showing stays where it is on screen
//! as children arrive, go or move above it, as a `ui.list`'s first row
//! does. At the top nothing is kept: arrivals join the top.
//!
//! Each keyed child, as it draws inside such a column, is wrapped so that
//! its layout is noted as the window lays out. The column itself is wrapped
//! so that, once laid out and before it places its children, it scrolls by
//! as much as the kept child moved: the frame shows it in place, with no
//! frame in between. After the children are placed, it notes the first
//! keyed child showing, and how far below the column's top it is.

use {
    super::{Painter, Place},
    gpui_kit::*,
    moonframe_core::{Direction, Node, Scroll, Stack},
    std::{
        cell::RefCell,
        collections::HashMap,
        rc::Rc,
        time::{Duration, Instant},
    },
};

/// Less than this is layout's rounding, not a move.
const SLACK: Pixels = px(0.5);
/// How long a column not drawn is remembered.
const FORGET: Duration = Duration::from_secs(60);

thread_local! {
    /// The child each column keeps in place, by window and place.
    static KEPT: RefCell<HashMap<String, Kept>> = RefCell::new(HashMap::new());
}

/// The first keyed child a column showed.
#[derive(Clone, Debug, PartialEq)]
struct Kept {
    key: String,
    /// How far its top was below the column's top, on screen.
    below: Pixels,
    /// How far the column was scrolled then (GPUI's offset, at or below 0):
    /// a wheel since moves the child with everything else.
    offset: Pixels,
    seen: Instant,
}

/// A column that keeps its first keyed child in place, while its children
/// draw: each keyed one notes its layout here.
#[derive(Debug, Default)]
pub(crate) struct Anchoring {
    path: String,
    laid: RefCell<Vec<(String, LayoutId)>>,
}

/// Before `stack` draws its children: the anchoring its keyed children note
/// their layout in, if it scrolls down and has any.
pub(super) fn begin(
    stack: &Stack,
    painter: &mut Painter<'_, '_>,
    place: &Place<'_>,
) -> Option<Rc<Anchoring>> {
    let down = matches!(
        (stack.frame.scroll, stack.direction),
        (Scroll::Along, Direction::Column) | (Scroll::Y | Scroll::Both, _)
    );
    let keyed = stack
        .children
        .iter()
        .any(|child| matches!(child, Node::Stack(child) if child.key.is_some()));
    if !down || !keyed || painter.preview {
        return None;
    }
    let anchoring = Rc::new(Anchoring {
        path: place.path.clone(),
        laid: RefCell::default(),
    });
    painter.anchoring.push(anchoring.clone());
    Some(anchoring)
}

/// Once its children are drawn.
pub(super) fn end(painter: &mut Painter<'_, '_>, anchoring: Option<&Rc<Anchoring>>) {
    if anchoring.is_some() {
        painter.anchoring.pop();
    }
}

/// `element`, the stack `stack` drew at `place`, kept in place as a column
/// that `anchoring`s (scrolled by `handle`), and noting its layout as a
/// keyed child of the column it is directly in.
pub(super) fn wrap(
    element: AnyElement,
    stack: &Stack,
    anchoring: Option<Rc<Anchoring>>,
    handle: &ScrollHandle,
    painter: &Painter<'_, '_>,
    place: &Place<'_>,
) -> AnyElement {
    let element = match anchoring {
        Some(anchoring) => Keeper {
            inner: element,
            anchoring,
            handle: handle.clone(),
            key: format!("{:?}/{}", painter.window, place.path),
        }
        .into_any_element(),
        None => element,
    };
    let around = painter.anchoring.last().zip(stack.key.as_deref());
    match around {
        // Directly in that column: keyed children are drawn at `/#key`.
        Some((column, key)) if place.path == format!("{}/#{key}", column.path) => Noter {
            inner: element,
            key: key.to_owned(),
            anchoring: column.clone(),
        }
        .into_any_element(),
        _ => element,
    }
}

/// How far to scroll (GPUI's offset) so that the child `kept` showed, now
/// laid out with its top at `top`, is as far below `column_top` as it was;
/// `offset` is how far the column is scrolled now. `None` when it is there.
fn kept_in_place(kept: &Kept, top: Pixels, column_top: Pixels, offset: Pixels) -> Option<Pixels> {
    // A wheel since moved it with everything else.
    let below = kept.below + (offset - kept.offset);
    let wanted = (column_top + below - top).min(px(0.));
    ((wanted - offset).abs() > SLACK).then_some(wanted)
}

/// The first of `laid` (each child's key and top and height, not scrolled)
/// that shows in a column at `column_top` scrolled by `offset`; none at the
/// top, where arrivals join it.
fn first_showing(
    laid: impl IntoIterator<Item = (String, Pixels, Pixels)>,
    column_top: Pixels,
    offset: Pixels,
    now: Instant,
) -> Option<Kept> {
    if offset > -SLACK {
        return None;
    }
    laid.into_iter().find_map(|(key, top, height)| {
        let top = top + offset;
        (top + height > column_top + SLACK).then(|| Kept {
            key,
            below: top - column_top,
            offset,
            seen: now,
        })
    })
}

/// The column, wrapped: it moves its scroll before placing its children.
struct Keeper {
    inner: AnyElement,
    anchoring: Rc<Anchoring>,
    handle: ScrollHandle,
    key: String,
}

impl IntoElement for Keeper {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Keeper {
    type PrepaintState = ();
    type RequestLayoutState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (self.inner.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let laid = self.anchoring.laid.borrow().clone();
        let now = Instant::now();
        let kept = KEPT.with_borrow_mut(|kept| {
            kept.retain(|_, kept| now.saturating_duration_since(kept.seen) < FORGET);
            kept.get(&self.key).cloned()
        });
        let offset = self.handle.offset();
        let moved = kept
            .as_ref()
            .filter(|_| offset.y < -SLACK)
            .and_then(|kept| {
                let (_, id) = laid.iter().find(|(key, _)| *key == kept.key)?;
                kept_in_place(
                    kept,
                    window.layout_bounds(*id).top(),
                    bounds.top(),
                    offset.y,
                )
            });
        if let Some(y) = moved {
            self.handle.set_offset(point(offset.x, y));
        }
        self.inner.prepaint(window, cx);
        let offset = self.handle.offset().y;
        let showing = first_showing(
            laid.into_iter().map(|(key, id)| {
                let laid = window.layout_bounds(id);
                (key, laid.top(), laid.size.height)
            }),
            bounds.top(),
            offset,
            now,
        );
        KEPT.with_borrow_mut(|kept| match showing {
            Some(showing) => {
                kept.insert(self.key.clone(), showing);
            },
            None => {
                kept.remove(&self.key);
            },
        });
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
        self.inner.paint(window, cx);
    }
}

/// A keyed child, wrapped: it notes its layout in its column's anchoring.
struct Noter {
    inner: AnyElement,
    key: String,
    anchoring: Rc<Anchoring>,
}

impl IntoElement for Noter {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Noter {
    type PrepaintState = ();
    type RequestLayoutState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let id = self.inner.request_layout(window, cx);
        self.anchoring
            .laid
            .borrow_mut()
            .push((self.key.clone(), id));
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.prepaint(window, cx);
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
        self.inner.paint(window, cx);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
