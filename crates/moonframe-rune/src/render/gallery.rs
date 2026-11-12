//! The component browser, as the Components window shows it on GPUI: every
//! component by category, a preview of the chosen one from its demo or its
//! `example`, and the Luau that uses it.

use {
    super::{Painter, Place},
    crate::{Ui, icons, style::Ink},
    moonframe_core::{Component, Direction, IconName, Token},
    rune::{Color, El, ScrollSpec, col, edit_access, el, row, text, text_input},
    std::collections::BTreeMap,
};

/// The list's width.
const LIST: f32 = 240.;
/// What the window's buttons take at the top of a window without a title bar.
const TITLE_BAR: f32 = 28.;
/// The search field's entry among the fields.
const SEARCH: &str = "gallery:search";
/// The least a preview takes, and one with a dialog, which floats in it.
const PREVIEW: f32 = 96.;
const PREVIEW_WITH_DIALOG: f32 = 560.;
/// A list entry's icon.
const ENTRY_ICON: f32 = 14.;
/// What a component without an `icon` shows.
const DEFAULT_ICON: &str = "square";

/// Whether `component` matches a search: every word of `query`, in any case,
/// found in its name, title, category, description or `when`, as on GPUI.
fn matches(component: &Component, query: &str) -> bool {
    let text = format!(
        "{} {} {} {} {}",
        component.name, component.title, component.category, component.description, component.when
    )
    .to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| text.contains(word))
}

fn icon(name: Option<&IconName>, size: f32, color: Color) -> El {
    let fallback = IconName::new(DEFAULT_ICON);
    match name.or(fallback.as_ref()).and_then(icons::icon) {
        Some(found) => rune::icon(found, size).color(color),
        None => el().size(size, size),
    }
}

impl<'w> Painter<'w> {
    /// The list of components beside the chosen one's page.
    pub(super) fn gallery(&self, place: &Place<'w>) -> El {
        let query = self
            .ui
            .fields
            .borrow()
            .get(SEARCH)
            .map(|field| field.edit.text.clone())
            .unwrap_or_default();
        let components: Vec<&'w Component> = self
            .ws
            .components()
            .iter()
            .filter(|component| matches(component, &query))
            .collect();
        // The chosen one while it matches, else the first that does.
        let chosen = components
            .iter()
            .find(|component| self.ui.preview.as_deref() == Some(component.name.as_str()))
            .or(components.first())
            .copied();
        if self.ui.preview_only
            && let Some(component) = chosen
        {
            return self.preview_alone(component, place);
        }
        let page = chosen.map_or_else(
            || self.nothing_chosen(&query, place),
            |component| self.page(component, place),
        );
        row()
            .key("gallery")
            .w_full()
            .h_full()
            .child(self.gallery_list(&components, chosen, place))
            .child(page)
    }

    fn gallery_list(
        &self,
        components: &[&'w Component],
        chosen: Option<&'w Component>,
        place: &Place<'w>,
    ) -> El {
        let mut categories: BTreeMap<&str, Vec<&Component>> = BTreeMap::new();
        for component in components {
            categories
                .entry(&component.category)
                .or_default()
                .push(component);
        }
        let mut list = col().key("gallery-list").min_h(0.).gap(2.).padding(8.);
        for (category, members) in categories {
            list = list.child(
                text(category)
                    .font(self.small(place, 500.).text_style())
                    .color(self.palette.token(Token::MutedForeground))
                    .px(8.)
                    .pt(10.)
                    .pb(4.),
            );
            for component in members {
                let on = chosen.is_some_and(|chosen| chosen.name == component.name);
                list = list.child(self.entry(component, on, place));
            }
        }
        let mut side = col()
            .key("gallery-side")
            .w(LIST)
            .h_full()
            .min_h(0.)
            .fill(self.palette.token(Token::Sidebar));
        // Clear of macOS's window buttons, and something to drag it by.
        if cfg!(target_os = "macos") {
            side = side.child(el().h(TITLE_BAR).w_full().window_drag());
        }
        side.child(self.search(place))
            .child(list.grow(1.).scroll(ScrollSpec {
                stick_bottom: false,
            }))
    }

    fn search(&self, place: &Place<'w>) -> El {
        let mut fields = self.ui.fields.borrow_mut();
        let field = fields.entry(SEARCH.to_owned()).or_default();
        let shown = text_input(&field.edit, place.ink.text_style());
        drop(fields);
        let muted = self.palette.token(Token::MutedForeground);
        row()
            .key("gallery-search-box")
            .mx(12.)
            .mt(4.)
            .mb(4.)
            .px(10.)
            .h(32.)
            .gap(8.)
            .items_center()
            .radius(10.)
            .fill(self.palette.token(Token::Background))
            .child(icon(IconName::new("search").as_ref(), ENTRY_ICON, muted))
            .child(
                shown
                    .single_line()
                    .key("gallery-search")
                    .grow(1.)
                    .min_w(0.)
                    .placeholder("Search components", muted)
                    .color(place.ink.color)
                    .on_edit(edit_access(|ui: &mut Ui| {
                        &mut ui
                            .fields
                            .get_mut()
                            .entry(SEARCH.to_owned())
                            .or_default()
                            .edit
                    })),
            )
    }

    fn entry(&self, component: &Component, on: bool, place: &Place<'w>) -> El {
        let primary = self.palette.token(Token::Accent);
        let muted = self.palette.token(Token::MutedForeground);
        let name = component.name.clone();
        let entry = row()
            .key(format!("gallery-{name}"))
            .px(8.)
            .py(4.)
            .gap(8.)
            .items_center()
            .radius(6.)
            .child(icon(
                component.icon.as_ref(),
                ENTRY_ICON,
                if on {
                    primary
                } else {
                    muted
                },
            ))
            .child(
                text(component.title.clone())
                    .font(place.ink.text_style())
                    .color(if on {
                        primary
                    } else {
                        place.ink.color
                    })
                    .grow(1.)
                    .truncate(),
            )
            .on_press(move |ui: &mut Ui| ui.preview = Some(name.clone()));
        // Components that move get a mark, so they can be found to play with.
        let entry = if component.animated() {
            let mark = if on {
                primary
            } else {
                muted.alpha(0.7)
            };
            entry.child(
                icon(IconName::new("sparkles").as_ref(), 12., mark).help("Animated".to_owned()),
            )
        } else {
            entry
        };
        if on {
            entry.fill(primary.alpha(0.15))
        } else {
            entry.hover(self.palette.token(Token::ListHover))
        }
    }

    /// The component drawn from its demo, or its example; and whether it
    /// runs live. Without a demo its controls send nowhere.
    fn preview(&self, component: &'w Component, place: &Place<'w>) -> (El, bool) {
        let inner = Place {
            parent: Direction::Column,
            ..place.clone()
        };
        let demo = self.ws.demo(&component.name);
        match (&component.preview, demo) {
            (Err(error), _) => (
                self.marker(&format!("{}: {error}", component.path_name()), place),
                false,
            ),
            (_, Some((owner, slot))) if slot.view().is_some() || slot.error().is_some() => {
                (self.mounted(owner, Some(slot), &inner), true)
            },
            (Ok(node), _) => {
                let example = Place {
                    owner: &component.name,
                    ..inner
                };
                (self.node(node, &example), false)
            },
        }
    }

    /// For a screenshot: the preview alone, with the window's room for its
    /// overlays and popovers.
    fn preview_alone(&self, component: &'w Component, place: &Place<'w>) -> El {
        let (preview, _) = self.preview(component, place);
        col()
            .key("gallery-stage")
            .w_full()
            .h_full()
            .min_h(0.)
            .padding(16.)
            .child(preview)
            .children(self.stage.take())
    }

    fn page(&self, component: &'w Component, place: &Place<'w>) -> El {
        let (preview, live) = self.preview(component, place);
        let layers = self.stage.take();
        let least = if layers.is_empty() {
            PREVIEW
        } else {
            PREVIEW_WITH_DIALOG
        };
        let muted = self.palette.token(Token::MutedForeground);
        let stage = col()
            .key("gallery-stage")
            .w_full()
            .min_h(least)
            .padding(16.)
            .radius(8.)
            .border(1., self.palette.token(Token::Border))
            .border_dash(5., 4.)
            .clip_rounded(8.)
            .child(preview)
            .children(layers);
        // A static preview shows; its controls send to no mod.
        let stage = if live {
            stage
        } else {
            stage.child(
                el().key("gallery-static")
                    .pin(Some(0.), Some(0.), Some(0.), Some(0.))
                    .on_press(|_: &mut Ui| {}),
            )
        };
        let usage = component
            .demo
            .as_ref()
            .map_or_else(|| component.usage.clone(), |demo| demo.source.clone());
        let title = Ink {
            size: place.ink.size * 1.3,
            weight: 600.,
            ..place.ink
        };
        let code = Ink {
            mono: true,
            ..self.small(place, 400.)
        };
        col()
            .key("gallery-page")
            .grow(1.)
            .min_w(0.)
            .h_full()
            .min_h(0.)
            .child(
                col()
                    .key("gallery-detail")
                    .w_full()
                    .padding(20.)
                    .gap(12.)
                    .child(el().h(TITLE_BAR / 2.).w_full().window_drag())
                    .child(
                        text(component.title.clone())
                            .font(title.text_style())
                            .color(place.ink.color),
                    )
                    .child(
                        text(component.description.clone())
                            .font(place.ink.text_style())
                            .color(muted)
                            .wrap(),
                    )
                    .child(self.section("Preview", place))
                    .child(stage)
                    .child(self.section("Use it in a mod", place))
                    .child(
                        el().w_full()
                            .padding(12.)
                            .radius(8.)
                            .fill(self.palette.token(Token::Muted))
                            .child(
                                text(usage)
                                    .font(code.text_style())
                                    .color(place.ink.color)
                                    .wrap(),
                            ),
                    )
                    .child(
                        text(component.path.display().to_string())
                            .font(self.small(place, 400.).text_style())
                            .color(muted)
                            .truncate_head(),
                    )
                    .grow(1.)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    }),
            )
    }

    fn nothing_chosen(&self, query: &str, place: &Place<'w>) -> El {
        let said = if query.is_empty() {
            "No components yet. Add components/<name>.luau to the app.".to_owned()
        } else {
            format!("No components match “{query}”.")
        };
        col().key("gallery-page").grow(1.).padding(24.).child(
            text(said)
                .font(place.ink.text_style())
                .color(self.palette.token(Token::MutedForeground)),
        )
    }

    fn section(&self, title: &str, place: &Place<'w>) -> El {
        text(title.to_owned())
            .font(self.small(place, 500.).text_style())
            .color(self.palette.token(Token::MutedForeground))
    }

    /// Ink a size down, for captions.
    fn small(&self, place: &Place<'w>, weight: f32) -> Ink {
        Ink {
            size: place.ink.size * 0.85,
            weight,
            ..place.ink
        }
    }
}
