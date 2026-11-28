//! ` ```mermaid `, ` ```dot ` and ` ```graphviz ` fences drawn as diagrams
//! in every `ui.markdown` (chat answers, docs, notes), laid out by
//! moonframe-diagram in the theme's colors and font, with "Copy as
//! Mermaid" and "Copy source" in the corner.
//!
//! While an answer streams in, the fence grows: each version that parses
//! is laid out from the one before (so what was there stays put), and one
//! that does not yet (a line half written) shows the longest earlier
//! version that did, so it never flickers back to source. Once the fence
//! is closed its text is final: text that does not parse shows as its
//! source, with why.

mod cache;

use {
    cache::{Cache, Kind, Laid},
    gpui_kit::{
        component::{
            ActiveTheme as _,
            text::{MarkdownExtensions, MarkdownNode, markdown_ast::Node},
        },
        *,
    },
    moonframe_diagram::{
        Diagram, Options,
        layout::{self, layout},
        parse, view, write,
    },
    std::{cell::RefCell, sync::Arc},
};

const BLOCK: &str = "moonframe-diagram";
/// The tallest a diagram stands in a document; wider ones scale down.
const MOST_HEIGHT: f32 = 480.0;
/// The most nodes a fence draws: it is laid out as the document draws,
/// so a bigger one would hold the window up. Agents' diagrams are far
/// smaller.
const MOST_INLINE: usize = 300;
/// The most edges, messages and notes a fence draws, for the same reason.
const MOST_INLINE_LINES: usize = 600;
/// The most items its layout may place, stand-ins for long edges
/// included ([`moonframe_diagram::layout::cost`]): a few edges across
/// many ranks cost as much as a big diagram.
const MOST_INLINE_COST: usize = 3_000;

/// A fence: its language and text, and whether it is still open (its
/// closing line not yet written, as while an answer streams in).
#[derive(Clone)]
struct Fence {
    language: String,
    source: String,
    open: bool,
}

thread_local! {
    /// Each fence's text laid out (or why not).
    static LAID: RefCell<Cache> = RefCell::new(Cache::default());
}

pub(super) fn with(extensions: MarkdownExtensions) -> MarkdownExtensions {
    extensions
        .block_parser(|node, cx| {
            let Node::Code(code) = node else {
                return None;
            };
            let language = code.lang.as_deref()?.to_ascii_lowercase();
            if !matches!(language.as_str(), "mermaid" | "dot" | "graphviz" | "gv") {
                return None;
            }
            let markdown = cx.node_source(node)?.to_owned();
            let fence = Fence {
                language,
                source: code.value.clone(),
                open: !closed(&markdown),
            };
            Some(
                MarkdownNode::new(BLOCK, fence)
                    .text(code.value.clone())
                    .markdown(markdown),
            )
        })
        .block_renderer(BLOCK, |node, window, cx| {
            let Some(fence) = node.data::<Fence>() else {
                return div().into_any_element();
            };
            draw(fence, window, cx)
        })
}

/// Whether a fence's markdown ends in its closing line: a run of the
/// opening's backticks or tildes, at least as long, and nothing else.
fn closed(markdown: &str) -> bool {
    let mut lines = markdown.trim_end().lines();
    let (Some(first), Some(last)) = (lines.next(), lines.next_back()) else {
        return false;
    };
    let first = first.trim_start();
    let Some(mark) = first.chars().next().filter(|c| matches!(c, '`' | '~')) else {
        return false;
    };
    let opening = first.chars().take_while(|&c| c == mark).count();
    let last = last.trim();
    last.len() >= opening && last.chars().all(|c| c == mark)
}

/// The fence laid out: this text, else while the fence is open the
/// longest earlier text of it that was, else why not.
fn laid(fence: &Fence, window: &mut Window, cx: &App) -> Laid {
    let kind: Kind = (cx.theme().font_family.clone(), fence.language.clone());
    if let Some(known) = LAID.with_borrow_mut(|laid| laid.get(&kind, &fence.source)) {
        return known;
    }
    let before = LAID.with_borrow_mut(|laid| laid.earlier(&kind, &fence.source));
    let parsed = parse::fenced(&fence.language, &fence.source).unwrap_or_else(|| {
        Err(parse::Unsupported {
            line: 0,
            reason: format!("no parser for `{}`", fence.language),
        })
    });
    let result: Laid = match parsed {
        // Laid out as the document draws: bigger diagrams show as source.
        Ok(diagram) if too_big(&diagram) => Err(format!(
            "{} nodes and {} lines, more than a document draws ({MOST_INLINE} and {MOST_INLINE_LINES})",
            diagram.nodes.len() + diagram.groups.len(),
            diagram.edges.len() + diagram.messages.len() + diagram.notes.len(),
        )),
        Ok(diagram) if layout::cost(&diagram) > MOST_INLINE_COST => Err(format!(
            "its edges cross too many ranks: {} items to place, more than a document draws ({MOST_INLINE_COST})",
            layout::cost(&diagram),
        )),
        Ok(diagram) => {
            let options = Options::default();
            let widths = view::measure(&diagram, options.font_size, window, cx);
            let previous = before
                .as_ref()
                .and_then(|b| b.as_ref().ok())
                .map(|(_, l)| l.as_ref());
            let out = layout(&diagram, &widths, &options, previous);
            Ok((Arc::new(diagram), Arc::new(out)))
        },
        // Half written: show what was there, and try again next time.
        Err(problem) => match before {
            Some(earlier) if fence.open => return earlier,
            _ => Err(problem.to_string()),
        },
    };
    LAID.with_borrow_mut(|laid| laid.insert(kind, fence.source.clone(), result.clone()));
    result
}

/// Whether `diagram` is more than a document lays out as it draws.
fn too_big(diagram: &Diagram) -> bool {
    diagram.nodes.len() + diagram.groups.len() > MOST_INLINE
        || diagram.edges.len() + diagram.messages.len() + diagram.notes.len() > MOST_INLINE_LINES
}

fn draw(fence: &Fence, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme();
    let (border, muted, background) = (theme.border, theme.muted_foreground, theme.background);
    let frame = div()
        .relative()
        .w_full()
        .my_2()
        .rounded(px(10.0))
        .border_1()
        .border_color(border)
        .overflow_hidden();
    match laid(fence, window, cx) {
        Ok((diagram, layout)) => {
            let mermaid = write::mermaid(&diagram);
            let source = fence.source.clone();
            let action = |label: &'static str, text: String| {
                div()
                    .id(label)
                    .px_2()
                    .py_0p5()
                    .rounded(px(6.0))
                    .text_xs()
                    .text_color(muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(muted.opacity(0.12)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                        cx.stop_propagation();
                    })
                    .child(label)
            };
            let corner = div()
                .absolute()
                .top_1()
                .right_1()
                .flex()
                .gap_1()
                .bg(background.opacity(0.85))
                .rounded(px(6.0))
                .child(action("Copy as Mermaid", mermaid))
                .child(action("Copy source", source));
            frame
                .child(view::picture(diagram, layout, MOST_HEIGHT))
                .child(corner)
                .into_any_element()
        },
        Err(problem) => frame
            .p_3()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(format!("Not drawn: {problem}")),
            )
            .child(
                div()
                    .font_family(theme.mono_font_family.clone())
                    .text_sm()
                    .child(fence.source.clone()),
            )
            .into_any_element(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use moonframe_diagram::Diagram;

    #[test]
    fn a_fence_is_closed_by_a_run_as_long_as_its_opening() {
        assert!(super::closed("```mermaid\nflowchart TD\n A-->B\n```"));
        assert!(super::closed("~~~~dot\ndigraph { a -> b }\n~~~~~\n"));
        assert!(super::closed("  ```mermaid\nflowchart TD\n  ```"));
        // Still arriving: no closing line, or one half written.
        assert!(!super::closed("```mermaid\nflowchart TD\n A-->B"));
        assert!(!super::closed("````mermaid\nflowchart TD\n```"));
        assert!(!super::closed("```mermaid\nflowchart TD\n``"));
        assert!(!super::closed("```mermaid"));
        assert!(!super::closed("~~~dot\ndigraph {}\n```"));
    }

    #[test]
    fn a_fence_with_few_nodes_but_many_lines_is_too_big() {
        let mut diagram = Diagram::default();
        for _ in 0..1_000 {
            diagram.messages.push(moonframe_diagram::Message::default());
        }
        assert!(super::too_big(&diagram));
        assert!(!super::too_big(&Diagram::default()));
    }
}
