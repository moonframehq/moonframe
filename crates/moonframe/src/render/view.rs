//! A `ui.view`: a box styled by the `view` rule, which the app's own view
//! fills (see `crate::hosted`). Previews, and windows that draw no views,
//! show its name instead.

use {
    super::{Cx, Draw, Painter, Place, problem, styling::*},
    crate::hosted::Shown,
    gpui_kit::{component::ActiveTheme as _, *},
    moonframe_core::{Part, View},
};

impl<'w> Draw<'w> for View {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::View, &self.look, false, place);
        let theme = cx.theme();
        let frame = div()
            .id(place.id())
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        let views = match painter.views.as_deref_mut() {
            Some(views) if !painter.preview => views,
            // A preview shows what goes there; it makes no view.
            _ => {
                return frame
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(format!("the app's `{}` view goes here", self.name))
                    .into_any_element();
            },
        };
        match views.show(place.owner, &self.id, &self.name, &self.props) {
            Shown::View(view) => {
                painter.host_view(view.entity_id());
                frame.child(view).into_any_element()
            },
            Shown::Pending => frame.into_any_element(),
            Shown::Missing(why) => problem(why, cx),
        }
    }
}
