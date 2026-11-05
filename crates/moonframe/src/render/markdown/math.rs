//! LaTeX in a document: `$...$` in a line and `$$...$$` on its own, drawn by
//! latex-rust as an SVG in the text's color. Each formula is drawn once per
//! color and size; a formula it can't read shows as its source.

use {
    super::parts,
    gpui_kit::{
        component::{
            ActiveTheme as _,
            text::{
                InlineElement, InlineRenderContext, MarkdownExtensions, MarkdownNode,
                MarkdownParseContext, MarkdownPlugin, markdown_ast::Node,
            },
        },
        *,
    },
    latex_rust::{Color, Dim, MathFont, MathStyle, SvgOptions, latex_to_svg, layout, parse},
    std::{cell::RefCell, collections::HashMap, sync::Arc},
};

const INLINE: &str = "moonframe-math";
const BLOCK: &str = "moonframe-math-block";

/// The em of a block formula, in points.
const BLOCK_SIZE: i64 = 18;

/// A formula is drawn at points of this many pixels: SVG's own ratio.
const PIXELS_PER_POINT: f32 = 4.0 / 3.0;

type Key = (String, bool, u32, [u8; 3]);

/// A formula as a picture, with its size and how far down it its baseline
/// is, in pixels: the size is known before the picture has loaded, so the
/// line it sits on is laid out once.
#[derive(Clone)]
struct Drawn {
    picture: Arc<Image>,
    width: f32,
    height: f32,
    baseline: f32,
}

impl Drawn {
    fn element(&self) -> Img {
        img(self.picture.clone())
            .w(px(self.width))
            .h(px(self.height))
    }
}

thread_local! {
    static FONT: Option<MathFont> = MathFont::stix_two_math().ok();
    static DRAWN: RefCell<HashMap<Key, Option<Drawn>>> = RefCell::new(HashMap::new());
}

pub(super) fn with(extensions: MarkdownExtensions) -> MarkdownExtensions {
    extensions
        .block_parser(|node, cx| {
            let Node::Math(math) = node else {
                return None;
            };
            Some(
                MarkdownNode::new(BLOCK, math.value.clone())
                    .text(math.value.clone())
                    .markdown(cx.node_source(node)?.to_owned()),
            )
        })
        .block_renderer(BLOCK, |node, _, cx| {
            let source = node.data::<String>().cloned().unwrap_or_default();
            let color = cx.theme().foreground;
            let picture = drawn(&source, true, BLOCK_SIZE as u32, color);
            let frame = div().flex().justify_center().w_full().py_2();
            match picture {
                Some(drawn) => frame.child(drawn.element()).into_any_element(),
                None => frame.child(source).into_any_element(),
            }
        })
        .plugin(Inline)
}

struct Inline;

impl MarkdownPlugin for Inline {
    fn name(&self) -> &str {
        INLINE
    }

    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::InlineMath(math) = node else {
            return None;
        };
        let end = node.position()?.end.offset.checked_sub(cx.offset())?;
        let after = cx.source().get(end..).and_then(|rest| rest.chars().next());
        parts::inline_math(&math.value, after)
            .then(|| MarkdownNode::new(INLINE, math.value.clone()).text(math.value.clone()))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        _: &mut Window,
        _: &mut App,
    ) -> Option<InlineElement> {
        let source = node.data::<String>()?;
        let size = (f32::from(context.font_size()) / PIXELS_PER_POINT).round() as u32;
        let drawn = drawn(source, false, size.max(1), context.text_style().color)?;
        Some(InlineElement::new(drawn.element()).with_baseline(px(drawn.baseline)))
    }
}

/// `source` as a picture, if it reads as math.
fn drawn(source: &str, display: bool, size: u32, color: Hsla) -> Option<Drawn> {
    let rgba = color.to_rgb();
    let rgb = [rgba.r, rgba.g, rgba.b].map(|channel| (channel * 255.0).round() as u8);
    let key = (source.to_owned(), display, size, rgb);
    DRAWN.with_borrow_mut(|drawn| {
        drawn
            .entry(key)
            .or_insert_with(|| {
                FONT.with(|font| {
                    let font = font.as_ref()?;
                    let options = SvgOptions {
                        font_size_pt: Dim::from_i64(i64::from(size)),
                        color: Color::rgb(rgb[0], rgb[1], rgb[2]),
                        display,
                    };
                    let svg = latex_to_svg(source, font, &options).ok()?;
                    let style = if display {
                        MathStyle::Display
                    } else {
                        MathStyle::Text
                    };
                    let tree = layout(&parse(source).ok()?, font, style).ok()?;
                    let pixels = |dim: &Dim| -> Option<f32> {
                        let ems: f32 = dim.to_svg_string().parse().ok()?;
                        Some(ems * size as f32 * PIXELS_PER_POINT)
                    };
                    let baseline = pixels(&tree.height)?;
                    Some(Drawn {
                        picture: Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())),
                        width: pixels(&tree.width)?,
                        height: baseline + pixels(&tree.depth)?,
                        baseline,
                    })
                })
            })
            .clone()
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::drawn;

    #[test]
    fn a_formula_draws_with_its_baseline() {
        let drawn = drawn("E = mc^2", false, 12, gpui_kit::black()).expect("drawn");
        assert!(drawn.baseline > 0.0 && drawn.baseline <= drawn.height);
        assert!(drawn.width > 0.0);
    }

    #[test]
    fn what_is_not_math_draws_nothing() {
        assert!(drawn(r"\frac{", true, 18, gpui_kit::black()).is_none());
    }
}
