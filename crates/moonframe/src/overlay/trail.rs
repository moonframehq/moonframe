//! The selection card's ladder: every element around the selection, and
//! those inside it down to what was under the pointer, as an indented tree.
//! Hovering a row outlines its element in the window, and clicking selects
//! it instead, so a dense cell's row, list or panel is a click away, and the
//! cell a click back.

use {
    super::dimensions,
    crate::{app::MoonframeApp, inspect::Placed},
    gpui_kit::{
        component::{ActiveTheme as _, Icon, Sizable as _},
        *,
    },
};

/// How far each level sits in from the one around it.
const INDENT: f32 = 10.;
/// The deepest indent: past it, names keep their room in a very deep tree.
const MOST_LEVELS: usize = 14;
/// The tallest the list gets before it scrolls.
const MOST_HEIGHT: f32 = 188.;

/// The icon for an element, from the start of its label.
fn icon_of(label: &str) -> &'static str {
    let end = label.find(['.', ' ']).unwrap_or(label.len());
    match &label[..end] {
        "badge" => "tag",
        "button" => "square-check",
        "column" => "rows-2",
        "hsplit" => "square-split-horizontal",
        "image" => "image",
        "input" => "square-pen",
        "item" | "list" => "list",
        "mod" => "plug",
        "region" => "layout-grid",
        "row" => "columns-2",
        "terminal" => "terminal",
        "text" => "a-large-small",
        "vsplit" => "square-split-vertical",
        _ => "square",
    }
}

fn icon(label: &str) -> Icon {
    let path =
        moonframe_core::IconName::new(icon_of(label)).map_or_else(String::new, |name| name.path());
    Icon::default().path(path).xsmall()
}

impl MoonframeApp {
    pub(super) fn trail(&self, selected: &Placed, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let leaf = self.inspector.leaf.as_ref().unwrap_or(selected);
        let mut ladder = div()
            .id("inspect-ladder")
            .track_scroll(&self.inspector.ladder)
            .overflow_y_scroll()
            .max_h(px(MOST_HEIGHT))
            .mx_3()
            .mt_2()
            .p_1()
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.muted.opacity(0.5))
            .font_family("Menlo")
            .text_xs()
            .text_color(theme.foreground);
        let mut level = 0;
        for (index, crumb) in leaf.crumb.trail().iter().enumerate() {
            let chosen = crumb.path.as_deref() == Some(selected.path.as_str());
            let row = div()
                .id(("inspect-trail", index))
                .flex()
                .flex_none()
                .items_center()
                .gap_1p5()
                .py_0p5()
                .pr_1p5()
                .pl(px(6. + INDENT * level.min(MOST_LEVELS) as f32))
                .rounded(px(5.))
                .child(icon(&crumb.label))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(crumb.label.clone()),
                );
            // A mod's root marker is not drawn: it is text, on its parent's
            // level. (A node scrolled away since this row was drawn is
            // looked for when clicked.)
            let Some(path) = crumb.path.clone() else {
                ladder = ladder.child(row.text_color(theme.muted_foreground));
                continue;
            };
            level += 1;
            let size = self.inspector.drawn_size(&path).map(dimensions);
            let size = div()
                .flex_none()
                .text_color(theme.muted_foreground)
                .children(size);
            ladder = ladder.child(if chosen {
                row.bg(theme.primary.opacity(0.14))
                    .text_color(theme.primary)
                    .child(size)
            } else {
                let hovered = path.clone();
                row.cursor_pointer()
                    .hover(|style| style.bg(theme.muted))
                    .child(size)
                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                        this.preview(&hovered, *hovering, cx);
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| this.select_path(&path, cx)))
            });
        }
        ladder.into_any_element()
    }

    /// Outlines the element of the row the pointer entered, and stops when
    /// it leaves. The next row may be entered before this one is left.
    fn preview(&mut self, path: &str, hovering: bool, cx: &mut Context<Self>) {
        let previewed = &mut self.inspector.previewed;
        if hovering {
            *previewed = Some(path.to_owned());
        } else if previewed.as_deref() == Some(path) {
            *previewed = None;
        } else {
            return;
        }
        cx.notify();
    }

    /// Scrolls the ladder to the selection, which a long one would
    /// otherwise leave out of sight below the elements around it.
    pub(crate) fn reveal_selection(&self) {
        let inspector = &self.inspector;
        let Some(selected) = inspector.selected.as_ref() else {
            return;
        };
        let leaf = inspector.leaf.as_ref().unwrap_or(selected);
        let row = leaf
            .crumb
            .trail()
            .iter()
            .position(|crumb| crumb.path.as_deref() == Some(selected.path.as_str()));
        if let Some(row) = row {
            inspector.ladder.scroll_to_item(row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::icon_of;

    #[test]
    fn every_element_has_a_bundled_icon() {
        for label in [
            "badge.new \"3\"",
            "button \"Save\"",
            "column.mail-cell",
            "hsplit",
            "image \"a.png\"",
            "input",
            "item \"Inbox\"",
            "list.mail-list \"mail\" (4 rows)",
            "mod shell",
            "panel.mail",
            "region \"main\"",
            "row.mail-row",
            "terminal \"t\" (tmux)",
            "text.subject \"Hi\"",
            "vsplit",
        ] {
            assert!(
                moonframe_core::IconName::new(icon_of(label)).is_some(),
                "{label}"
            );
        }
    }

    #[test]
    fn an_icon_follows_the_element_not_its_classes() {
        assert_eq!(icon_of("row.text \"list\""), "columns-2");
        assert_eq!(icon_of("text.row"), "a-large-small");
    }
}
