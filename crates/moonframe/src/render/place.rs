//! A mounted mod shares its state, but each place it is drawn has its own
//! element identities, layout measurements and cached views.

use super::Place;

impl<'w> Place<'w> {
    /// The root of `owner` at this mount or region, retaining the mounting
    /// path even when another occurrence draws the same mod or instance.
    pub(super) fn mounted(&self, owner: &'w str) -> Self {
        Self {
            path: format!("{}/@{owner}", self.path),
            ..Self::root(owner)
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::Place, moonframe_core::Direction};

    #[test]
    fn repeated_mounts_have_distinct_descendant_identities() {
        let shell = Place::root("shell");
        let left = shell.child(0, Direction::Row).mounted("map");
        let right = shell.child(1, Direction::Row).mounted("map");
        let left_map = left.child(0, Direction::Column);
        let right_map = right.child(0, Direction::Column);
        assert_eq!(left_map.owner, right_map.owner);
        assert_ne!(left_map.path, right_map.path);
        assert_ne!(left_map.id(), right_map.id());
        // The same occurrence keeps its identity on the next frame.
        assert_eq!(
            left_map.path,
            shell
                .child(0, Direction::Row)
                .mounted("map")
                .child(0, Direction::Column)
                .path
        );
    }

    #[test]
    fn nested_mounts_and_regions_keep_their_ancestry() {
        let shell = Place::root("shell");
        let left = shell.child(0, Direction::Row).mounted("panel");
        let right = shell.child(1, Direction::Row).mounted("panel");
        let nested = |place: &Place| {
            place
                .child(0, Direction::Column)
                .mounted("viewer#shared")
                .child(0, Direction::Column)
                .path
        };
        assert_ne!(nested(&left), nested(&right));
        // Region members are identified by name, not their order.
        assert_ne!(left.mounted("first").path, left.mounted("second").path);
        assert_ne!(left.mounted("first").path, right.mounted("first").path);
    }
}
