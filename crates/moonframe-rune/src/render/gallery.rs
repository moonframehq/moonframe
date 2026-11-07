//! Every component with its live demo, as the Components window shows them
//! on GPUI: what this backend can draw, to look through.

use {
    super::{Painter, Place},
    crate::Ui,
    moonframe_core::{Component, Direction, Token},
    rune::{El, ScrollSpec, col, el, row, text},
    std::collections::BTreeMap,
};

/// The list's width.
const LIST: f32 = 240.;
/// What the window's buttons take at the top of a window without a title bar.
const TITLE_BAR: f32 = 28.;

impl<'w> Painter<'w> {
    /// The list of components beside the chosen one's demo.
    pub(super) fn gallery(&self, place: &Place<'w>) -> El {
        let components = self.ui.workspace.components();
        let chosen = self
            .ui
            .preview
            .as_deref()
            .or(components.first().map(|component| component.name.as_str()));
        let mut categories: BTreeMap<&str, Vec<&Component>> = BTreeMap::new();
        for component in components {
            categories
                .entry(&component.category)
                .or_default()
                .push(component);
        }
        let muted = self.palette.token(Token::MutedForeground);
        let mut list = col()
            .key("gallery-list")
            .w(LIST)
            .min_h(0.)
            .gap(2.)
            .padding(8.);
        // Clear of macOS's window buttons, and something to drag it by.
        if cfg!(target_os = "macos") {
            list = list.child(el().h(TITLE_BAR).w_full().window_drag());
        }
        for (category, members) in categories {
            list = list.child(
                text(category)
                    .font(place.ink.text_style())
                    .color(muted)
                    .pt(8.),
            );
            for component in members {
                let name = component.name.clone();
                let on = chosen == Some(component.name.as_str());
                let entry = row()
                    .key(format!("gallery-{name}"))
                    .px(8.)
                    .py(4.)
                    .radius(6.)
                    .child(
                        text(component.title.clone())
                            .font(place.ink.text_style())
                            .color(place.ink.color)
                            .truncate(),
                    )
                    .on_press(move |ui: &mut Ui| ui.preview = Some(name.clone()));
                list = list.child(if on {
                    entry.fill(self.palette.token(Token::ListActive))
                } else {
                    entry.hover(self.palette.token(Token::ListHover))
                });
            }
        }
        let preview = chosen
            .and_then(|name| self.ui.workspace.demo(name))
            .map(|(owner, slot)| {
                let inner = Place {
                    parent: Direction::Column,
                    ..place.clone()
                };
                self.mounted(owner, Some(slot), &inner)
            });
        let stage = col()
            .key("gallery-stage")
            .grow(1.)
            .min_w(0.)
            .min_h(0.)
            .padding(24.)
            .gap(8.)
            // Not a scroll view: it would give a demo's `grow` no height to fill.
            .clip_rounded(0.)
            .children(preview)
            .children(self.stage.take());
        row()
            .key("gallery")
            .w_full()
            .h_full()
            .child(
                list.fill(self.palette.token(Token::Sidebar))
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    }),
            )
            .child(stage)
    }
}
