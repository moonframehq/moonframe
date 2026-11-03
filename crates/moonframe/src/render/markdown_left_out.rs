//! Diagnostic for rich Markdown requested in a build without its renderer.

use {
    super::{Cx, Draw, Painter, Place, problem},
    gpui_kit::AnyElement,
    moonframe_core::Markdown,
};

impl<'w> Draw<'w> for Markdown {
    fn draw(&'w self, _: &mut Painter<'w, '_>, _: &Place<'w>, cx: Cx) -> AnyElement {
        problem(
            "Markdown support was left out of this build; enable `markdown`".into(),
            cx,
        )
    }
}
