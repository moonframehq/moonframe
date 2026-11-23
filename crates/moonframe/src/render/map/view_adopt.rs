//! A map moved by something other than its own gestures.

use {
    super::{Frame, KEPT},
    moonframe_core::world,
};

/// The map `key` moved by something other than its own gestures -- the
/// gpui-maps view of a `vector-*` provider, which takes the gestures there --
/// to `(lat, lon)` at `zoom`, turned to `bearing`: kept as if the user had
/// left it there. `None` before the map was first drawn.
pub(in super::super) fn adopt(
    key: &str,
    lat: f64,
    lon: f64,
    zoom: f64,
    bearing: f64,
) -> Option<Frame> {
    KEPT.with_borrow_mut(|kept| {
        let entry = kept.get_mut(key)?;
        let drawn = entry.drawn?;
        let bearing = bearing.rem_euclid(360.0);
        let frame = Frame {
            center: world(lat, lon),
            zoom,
            bearing,
            ..drawn
        };
        entry.moved = Some(frame);
        entry.bearing = bearing;
        entry.glide = None;
        entry.heading = None;
        entry.pinch = None;
        entry.turn = None;
        Some(frame)
    })
}
