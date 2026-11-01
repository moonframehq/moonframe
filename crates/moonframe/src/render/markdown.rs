//! `ui.markdown`: a document the host renders with gpui-component's
//! `TextView`. The frame takes the `markdown` rule; code and quotes take
//! `markdown-code` and `markdown-quote`. A link click goes to the mod as its
//! `link` message and never opens anything; pictures are never loaded.

use {
    super::{
        Cx, Draw, Painter, Place,
        styling::{LookExt as _, StyleExt as _},
    },
    crate::color::ThemeColor as _,
    gpui_kit::{
        component::{
            ActiveTheme as _, Theme,
            text::{TextView, TextViewStyle},
        },
        *,
    },
    moonframe_core::{Action, Markdown, Part, Style},
};

impl<'w> Draw<'w> for Markdown {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Markdown, &self.look, false, place);
        let code = painter
            .look(Part::MarkdownCode, &Default::default(), false, place)
            .base;
        let quote = painter
            .look(Part::MarkdownQuote, &Default::default(), false, place)
            .base;
        let theme = cx.theme();
        let mut view = TextView::markdown(place.part("text").id(), self.text.clone())
            .selectable(true)
            .scrollable(self.scroll)
            // A document may name any address as a picture: none is loaded,
            // so rendering one never reaches the network or the disk.
            .image_source(|_| ImageSource::from(std::path::PathBuf::new()))
            .style(TextViewStyle {
                code_block: refinement(&code, theme),
                ..TextViewStyle::default()
            })
            .inline_code_style(inline(&code, theme))
            .blockquote_style(refinement(&quote, theme));
        if let Some(link) = &self.link {
            let owner = place.owner.to_owned();
            let to = self.to.clone();
            let link = link.clone();
            let app = cx.entity().downgrade();
            view = view.on_link_click(move |url, _, window, cx| {
                let action = Action::Send {
                    to: to.clone(),
                    msg: Markdown::message(&link, url),
                };
                app.update(cx, |this, cx| this.dispatch(&owner, &action, window, cx))
                    .ok();
            });
        } else {
            // Without a `link`, a click on one does nothing: the default
            // would open it in the system browser.
            view = view.on_link_click(|_, _, _, _| {});
        }
        let mut frame = div()
            .id(place.id())
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        if self.grow {
            frame = frame.flex_1();
        }
        if let Some(width) = self.width.or(look.base.width) {
            frame = frame.w(px(width));
        }
        if let Some(height) = self.height.or(look.base.height) {
            frame = frame.h(px(height));
        }
        if let Some(width) = self.max_width {
            frame = frame.max_w(px(width));
        }
        let view = if self.scroll {
            view.size_full().into_any_element()
        } else {
            view.w_full().into_any_element()
        };
        frame.child(view).into_any_element()
    }
}

/// A box's look as a style the text view folds over its own: background,
/// text color, border, corners, padding and font.
fn refinement(style: &Style, theme: &Theme) -> StyleRefinement {
    let mut refined = StyleRefinement::default()
        .typography(style, theme)
        .edges(style.border, style, theme)
        .padded(&style.padding);
    if let Some(color) = style.background {
        refined = refined.bg(color.hsla(theme));
    }
    if let Some(radius) = style.radius {
        refined = refined.cornered(radius);
    }
    refined
}

/// Inline code takes the same, less the padding a line can't hold.
fn inline(style: &Style, theme: &Theme) -> StyleRefinement {
    let mut refined = StyleRefinement::default().typography(style, theme);
    if let Some(color) = style.background {
        refined = refined.bg(color.hsla(theme));
    }
    if let Some(radius) = style.radius {
        refined = refined.cornered(radius);
    }
    refined
}
