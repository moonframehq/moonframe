//! Hover history belongs to a drag session, not its short-lived elements.

use {
    gpui_kit::{Pixels, Point},
    std::{collections::HashMap, hash::Hash},
};

pub(super) struct Hovers<K, V> {
    entered: HashMap<K, (Point<Pixels>, V)>,
}

impl<K, V> Default for Hovers<K, V> {
    fn default() -> Self {
        Self {
            entered: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash, V> Hovers<K, V> {
    pub(super) fn over(&mut self, key: K, at: Point<Pixels>, visit: V) -> bool {
        if let Some((before, current)) = self.entered.get_mut(&key) {
            *current = visit;
            if (at - *before).magnitude() < f64::from(super::STILL) {
                return false;
            }
            *before = at;
        } else {
            self.entered.insert(key, (at, visit));
        }
        true
    }

    pub(super) fn leave(&mut self, key: &K) -> Option<V> {
        self.entered.remove(key).map(|(_, visit)| visit)
    }

    pub(super) fn finish(&mut self, winner: Option<&K>) -> Vec<(K, V)> {
        self.entered
            .drain()
            .filter_map(|(key, (_, visit))| (Some(&key) != winner).then_some((key, visit)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        gpui_kit::{point, px},
        std::{cell::RefCell, rc::Rc},
    };

    #[test]
    fn redraws_keep_entry_and_movement_history() {
        let session = Rc::new(RefCell::new(Hovers::default()));
        assert!(
            session
                .borrow_mut()
                .over("row", point(px(4.), px(8.)), "first frame")
        );
        let redrawn = session.clone();
        assert!(
            !redrawn
                .borrow_mut()
                .over("row", point(px(4.), px(9.)), "next frame")
        );
        assert_eq!(redrawn.borrow_mut().leave(&"row"), Some("next frame"));
        assert_eq!(session.borrow_mut().leave(&"row"), None);
    }

    #[test]
    fn a_nested_drop_ends_other_targets_once() {
        let mut session = Hovers::default();
        session.over("parent", point(px(0.), px(0.)), "column");
        session.over("child", point(px(0.), px(0.)), "row");
        assert_eq!(session.finish(Some(&"child")), vec![("parent", "column")]);
        assert!(session.finish(None).is_empty());
    }

    #[test]
    fn cancellation_ends_every_entered_target() {
        let mut session = Hovers::default();
        session.over("row", point(px(0.), px(0.)), "hover");
        assert_eq!(session.finish(None), vec![("row", "hover")]);
        assert!(session.finish(None).is_empty());
    }
}
