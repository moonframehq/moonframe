//! The stylesheet's and the providers' problems: a small red pill at the
//! top of the window, over the title bar rather than in the layout, so a
//! broken stylesheet or provider never moves anything. A click opens the
//! whole message, with Retry (a provider), Copy, and Dismiss.

use {
    super::*,
    gpui_kit::component::{
        Icon, Sizable as _,
        button::{Button, ButtonVariants as _},
    },
};

/// Over the shell and its popovers, under overlays and the inspector.
const PRIORITY: usize = 1;
/// How wide the details get.
const DETAILS_WIDTH: f32 = 440.;

/// Whether the details are open, and what was dismissed.
#[derive(Default)]
pub(super) struct Problems {
    open: bool,
    /// Problems dismissed, by their text: hidden until they change.
    dismissed: Vec<String>,
}

/// One problem, as the pill and the details show it.
#[derive(Clone)]
struct Problem {
    /// The provider it is from; none for the stylesheet.
    provider: Option<String>,
    /// `style` or the provider's name.
    source: String,
    message: String,
}

impl Problem {
    /// What dismissing it remembers, and what Copy copies.
    fn text(&self) -> String {
        format!("{}: {}", self.source, self.message)
    }
}

impl MoonframeApp {
    fn problems(&self) -> Vec<Problem> {
        let style = self.workspace.style_error().map(|error| Problem {
            provider: None,
            source: "style".to_owned(),
            message: error.to_string(),
        });
        let providers = self.workspace.host_problems().map(|(name, error)| Problem {
            provider: Some(name.to_owned()),
            source: name.to_owned(),
            message: error.to_string(),
        });
        style.into_iter().chain(providers).collect()
    }

    /// The pill and, while open, the details; nothing without a problem.
    pub(super) fn problems_layer(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let all = self.problems();
        // A problem that went (or changed) is shown again if it comes back.
        self.problem_state
            .dismissed
            .retain(|text| all.iter().any(|problem| problem.text() == *text));
        let shown: Vec<Problem> = all
            .into_iter()
            .filter(|problem| !self.problem_state.dismissed.contains(&problem.text()))
            .collect();
        if shown.is_empty() {
            self.problem_state.open = false;
            return Vec::new();
        }
        let theme = cx.theme();
        let summary = match shown.as_slice() {
            [one] if one.provider.is_some() => format!("{} provider failed", one.source),
            [_] => "style.luau has a problem".to_owned(),
            many => format!("{} problems", many.len()),
        };
        let pill = div()
            .id("problems-pill")
            .occlude()
            .flex()
            .items_center()
            .gap_1()
            .pl_1p5()
            .pr_2()
            .py_0p5()
            .rounded_full()
            .border_1()
            .border_color(theme.danger.opacity(0.4))
            .bg(theme.background)
            .text_xs()
            .text_color(theme.danger)
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(icon("circle-alert").xsmall().text_color(theme.danger))
                    .child(summary),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.problem_state.open = !this.problem_state.open;
                cx.notify();
            }));
        let mut layers = vec![
            deferred(
                div()
                    .absolute()
                    .top(px(6.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        // The pill tinted on the window's own background.
                        div()
                            .rounded_full()
                            .bg(theme.danger.opacity(0.08))
                            .child(pill),
                    ),
            )
            .with_priority(PRIORITY + 1)
            .into_any_element(),
        ];
        if self.problem_state.open {
            layers.push(self.problem_details(&shown, cx));
        }
        layers
    }

    /// Each problem in full, with what can be done about it; a click
    /// anywhere else closes it.
    fn problem_details(&self, shown: &[Problem], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let entries = shown.iter().enumerate().map(|(index, problem)| {
            let retry = problem.provider.clone().map(|name| {
                Button::new(("problem-retry", index))
                    .small()
                    .outline()
                    .label("Retry")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Gone until it fails again.
                        this.workspace.clear_host(&name, None);
                        if let Err(problem) = this.providers.restart(&name) {
                            tracing::warn!("{problem}");
                        }
                        this.problem_state.open = false;
                        cx.notify();
                    }))
            });
            let text = problem.text();
            let copied = text.clone();
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .when(index > 0, |entry| {
                    entry.pt_3().border_t_1().border_color(theme.border)
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .child(icon("circle-alert").small().text_color(theme.danger))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child(match &problem.provider {
                                    Some(name) => format!("{name}: provider failed"),
                                    None => "style.luau".to_owned(),
                                }),
                        ),
                )
                .child(
                    div()
                        .id(("problem-message", index))
                        .max_h(px(160.))
                        .overflow_y_scroll()
                        .font_family("Menlo")
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(strip_prefix(&problem.message)),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .children(retry)
                        .child(
                            Button::new(("problem-copy", index))
                                .small()
                                .ghost()
                                .label("Copy")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.copy(copied.clone(), cx);
                                })),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new(("problem-dismiss", index))
                                .small()
                                .ghost()
                                .label("Dismiss")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.problem_state.dismissed.push(text.clone());
                                    cx.notify();
                                })),
                        ),
                )
        });
        deferred(
            div()
                .absolute()
                .inset_0()
                .child(
                    div()
                        .id("problems-backdrop")
                        .absolute()
                        .inset_0()
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                this.problem_state.open = false;
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(34.))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .id("problems-details")
                                .occlude()
                                .w(px(DETAILS_WIDTH))
                                .flex()
                                .flex_col()
                                .gap_3()
                                .p_3()
                                .rounded(px(10.))
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.popover)
                                .shadow_lg()
                                .children(entries),
                        ),
                ),
        )
        .with_priority(PRIORITY)
        .into_any_element()
    }
}

fn icon(name: &str) -> Icon {
    let path = moonframe_core::IconName::new(name).map_or_else(String::new, |icon| icon.path());
    Icon::default().path(path)
}

/// The message without the "provider failed: " every provider's starts
/// with: the heading already says so.
fn strip_prefix(message: &str) -> String {
    message
        .strip_prefix("provider failed: ")
        .unwrap_or(message)
        .to_owned()
}
