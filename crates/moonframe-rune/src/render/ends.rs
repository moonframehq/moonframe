//! A scrolling row's or column's `at_end` and `scroll_end`, as on GPUI
//! (`crates/moonframe/src/render/ends.rs`): the mod hears as it leaves or
//! reaches its end, and a new `scroll_end` scrolls there. A view cannot
//! send or scroll, so both wait here for the next poll.

use {
    super::{Painter, Place},
    moonframe_core::{Data, Direction, Ends, Frame, Scroll, with_fields},
    rune::{El, Tx},
    std::{
        cell::RefCell,
        collections::{BTreeMap, HashMap, HashSet},
        rc::Rc,
    },
};

/// How near the end still counts as there, as on GPUI.
const SLACK: f32 = 24.;

/// What stacks asked of their ends, and what they last told.
#[derive(Default)]
pub(crate) struct StackEnds {
    /// The last `scroll_end` each stack took, by its key; absent once a
    /// stack draws without one, so the same value again is a new request.
    asked: RefCell<HashMap<String, Data>>,
    /// Stacks to scroll to their end next poll: key, and whether across.
    scrolls: RefCell<Vec<(String, bool)>>,
    /// Whether each stack was last told it is at its end, by key.
    told: Rc<RefCell<HashMap<String, bool>>>,
    /// Messages for mods: sender, recipient and message.
    notes: Rc<RefCell<Vec<(String, String, Data)>>>,
    /// The stacks with ends the view being drawn drew, by key.
    drawn: RefCell<HashSet<String>>,
}

impl StackEnds {
    /// The view is drawn: stacks it no longer drew are forgotten, so one
    /// drawn there again later (a chat reopened) starts afresh and tells
    /// its mod where its end is.
    pub(crate) fn drawn(&self) {
        let drawn = self.drawn.take();
        self.told.borrow_mut().retain(|key, _| drawn.contains(key));
        self.asked.borrow_mut().retain(|key, _| drawn.contains(key));
    }

    /// Carries out the scrolls stacks asked for while drawing, and hands
    /// back what they have to tell their mods.
    pub(crate) fn settle(&self, tx: &mut Tx) -> Vec<(String, String, Data)> {
        for (key, across) in self.scrolls.take() {
            if across {
                // Clamped to the content's far edge.
                tx.scroll_to(key, f32::MAX, 0.);
            } else {
                tx.scroll_to_bottom(key);
            }
        }
        self.notes.take()
    }
}

/// Whether a stack of `direction` scrolling as `scroll` scrolls only
/// sideways.
fn across(direction: Direction, scroll: Scroll) -> bool {
    matches!(
        (direction, scroll),
        (Direction::Row, Scroll::Along) | (_, Scroll::X)
    )
}

impl<'w> Painter<'w> {
    /// `element`, the scrolling stack at `place`, following its `ends`.
    pub(super) fn ends(
        &self,
        element: El,
        ends: Option<&Ends>,
        direction: Direction,
        frame: &Frame,
        place: &Place<'w>,
    ) -> El {
        let state = &self.ui.stack_ends;
        let key = place.path.clone();
        if ends.is_some() {
            state.drawn.borrow_mut().insert(key.clone());
        }
        let across = across(direction, frame.scroll);
        match ends.and_then(|ends| ends.scroll_end.as_ref()) {
            Some(asked) => {
                let mut seen = state.asked.borrow_mut();
                if seen.get(&key) != Some(asked) {
                    seen.insert(key.clone(), asked.clone());
                    state.scrolls.borrow_mut().push((key.clone(), across));
                }
            },
            None => {
                let mut seen = state.asked.borrow_mut();
                if !seen.is_empty() {
                    seen.remove(&key);
                }
            },
        }
        let Some(msg) = ends.and_then(|ends| ends.at_end.clone()) else {
            return element;
        };
        let (told, notes) = (Rc::clone(&state.told), Rc::clone(&state.notes));
        let owner = place.owner.to_owned();
        element.on_scroll_offset(move |position| {
            let axis = usize::from(!across);
            let end = position.offset[axis] >= position.max[axis] - SLACK;
            if told.borrow_mut().insert(key.clone(), end) == Some(end) {
                return;
            }
            let msg = with_fields(
                &msg,
                BTreeMap::from([("at_end".to_owned(), Data::Bool(end))]),
            );
            notes.borrow_mut().push((owner.clone(), owner.clone(), msg));
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::across,
        moonframe_core::{Direction, Scroll},
    };

    #[test]
    fn a_row_scrolls_across_and_a_column_down() {
        assert!(across(Direction::Row, Scroll::Along));
        assert!(across(Direction::Column, Scroll::X));
        assert!(!across(Direction::Column, Scroll::Along));
        assert!(!across(Direction::Row, Scroll::Both));
    }
}
