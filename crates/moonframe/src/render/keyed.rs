//! Keyed children: a row's or column's children with a `key` stay
//! themselves as their siblings come and go. One with a `transition` rule
//! grows in when it appears, and when it goes its last drawing stays where
//! it was, shrinking away, as tabs open and close. With `rise` it moves
//! instead, whole, out of the foot of its box, and what replaces it waits.

use {
    super::{Cx, Draw, Painter, Place, Shown},
    crate::motion::{Drawn, Kept},
    gpui_kit::*,
    moonframe_core::{Direction, Node, Part, Stack, Transition, TransitionKind},
    std::rc::Rc,
};

/// How far below its box's top a `rise` child starts and ends, in heights
/// of the box: a little past its foot, so that its shadow leaves too.
const SUNK: f32 = 1.05;

/// The key of a keyed child, and the stack it is.
fn keyed(node: &Node) -> Option<(&Stack, &str)> {
    match node {
        Node::Stack(stack) => stack.key.as_deref().map(|key| (stack, key)),
        _ => None,
    }
}

impl<'w> Place<'w> {
    /// The child keyed `key`, named by its key rather than its index so it
    /// keeps its element state as its siblings move.
    fn keyed(&self, key: &str, parent: Direction) -> Self {
        Self {
            path: format!("{}/#{key}", self.path),
            parent,
            ..self.clone()
        }
    }
}

impl<'w> Painter<'w, '_> {
    /// A container's children, its keyed ones played in and out. Previews
    /// draw them as they are.
    pub(super) fn keyed_children(
        &mut self,
        nodes: &'w [Node],
        place: &Place<'w>,
        direction: Direction,
        cx: Cx,
    ) -> Vec<AnyElement> {
        let last = match self.last_keyed {
            Some(last) if nodes.iter().any(|node| keyed(node).is_some()) => last,
            _ => return self.children(nodes, place, direction, cx),
        };
        // `None` when the container was not drawn last frame: what it holds
        // as it appears is where it starts, not something arriving.
        let before = last.containers.get(&place.path);
        let sinking = before.is_some_and(|before| {
            before.kept.iter().any(|kept| {
                let stays = nodes
                    .iter()
                    .any(|node| keyed(node).is_some_and(|(_, key)| key == kept.key));
                let child = place.keyed(&kept.key, direction);
                kept.transition.kind == TransitionKind::Rise
                    && !stays
                    && !self.shown(&child, false, Some(&kept.transition)).hidden()
            })
        });
        let mut drawn = Drawn::default();
        let mut elements = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            let Some((stack, key)) = keyed(node) else {
                elements.push(node.draw(self, &place.child(index, direction), cx));
                continue;
            };
            let child = place.keyed(key, direction);
            let Some(transition) = self.keyed_transition(stack, &child) else {
                drawn.keys.push(key.to_owned());
                elements.push(node.draw(self, &child, cx));
                continue;
            };
            let arriving = before.is_some_and(|before| !before.keys.iter().any(|seen| seen == key));
            // Not drawn, nor noted as drawn: it arrives once the way is clear.
            if arriving && sinking && transition.kind == TransitionKind::Rise {
                continue;
            }
            drawn.keys.push(key.to_owned());
            // One called back while leaving turns round where it is.
            let leaving =
                before.is_some_and(|before| before.kept.iter().any(|kept| kept.key == key));
            if arriving && !leaving {
                self.motion.enter(&child.path);
            }
            let shown = self.shown(&child, true, Some(&transition));
            // The last frame's copy, if the child has not changed since.
            let same = before
                .and_then(|before| before.kept.iter().find(|kept| kept.key == key))
                .filter(|kept| *kept.node == *node)
                .map(|kept| kept.node.clone());
            drawn.kept.push(Kept {
                key: key.to_owned(),
                index,
                node: same.unwrap_or_else(|| Rc::new(node.clone())),
                transition,
            });
            let element = node.draw(self, &child, cx);
            elements.push(self.sized(element, shown, transition.kind, stack, &child, false));
        }
        // What went since the last frame, drawn where it was until it is
        // gone; it is kept again, for the frames still to come.
        let mut leaving: Vec<&'w Kept> = before
            .into_iter()
            .flat_map(|before| &before.kept)
            .filter(|kept| !drawn.keys.contains(&kept.key))
            .collect();
        leaving.sort_by_key(|kept| kept.index);
        for kept in leaving {
            let child = place.keyed(&kept.key, direction);
            let shown = self.shown(&child, false, Some(&kept.transition));
            let Node::Stack(stack) = &*kept.node else {
                continue;
            };
            if shown.hidden() {
                self.motion.forget(&child.path);
                continue;
            }
            let element = kept.node.draw(self, &child, cx);
            let element = self.sized(element, shown, kept.transition.kind, stack, &child, true);
            elements.insert(kept.index.min(elements.len()), element);
            drawn.kept.push(Kept {
                key: kept.key.clone(),
                index: kept.index,
                node: kept.node.clone(),
                transition: kept.transition,
            });
        }
        self.motion.drew(place.path.clone(), drawn);
        elements
    }

    /// How a keyed stack comes and goes, if its rules give it a transition
    /// that moves.
    fn keyed_transition(&self, stack: &Stack, place: &Place<'w>) -> Option<Transition> {
        let part = match stack.direction {
            Direction::Row => Part::Row,
            Direction::Column => Part::Column,
        };
        self.look(part, &stack.look, stack.selected, place)
            .base
            .transition
            .filter(|transition| transition.kind != TransitionKind::None)
            .filter(|transition| !transition.duration.is_zero())
    }

    /// A keyed child in a box that measures it at rest and, while it moves,
    /// shows `amount` of it along its parent's axis, faded as its
    /// transition says. One that is leaving takes no clicks.
    fn sized(
        &mut self,
        element: AnyElement,
        shown: Shown,
        kind: TransitionKind,
        stack: &Stack,
        place: &Place<'w>,
        leaving: bool,
    ) -> AnyElement {
        if kind == TransitionKind::Rise {
            return risen(element, shown, leaving);
        }
        let size = self.motion.size(&place.path);
        let measured = size.get();
        let outer = div().flex().on_children_prepainted({
            move |bounds: Vec<Bounds<Pixels>>, _: &mut Window, _: &mut App| {
                if let Some(bounds) = bounds.first() {
                    size.set(Some(bounds.size));
                }
            }
        });
        let outer = match place.parent {
            Direction::Row => outer,
            Direction::Column => outer.flex_col(),
        };
        let Some(amount) = shown.moving() else {
            // At rest it takes the room the child itself would.
            let outer = if stack.frame.grow {
                outer.flex_1()
            } else {
                outer
            };
            let outer = if stack.frame.shrink {
                outer.min_w_0().min_h_0()
            } else {
                outer.flex_shrink_0()
            };
            return outer.child(element).into_any_element();
        };
        let slides = matches!(kind, TransitionKind::Slide | TransitionKind::SlideFade);
        let fades = matches!(kind, TransitionKind::Fade | TransitionKind::SlideFade);
        // The child keeps its size at rest inside a box that clips it.
        let inner = div().flex().flex_none();
        let (outer, inner) = match (place.parent, measured) {
            (Direction::Row, Some(size)) => (
                if slides {
                    outer.w(size.width * amount)
                } else {
                    outer
                },
                inner.w(size.width),
            ),
            (Direction::Column, Some(size)) => (
                if slides {
                    outer.h(size.height * amount)
                } else {
                    outer
                },
                inner.flex_col().h(size.height),
            ),
            // Not measured yet: hidden for the frame that measures it.
            (Direction::Row, None) => (outer.w(px(0.)), inner),
            (Direction::Column, None) => (outer.h(px(0.)), inner.flex_col()),
        };
        let outer = outer.flex_none().overflow_hidden().relative();
        let outer = if fades {
            outer.opacity(amount)
        } else {
            outer
        };
        let outer = outer.child(inner.child(element));
        let outer = if leaving {
            outer.child(div().absolute().inset_0().occlude())
        } else {
            outer
        };
        outer.into_any_element()
    }
}

/// A `rise` child: it fills its box and moves as a whole, out of a box that
/// need not clip it, without being measured.
fn risen(element: AnyElement, shown: Shown, leaving: bool) -> AnyElement {
    let sunk = shown.moving().map_or(0., |amount| (1. - amount) * SUNK);
    let outer = div()
        .absolute()
        .left_0()
        .top(relative(sunk))
        .size_full()
        .flex()
        .flex_col()
        .child(element);
    let outer = if leaving {
        outer.child(div().absolute().inset_0().occlude())
    } else {
        outer
    };
    outer.into_any_element()
}
