//! A requested terminal still has a useful diagnostic in a terminal-free build.

use {
    super::{Cx, Draw, Painter, Place, problem},
    gpui_kit::AnyElement,
    moonframe_core::Terminal,
};

impl<'w> Draw<'w> for Terminal {
    fn draw(&'w self, _: &mut Painter<'w, '_>, _: &Place<'w>, cx: Cx) -> AnyElement {
        problem(
            "terminal support was left out of this build; enable `alacritty` or `ghostty`".into(),
            cx,
        )
    }
}
