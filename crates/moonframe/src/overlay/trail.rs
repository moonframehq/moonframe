//! The selection card's trail: every element around the selection, and
//! those inside it down to what was under the pointer, each one a button
//! that selects it instead. A dense cell's row, list or panel is a click
//! away, and the cell a click back.

use {
    crate::{app::MoonframeApp, inspect::Placed},
    gpui_kit::{component::ActiveTheme as _, *},
};

impl MoonframeApp {
    pub(super) fn trail(&self, selected: &Placed, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let leaf = self.inspector.leaf.as_ref().unwrap_or(selected);
        let mut trail = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x_1()
            .text_xs()
            .text_color(theme.muted_foreground);
        for (index, crumb) in leaf.crumb.trail().iter().enumerate() {
            if index > 0 {
                trail = trail.child("›");
            }
            let label = crumb.label.clone();
            // A mod's root marker is not drawn: it is text. (What is drawn
            // is only known once this frame is laid out, so a node scrolled
            // away since is looked for when clicked.)
            let path = crumb.path.clone();
            let chosen = crumb.path.as_deref() == Some(selected.path.as_str());
            trail = trail.child(match path {
                Some(path) if !chosen => div()
                    .id(("inspect-trail", index))
                    .rounded(px(4.))
                    .px_0p5()
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.muted).text_color(theme.foreground))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.select_path(&path, cx)))
                    .into_any_element(),
                _ if chosen => div()
                    .rounded(px(4.))
                    .px_0p5()
                    .bg(theme.primary.opacity(0.12))
                    .text_color(theme.primary)
                    .child(label)
                    .into_any_element(),
                _ => div().child(label).into_any_element(),
            });
        }
        trail.into_any_element()
    }
}
