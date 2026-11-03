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

mod highlight;

pub(crate) use highlight::Highlight;

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
        let accent = cx.theme().primary;
        let still = cx.reduce_motion();
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
        let state = self.inspector.highlight.clone();
        deferred(
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    let targets = highlight::Targets {
                        hovered: hovered.as_deref(),
                        selected: selected.as_deref(),
                        accent,
                        still,
                    };
                    state
                        .borrow_mut()
                        .paint(&recorded.borrow(), &targets, window);
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
        let pick = inspect::pick(selected, &self.workspace);
        let mono = |text: String| {
            div()
                .font_family("Menlo")
                .text_xs()
                .text_color(theme.foreground)
                .min_w_0()
                .child(text)
        };
        let row = |name: &'static str, value: AnyElement| {
            div()
                .flex()
                .gap_3()
                .child(
                    div()
                        .w(px(44.))
                        .flex_none()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(name),
                )
                .child(div().flex_1().min_w_0().child(value))
        };
        let size = selected.bounds.size;
        deferred(
            div()
                .id("inspect-card")
                .occlude()
                .absolute()
                .right(px(16.))
                .bottom(px(48.))
                .w(px(CARD_WIDTH))
                .flex()
                .flex_col()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .shadow_lg()
                .overflow_hidden()
                .text_sm()
                .child(crate::browser::cover())
                .child(
                    div()
                        .px_3()
                        .pt_3()
                        .pb_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().size(px(8.)).rounded_full().bg(theme.primary))
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child("Selected element"),
                        )
                        .child(
                            div()
                                .font_family("Menlo")
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(dimensions(size)),
                        ),
                )
                .child(
                    div()
                        .mx_3()
                        .px_2()
                        .py_1p5()
                        .rounded(px(8.))
                        .bg(theme.primary.opacity(0.08))
                        .border_1()
                        .border_color(theme.primary.opacity(0.25))
                        .font_family("Menlo")
                        .text_xs()
                        .text_color(theme.primary)
                        .child(pick.element),
                )
                .child(
                    div()
                        .px_3()
                        .pt_2()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(row("mod", mono(pick.owner).into_any_element()))
                        .child(row("file", mono(pick.at).into_any_element()))
                        .child(row(
                            "inside",
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(pick.inside.join(" › "))
                                .into_any_element(),
                        )),
                )
                .child(
                    div()
                        .px_3()
                        .pt_2()
                        .pb_3()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Saved for agents, who read it when you say \"here\" or \"the selected element\"."),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .border_t_1()
                        .border_color(theme.border)
                        .bg(theme.muted.opacity(0.5))
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

/// An element's size, as `120 × 32`.
fn dimensions(size: Size<Pixels>) -> String {
    format!(
        "{} × {}",
        f32::from(size.width).round(),
        f32::from(size.height).round()
    )
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
                    .flex()
                    .items_center()
                    .gap_2()
                    .pl_2()
                    .pr_1()
                    .py_1()
                    .rounded_full()
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .text_color(theme.foreground)
                    .text_xs()
                    .shadow_md()
                    .child(div().size(px(8.)).rounded_full().bg(theme.primary))
                    .child("Click an element to select it")
                    .child(
                        div()
                            .px_1p5()
                            .rounded(px(4.))
                            .bg(theme.muted)
                            .text_color(theme.muted_foreground)
                            .font_family("Menlo")
                            .child("Esc"),
                    ),
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
    let gap = px(4. + 2.);
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
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_0p5()
            .rounded(px(6.))
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .text_xs()
            .font_family("Menlo")
            .whitespace_nowrap()
            .shadow_md()
            .child(hovered.crumb.label.clone())
            .child(div().opacity(0.7).child(dimensions(bounds.size))),
    )
    .with_priority(PRIORITY + 2)
    .into_any_element()
}
