//! What the inspector draws over the window: the highlight, the layer that
//! catches the pointer while picking, and the card for the selection.

use {
    crate::{app::MoonframeApp, inspect},
    gpui_kit::{
        component::{
            ActiveTheme as _, Sizable as _,
            button::{Button, ButtonVariants as _},
        },
        *,
    },
};

/// Above every overlay, popover and the toast, so picking reaches inside an
/// open dialog; the picker, card, banner and label sit one or two above.
const PRIORITY: usize = crate::render::INSPECTOR_LAYER;
/// Height kept free under an element for its label.
const LABEL_ROOM: f32 = 28.;
/// Widest the selection card gets.
const CARD_WIDTH: f32 = 420.;

impl MoonframeApp {
    /// The inspector's layers, if it is picking or has a selection.
    pub(crate) fn inspector_layers(
        &self,
        viewport: Size<Pixels>,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let inspector = &self.inspector;
        if !inspector.picking && inspector.selected.is_none() {
            return Vec::new();
        }
        let theme = cx.theme();
        let mut layers = vec![self.highlight(cx)];
        if inspector.picking {
            layers.push(
                deferred(
                    div()
                        .id("inspect-picker")
                        .absolute()
                        .inset_0()
                        .occlude()
                        .cursor_crosshair()
                        // Picking reaches what is under a page too.
                        .child(crate::browser::cover())
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            let hovered = this.inspector.at(event.position);
                            if hovered.as_ref().map(|placed| &placed.path)
                                != this.inspector.hovered.as_ref().map(|placed| &placed.path)
                            {
                                this.inspector.hovered = hovered;
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                let picked = this.inspector.at(event.position);
                                this.select(picked, cx);
                            }),
                        ),
                )
                .with_priority(PRIORITY + 1)
                .into_any_element(),
            );
            layers.push(banner(theme));
            if let Some(hovered) = &inspector.hovered {
                layers.push(label(hovered, viewport, theme));
            }
        } else if let Some(selected) = &inspector.selected {
            layers.push(self.card(selected, cx));
        }
        layers
    }

    /// Outlines the hovered and selected nodes where this frame drew them.
    fn highlight(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let accent = theme.primary;
        let hovered = self
            .inspector
            .picking
            .then(|| {
                self.inspector
                    .hovered
                    .as_ref()
                    .map(|placed| placed.path.clone())
            })
            .flatten();
        let selected = self
            .inspector
            .selected
            .as_ref()
            .map(|placed| placed.path.clone());
        let recorded = self.inspector.recorded();
        deferred(
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    for placed in recorded.borrow().iter() {
                        if Some(&placed.path) == hovered.as_ref() {
                            window.paint_quad(fill(placed.bounds, accent.opacity(0.12)));
                            window.paint_quad(outline(placed.bounds, accent, BorderStyle::Solid));
                        }
                        if Some(&placed.path) == selected.as_ref() {
                            window.paint_quad(fill(placed.bounds, accent.opacity(0.08)));
                            window.paint_quad(outline(placed.bounds, accent, BorderStyle::Dashed));
                        }
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .with_priority(PRIORITY)
        .into_any_element()
    }

    /// What was picked, how to hand it to an agent, and how to pick again.
    fn card(&self, selected: &inspect::Placed, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let report = inspect::report(selected, &self.workspace);
        deferred(
            div()
                .id("inspect-card")
                .occlude()
                .absolute()
                .right(px(16.))
                .bottom(px(48.))
                .w(px(CARD_WIDTH))
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .rounded(px(10.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .shadow_lg()
                .text_sm()
                .child(crate::browser::cover())
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.primary)
                        .child("Selected element"),
                )
                .child(
                    div()
                        .font_family("Menlo")
                        .text_xs()
                        .text_color(theme.foreground)
                        .child(report),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Saved for agents, who read it when you say \"here\" or \"the selected element\"."),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("inspect-again")
                                .small()
                                .ghost()
                                .label("Pick another")
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_inspect(cx))),
                        )
                        .child(
                            Button::new("inspect-clear")
                                .small()
                                .ghost()
                                .label("Clear")
                                .on_click(cx.listener(|this, _, _, cx| this.select(None, cx))),
                        )
                        .child(
                            Button::new("inspect-copy")
                                .small()
                                .primary()
                                .label("Copy for agent")
                                .on_click(cx.listener(|this, _, _, cx| this.copy_selection(cx))),
                        ),
                ),
        )
        .with_priority(PRIORITY + 1)
        .into_any_element()
    }
}

/// Says what picking does, across the top.
fn banner(theme: &component::Theme) -> AnyElement {
    deferred(
        div()
            .absolute()
            .top(px(8.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .bg(theme.primary)
                    .text_color(theme.primary_foreground)
                    .text_xs()
                    .shadow_md()
                    .child("Inspect: click an element to select it · Esc to stop"),
            ),
    )
    .with_priority(PRIORITY + 2)
    .into_any_element()
}

/// The hovered node's name, under it, or over it near the window's
/// bottom; from its right edge in the window's right half.
fn label(
    hovered: &inspect::Placed,
    viewport: Size<Pixels>,
    theme: &component::Theme,
) -> AnyElement {
    let bounds = hovered.bounds;
    let gap = px(4.);
    let element = div().absolute();
    let element = if bounds.bottom() + px(LABEL_ROOM) > viewport.height {
        element.bottom(viewport.height - bounds.top() + gap)
    } else {
        element.top(bounds.bottom() + gap)
    };
    let element = if bounds.center().x > viewport.width / 2. {
        element.right(viewport.width - bounds.right())
    } else {
        element.left(bounds.left())
    };
    deferred(
        element
            .px_2()
            .py_0p5()
            .rounded(px(4.))
            .bg(theme.foreground)
            .text_color(theme.background)
            .text_xs()
            .font_family("Menlo")
            .whitespace_nowrap()
            .child(hovered.crumb.label.clone()),
    )
    .with_priority(PRIORITY + 2)
    .into_any_element()
}
