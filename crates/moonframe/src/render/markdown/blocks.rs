//! What a document holds beyond the text view's own blocks: GitHub alerts,
//! pictures on a line of their own, footnote marks, source chips, the
//! streaming cursor, math, and the corner of a code block. They draw inside
//! the text view, after the frame that built them, so all they know of the
//! document comes through [`Shared`].

use {
    super::{
        document::{SharedRef, go_to},
        outline::Outline,
        parts::{self, AlertKind},
        send,
    },
    gpui_kit::{
        base::{StyledExt as _, text::CodeBlock},
        component::{
            ActiveTheme as _, Icon as GpuiIcon,
            text::{
                InlineElement, InlineRenderContext, MarkdownExtensions, MarkdownNode,
                MarkdownParseContext, MarkdownPlugin, TextView, TextViewState, markdown_ast::Node,
            },
        },
        *,
    },
    moonframe_core::{Data, Markdown},
    std::{path::PathBuf, sync::Arc},
};

const ALERT: &str = "moonframe-alert";
const PICTURE: &str = "moonframe-picture";
const FOOTNOTE: &str = "moonframe-footnote";
const CITE: &str = "moonframe-cite";

/// A link's address that draws a chip, and the streaming cursor's.
const CITE_SCHEME: &str = "cite:";
const CURSOR_SCHEME: &str = "cursor:";

/// The mark of a source chip, as `.chat-markdown-cite-mark` had it.
const CITE_MARK: u32 = 0xe5488f;

/// An alert's markdown, with the kind that colors it.
struct Alert {
    kind: AlertKind,
    body: SharedString,
}

/// A picture on a line of its own.
struct Picture {
    src: String,
    alt: String,
}

/// A `[label](cite:source)` link.
struct Cite {
    label: SharedString,
    source: SharedString,
}

pub(super) fn extensions(shared: &SharedRef) -> MarkdownExtensions {
    let alerts = shared.clone();
    let pictures = shared.clone();
    let extensions = MarkdownExtensions::default()
        .frontmatter()
        .block_parser(parse_alert)
        .block_renderer(ALERT, move |node, window, cx| {
            draw_alert(node, &alerts, window, cx)
        })
        .block_parser(parse_picture)
        .block_renderer(PICTURE, move |node, window, cx| {
            draw_picture(node, &pictures, window, cx)
        })
        .plugin(Footnotes(shared.clone()))
        .plugin(Chips);
    #[cfg(feature = "math")]
    let extensions = super::math::with(extensions);
    extensions
}

fn parse_alert(node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
    let Node::Blockquote(_) = node else {
        return None;
    };
    let source = cx.node_source(node)?;
    let (kind, body) = parts::alert(source)?;
    Some(
        MarkdownNode::new(ALERT, Alert {
            kind,
            body: body.clone().into(),
        })
        .text(body)
        .markdown(source.to_owned()),
    )
}

fn draw_alert(node: &MarkdownNode, shared: &SharedRef, _: &mut Window, cx: &mut App) -> AnyElement {
    let Some(alert) = node.data::<Alert>() else {
        return div().into_any_element();
    };
    let theme = cx.theme();
    let (color, icon) = match alert.kind {
        AlertKind::Note => (theme.info, "icons/info.svg"),
        AlertKind::Tip => (theme.success, "icons/sparkles.svg"),
        AlertKind::Important => (theme.primary, "icons/message-square.svg"),
        AlertKind::Warning => (theme.warning, "icons/triangle-alert.svg"),
        AlertKind::Caution => (theme.danger, "icons/octagon-alert.svg"),
    };
    let look = shared.lock().map(|shared| shared.alert.clone()).ok();
    let mut frame = div().flex().flex_col().gap_1().w_full().border_color(color);
    if let Some(look) = &look {
        let refined = super::refinement(look, theme);
        frame = frame.refine_style(&refined).border_color(color);
    }
    let id = node.source_range().map_or(0, |range| range.start);
    let body = guarded(
        TextView::markdown(("markdown-alert", id), alert.body.clone()),
        shared,
    );
    frame
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_color(color)
                .font_weight(FontWeight::SEMIBOLD)
                .child(GpuiIcon::default().path(icon))
                .child(alert.kind.title()),
        )
        .child(body.w_full())
        .into_any_element()
}

/// A view inside the document, kept to the document's rules: its links go
/// to the mod and its pictures load nothing the mod couldn't show.
fn guarded(view: TextView, shared: &SharedRef) -> TextView {
    view.selectable(true)
        .image_source(image_source(shared))
        .on_link_click(link_click(shared))
}

fn parse_picture(node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
    let Node::Paragraph(paragraph) = node else {
        return None;
    };
    let [Node::Image(image)] = paragraph.children.as_slice() else {
        return None;
    };
    Some(
        MarkdownNode::new(PICTURE, Picture {
            src: image.url.clone(),
            alt: image.alt.clone(),
        })
        .text(image.alt.clone())
        .markdown(cx.node_source(node)?.to_owned()),
    )
}

fn draw_picture(
    node: &MarkdownNode,
    shared: &SharedRef,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Some(picture) = node.data::<Picture>() else {
        return div().into_any_element();
    };
    let id = node.source_range().map_or(0, |range| range.start);
    let Some(path) = resolved(&picture.src, shared) else {
        // Not shown: say what it was, rather than leave a gap.
        let muted = cx.theme().muted_foreground;
        let label = if picture.alt.is_empty() {
            picture.src.clone()
        } else {
            picture.alt.clone()
        };
        return div()
            .flex()
            .items_center()
            .gap_2()
            .text_color(muted)
            .child(GpuiIcon::default().path("icons/image.svg"))
            .child(label)
            .into_any_element();
    };
    let click = shared.clone();
    let src = picture.src.clone();
    let alt = picture.alt.clone();
    let clickable = shared
        .lock()
        .ok()
        .is_some_and(|shared| shared.image.is_some());
    let element = img(path).max_w_full();
    if !clickable {
        return element.into_any_element();
    }
    div()
        .id(("markdown-picture", id))
        .debug_selector(|| "markdown-picture".to_owned())
        .cursor_pointer()
        .child(element)
        .on_click(move |_, window, cx| {
            let Ok(shared) = click.lock() else {
                return;
            };
            if let Some(msg) = &shared.image {
                let msg = Markdown::note(msg, [
                    ("src", Data::String(src.clone())),
                    ("alt", Data::String(alt.clone())),
                ]);
                send(
                    &shared.app,
                    &shared.owner,
                    shared.to.clone(),
                    msg,
                    window,
                    cx,
                );
            }
        })
        .into_any_element()
}

/// The file a picture's address names, if the document may show it. Web
/// addresses never resolve: rendering must not reach the network.
fn resolved(src: &str, shared: &SharedRef) -> Option<PathBuf> {
    let shared = shared.lock().ok()?;
    if !shared.images || src.contains("://") || src.starts_with("data:") {
        return None;
    }
    let roots = shared.roots.clone();
    crate::render::images::resolve_src(src, &shared.owner, &shared.folder, || roots)
}

/// Where the view's own inline pictures come from: an allowed file, or an
/// empty path, which loads nothing.
pub(super) fn image_source(
    shared: &SharedRef,
) -> impl Fn(&SharedUri) -> ImageSource + Send + Sync + 'static {
    let shared = shared.clone();
    move |uri| ImageSource::from(resolved(uri.as_ref(), &shared).unwrap_or_default())
}

/// A click on a link: a `#heading` scrolls there, the rest goes to the mod.
pub(super) fn link_click(
    shared: &SharedRef,
) -> impl Fn(&SharedString, &ClickEvent, &mut Window, &mut App) + Send + Sync + 'static {
    let shared = shared.clone();
    move |url, _, window, cx| {
        if let Some(fragment) = url.strip_prefix('#')
            && jump(&shared, cx, |outline| {
                outline.anchor(fragment).map(|heading| heading.at)
            })
        {
            return;
        }
        let Ok(shared) = shared.lock() else {
            return;
        };
        if let Some(link) = &shared.link {
            let msg = Markdown::message(link, url);
            send(
                &shared.app,
                &shared.owner,
                shared.to.clone(),
                msg,
                window,
                cx,
            );
        }
    }
}

/// Scrolls to the place `find` picks from the document's outline, read
/// here: a click is rare, and a streaming view shouldn't pay for it.
fn jump(shared: &SharedRef, cx: &mut App, find: impl Fn(&Outline) -> Option<usize>) -> bool {
    let Some(state) = view_state(shared) else {
        return false;
    };
    let rendered = state.read(cx).rendered_text();
    let outline = Outline::parse(rendered.source());
    let Some(at) = find(&outline) else {
        return false;
    };
    let scroll = shared.lock().is_ok_and(|shared| shared.scroll);
    go_to(&state, &rendered, &outline, at, scroll, cx);
    true
}

fn view_state(shared: &SharedRef) -> Option<Entity<TextViewState>> {
    shared.lock().ok()?.state.as_ref()?.upgrade()
}

/// The corner of a code block: its language and a copy button.
pub(super) fn code_actions(block: &CodeBlock, _: &mut Window, cx: &mut App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let foreground = cx.theme().foreground;
    let code = block.code();
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .text_xs()
        .text_color(muted)
        .children(block.lang())
        .child(
            div()
                .id("copy")
                .debug_selector(|| "markdown-copy".to_owned())
                .cursor_pointer()
                .hover(move |this| this.text_color(foreground))
                .child(GpuiIcon::default().path("icons/copy.svg"))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
                }),
        )
        .into_any_element()
}

/// `[^1]` as a mark that scrolls to its footnote.
struct Footnotes(SharedRef);

impl MarkdownPlugin for Footnotes {
    fn name(&self) -> &str {
        FOOTNOTE
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::FootnoteReference(reference) = node else {
            return None;
        };
        let label = reference
            .label
            .clone()
            .unwrap_or_else(|| reference.identifier.clone());
        Some(
            MarkdownNode::new(FOOTNOTE, reference.identifier.clone())
                .text(format!("[{label}]"))
                .markdown(format!("[^{label}]")),
        )
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let identifier = node.data::<String>()?.clone();
        let shared = self.0.clone();
        let id = node.source_range().map_or(0, |range| range.start);
        let accent = cx.theme().primary;
        let size = context.font_size() * 0.75;
        Some(InlineElement::new(
            div()
                .id(("markdown-footnote", id))
                .debug_selector(|| "markdown-footnote".to_owned())
                .cursor_pointer()
                .text_size(size)
                .text_color(accent)
                .child(SharedString::from(node.as_text().to_owned()))
                .on_click(move |_, _, cx| {
                    jump(&shared, cx, |outline| {
                        outline.footnotes.get(&identifier).copied()
                    });
                }),
        ))
    }
}

/// `[label](cite:source)` as a source chip, and `[](cursor:)` as the dot
/// that follows text still being written.
struct Chips;

impl MarkdownPlugin for Chips {
    fn name(&self) -> &str {
        CITE
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Link(link) = node else {
            return None;
        };
        if link.url.starts_with(CURSOR_SCHEME) {
            return Some(MarkdownNode::new(CITE, None::<Arc<Cite>>).text("\u{25cf}"));
        }
        let source = link.url.strip_prefix(CITE_SCHEME)?;
        let label = node.to_string();
        Some(
            MarkdownNode::new(
                CITE,
                Some(Arc::new(Cite {
                    label: label.clone().into(),
                    source: source.to_owned().into(),
                })),
            )
            .text(label),
        )
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let theme = cx.theme();
        let chip = node.data::<Option<Arc<Cite>>>()?;
        let Some(cite) = chip else {
            let size = context.font_size() * 0.6;
            return Some(InlineElement::new(
                div()
                    .size(size)
                    .rounded_full()
                    .bg(theme.foreground)
                    .ml(px(2.)),
            ));
        };
        let letter = cite
            .label
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_default();
        let source = cite.source.clone();
        let _ = window;
        let id = node.source_range().map_or(0, |range| range.start);
        Some(InlineElement::new(
            div()
                .id(("markdown-cite", id))
                .flex()
                .items_center()
                .gap(px(5.))
                .pl(px(3.))
                .pr(px(7.))
                .py(px(1.))
                .rounded(px(9.))
                .bg(theme.secondary)
                .text_size(context.font_size() * 0.85)
                .text_color(theme.muted_foreground)
                .tooltip(move |window, cx| {
                    component::tooltip::Tooltip::new(source.clone()).build(window, cx)
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(14.))
                        .rounded(px(7.))
                        .bg(rgb(CITE_MARK))
                        .text_color(white())
                        .font_weight(FontWeight::BOLD)
                        .child(letter),
                )
                .child(cite.label.clone()),
        ))
    }
}
