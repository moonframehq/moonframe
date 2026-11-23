//! A picture drawn as a character of the text, as a chat draws a custom
//! emoji: `![:party:](~/.cache/moonframe/images/emoji/party.png "emoji")`,
//! inline at the line's height, from the image cache only, whether or not
//! the document shows pictures. Until it loads, or when it is not there,
//! its alt text stands in its place.

use {
    super::document::SharedRef,
    gpui_kit::{
        component::text::{
            InlineElement, InlineRenderContext, MarkdownNode, MarkdownParseContext, MarkdownPlugin,
            markdown_ast::Node,
        },
        *,
    },
    std::{path::PathBuf, sync::Arc},
};

const EMOJI: &str = "moonframe-emoji";

/// The title that marks a picture as one.
pub(super) const TITLE: &str = "emoji";

struct Emoji {
    src: String,
    alt: SharedString,
}

/// Whether `node` is a picture marked as an emoji.
pub(super) fn is_emoji(node: &Node) -> bool {
    matches!(node, Node::Image(image) if image.title.as_deref() == Some(TITLE))
}

pub(super) struct Emojis(pub(super) SharedRef);

impl MarkdownPlugin for Emojis {
    fn name(&self) -> &str {
        EMOJI
    }

    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Image(image) = node else {
            return None;
        };
        if image.title.as_deref() != Some(TITLE) {
            return None;
        }
        let emoji = Emoji {
            src: image.url.clone(),
            alt: image.alt.clone().into(),
        };
        let node_markdown = cx.node_source(node).map(str::to_owned);
        let parsed = MarkdownNode::new(EMOJI, Arc::new(emoji)).text(image.alt.clone());
        Some(match node_markdown {
            Some(markdown) => parsed.markdown(markdown),
            None => parsed,
        })
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        _: &mut Window,
        _: &mut App,
    ) -> Option<InlineElement> {
        let emoji = node.data::<Arc<Emoji>>()?.clone();
        let alt = emoji.alt.clone();
        let Some(path) = cached(&emoji.src, &self.0) else {
            return Some(InlineElement::new(div().child(alt)));
        };
        let side = context.line_height();
        let fallback = alt.clone();
        Some(InlineElement::new(
            img(path)
                .size(side)
                .object_fit(ObjectFit::Contain)
                .with_fallback(move || div().child(fallback.clone()).into_any_element()),
        ))
    }
}

/// The file `src` names in the image cache, if it is there: never from the
/// network, nor anywhere else, whatever the document may show.
fn cached(src: &str, shared: &SharedRef) -> Option<PathBuf> {
    if src.contains("://") || src.starts_with("data:") {
        return None;
    }
    let (owner, folder) = {
        let shared = shared.lock().ok()?;
        (shared.owner.clone(), shared.folder.clone())
    };
    crate::render::images::resolve_src(src, &owner, &folder, || vec![crate::paths::images()])
        .filter(|path| {
            crate::paths::images()
                .canonicalize()
                .is_ok_and(|cache| path.starts_with(cache))
        })
}

#[cfg(test)]
mod tests {
    use {
        super::is_emoji,
        gpui_kit::component::text::markdown_ast::{Image, Node},
    };

    fn picture(title: Option<&str>) -> Node {
        Node::Image(Image {
            position: None,
            alt: ":party:".to_owned(),
            url: "~/.cache/moonframe/images/emoji/party.png".to_owned(),
            title: title.map(str::to_owned),
        })
    }

    #[test]
    fn only_a_picture_titled_emoji_is_one() {
        assert!(is_emoji(&picture(Some("emoji"))));
        assert!(!is_emoji(&picture(None)));
        assert!(!is_emoji(&picture(Some("A party"))));
    }
}
