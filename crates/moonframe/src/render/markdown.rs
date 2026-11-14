//! `ui.markdown`: a document the host renders with gpui-component's
//! `TextView`. The frame takes the `markdown` rule; code, quotes and alerts
//! take `markdown-code`, `markdown-quote` and `markdown-alert`. A link click
//! goes to the mod as its `link` message and never opens anything, a
//! `#heading` link scrolls to its heading, and pictures show only with
//! `images`, from files a `ui.image` could show.

mod blocks;
mod document;
#[cfg(feature = "math")]
mod math;
mod outline;
mod parts;
mod scrolling;
mod search;

use {
    super::{
        Cx, Draw, Painter, Place,
        styling::{LookExt as _, StyleExt as _},
    },
    crate::{app::MoonframeApp, color::ThemeColor as _},
    document::{Colors, Document},
    gpui_kit::{
        component::{ActiveTheme as _, Theme},
        *,
    },
    moonframe_core::{Markdown, Part, Style},
    std::path::{Path, PathBuf},
};

/// The find colors when the stylesheet sets none: the warning color, light
/// enough to read the text through, and stronger for the current match.
const FIND_ALPHA: f32 = 0.3;
const CURRENT_ALPHA: f32 = 0.6;

impl<'w> Draw<'w> for Markdown {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Markdown, &self.look, false, place);
        let base = |painter: &Painter<'w, '_>, part| {
            painter.look(part, &Default::default(), false, place).base
        };
        let code = base(painter, Part::MarkdownCode);
        let quote = base(painter, Part::MarkdownQuote);
        let alert = base(painter, Part::MarkdownAlert);
        let find = base(painter, Part::MarkdownFind);
        let current = base(painter, Part::MarkdownFindCurrent);
        let theme = cx.theme();
        let folder = painter
            .folder(place.owner)
            .unwrap_or(Path::new("."))
            .to_owned();
        let roots: Vec<PathBuf> = if self.images {
            painter
                .workspace
                .roots()
                .layers()
                .map(Path::to_owned)
                .chain([crate::paths::images()])
                .collect()
        } else {
            Vec::new()
        };
        let document = Document {
            id: place.part("text").id(),
            owner: place.owner.to_owned(),
            app: cx.entity().downgrade(),
            markdown: self.clone(),
            folder,
            roots,
            code: refinement(&code, theme),
            inline_code: inline(&code, theme),
            quote: refinement(&quote, theme),
            alert,
            colors: Colors {
                find: find
                    .background
                    .map_or(theme.warning.opacity(FIND_ALPHA), |color| color.hsla(theme)),
                current: current
                    .background
                    .map_or(theme.warning.opacity(CURRENT_ALPHA), |color| {
                        color.hsla(theme)
                    }),
            },
        };
        let mut frame = div()
            .id(place.id())
            .debug_selector(|| "markdown".to_owned())
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
        frame.child(document).into_any_element()
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

/// Sends `msg` from the mod `owner`, once the frame being drawn is done.
fn send(
    app: &WeakEntity<MoonframeApp>,
    owner: &str,
    to: Option<String>,
    msg: moonframe_core::Data,
    window: &mut Window,
    cx: &mut App,
) {
    let app = app.clone();
    let owner = owner.to_owned();
    window.defer(cx, move |window, cx| {
        let action = moonframe_core::Action::Send { to, msg };
        app.update(cx, |this, cx| this.dispatch(&owner, &action, window, cx))
            .ok();
    });
}
