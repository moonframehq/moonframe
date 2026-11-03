//! Diagnostic for a street map requested in a build without its renderer.

use {
    super::{Cx, Draw, Painter, Place, problem},
    gpui_kit::AnyElement,
    moonframe_core::Map,
};

impl<'w> Draw<'w> for Map {
    fn draw(&'w self, _: &mut Painter<'w, '_>, _: &Place<'w>, cx: Cx) -> AnyElement {
        problem(
            "map support was left out of this build; enable `map`".into(),
            cx,
        )
    }
}
