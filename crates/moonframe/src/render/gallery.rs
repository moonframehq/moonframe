//! The component browser's content, for its own window (`crate::gallery`):
//! every component by category, a live preview of the chosen one drawn from
//! its `example`, and the Luau that uses it.

use {
    super::{Cx, Draw, Painter, Place, problem},
    gpui_kit::{
        component::{
            ActiveTheme as _, Icon, Sizable as _,
            button::{Button, ButtonVariants as _},
            h_resizable, resizable_panel,
        },
        prelude::FluentBuilder as _,
        *,
    },
    moonframe_core::Component,
    std::collections::BTreeMap,
};

/// The list's width to start with, and how far it can be dragged.
const LIST_WIDTH: f32 = 220.;
const LIST_MIN: f32 = 160.;
const LIST_MAX: f32 = 420.;
/// What a component without an `icon` shows.
const DEFAULT_ICON: &str = "square";

impl<'w> Painter<'w, '_> {
    /// The browser, with `chosen` (a component's name) shown, or the first.
    pub(crate) fn gallery(&mut self, chosen: &str, cx: Cx) -> AnyElement {
        let components = self.workspace.components();
        let theme = cx.theme();
        let current = components
            .iter()
            .find(|component| component.name == chosen)
            .or(components.first());
        let mut categories: BTreeMap<&str, Vec<&Component>> = BTreeMap::new();
        for component in components {
            categories
                .entry(&component.category)
                .or_default()
                .push(component);
        }
        let list = div()
            .id("gallery-list")
            .size_full()
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col()
            .gap_0p5()
            .bg(theme.sidebar)
            .children(categories.into_iter().flat_map(|(category, members)| {
                let header = div()
                    .px_2()
                    .pt_3()
                    .pb_1()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .child(category.to_owned())
                    .into_any_element();
                let items = members.into_iter().map(|component| {
                    let selected = current.is_some_and(|current| current.name == component.name);
                    let name = component.name.clone();
                    let icon = component
                        .icon
                        .as_ref()
                        .map_or_else(|| format!("icons/{DEFAULT_ICON}.svg"), |icon| icon.path());
                    div()
                        .id(SharedString::from(format!(
                            "gallery-item-{}",
                            component.name
                        )))
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded(px(6.))
                        .text_sm()
                        .cursor_pointer()
                        .when(selected, |item| {
                            item.bg(theme.primary.opacity(0.15))
                                .text_color(theme.primary)
                        })
                        .when(!selected, |item| {
                            item.hover(|item| item.bg(theme.list_hover))
                        })
                        .child(Icon::default().path(icon).small().text_color(if selected {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        }))
                        .child(component.title.clone())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.show_component(&name, cx);
                        }))
                        .into_any_element()
                });
                std::iter::once(header).chain(items)
            }));
        let detail = match current {
            Some(component) => self.component_detail(component, cx),
            None => div()
                .p_6()
                .text_color(theme.muted_foreground)
                .child("No components yet. Add components/<name>.luau to the app or to ~/.config/moonframe.")
                .into_any_element(),
        };
        h_resizable("gallery-split")
            .child(
                resizable_panel()
                    .size(px(LIST_WIDTH))
                    .size_range(px(LIST_MIN)..px(LIST_MAX))
                    .child(list),
            )
            .child(resizable_panel().child(detail))
            .into_any_element()
    }

    fn component_detail(&mut self, component: &'w Component, cx: Cx) -> AnyElement {
        let preview = match &component.preview {
            Ok(node) => node.draw(self, &Place::root(&component.name), cx),
            Err(error) => problem(format!("{}: {error}", component.path_name()), cx),
        };
        let theme = cx.theme();
        let name = component.name.clone();
        let usage = component.usage.clone();
        div()
            .id("gallery-detail")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(component.title.clone()),
                    )
                    .child(
                        Button::new("gallery-close")
                            .small()
                            .ghost()
                            .label("Close")
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(component.description.clone()),
            )
            .child(section("Preview", theme))
            .child(
                div()
                    .relative()
                    .p_4()
                    .min_h(px(96.))
                    .flex()
                    .flex_col()
                    .rounded(px(8.))
                    .border_1()
                    .border_dashed()
                    .border_color(theme.border)
                    .child(preview)
                    // A preview shows; its controls send to no mod.
                    .child(div().absolute().inset_0().occlude()),
            )
            .child(section("Use it in a mod", theme))
            .child(
                div()
                    .p_3()
                    .rounded(px(8.))
                    .bg(theme.muted)
                    .font_family("Menlo")
                    .text_xs()
                    .whitespace_normal()
                    .child(component.usage.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(component.path.display().to_string()),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("gallery-copy-code")
                            .small()
                            .outline()
                            .label("Copy code")
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.copy(usage.clone(), cx)),
                            ),
                    )
                    .child(
                        Button::new("gallery-copy-request")
                            .small()
                            .primary()
                            .label("Copy for agent")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.copy_component_request(&name, cx);
                                window.remove_window();
                            })),
                    ),
            )
            .into_any_element()
    }
}

fn section(title: &str, theme: &component::Theme) -> AnyElement {
    div()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.muted_foreground)
        .child(title.to_owned())
        .into_any_element()
}
