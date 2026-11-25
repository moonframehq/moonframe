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
    std::{
        cell::RefCell,
        collections::HashMap,
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    },
};

const EMOJI: &str = "moonframe-emoji";
/// How often a missing emoji is looked for again, as a provider may be
/// downloading it, and for how long.
const AGAIN: Duration = Duration::from_secs(1);
const AWHILE: Duration = Duration::from_secs(60);

thread_local! {
    /// Emoji not there yet, by address: when first missed, and whether a
    /// look is due.
    static WAITING: RefCell<HashMap<String, (Instant, bool)>> = RefCell::new(HashMap::new());
}

/// Whether to look for the missing emoji `src` again in a while: for
/// [`AWHILE`] after it was first missed, once at a time.
fn look_again(src: &str, now: Instant) -> bool {
    WAITING.with_borrow_mut(|waiting| {
        let (since, due) = waiting.entry(src.to_owned()).or_insert((now, false));
        if *due || now.saturating_duration_since(*since) > AWHILE {
            return false;
        }
        *due = true;
        true
    })
}

/// The look for `src` is done: the next miss may ask for another.
fn looked(src: &str) {
    WAITING.with_borrow_mut(|waiting| {
        if let Some((_, due)) = waiting.get_mut(src) {
            *due = false;
        }
    });
}

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
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let emoji = node.data::<Arc<Emoji>>()?.clone();
        let alt = emoji.alt.clone();
        let Some(path) = cached(&emoji.src, &self.0) else {
            // A provider may still be downloading it: draw again in a
            // while, when the lookup is tried anew.
            if look_again(&emoji.src, Instant::now()) {
                let src = emoji.src.clone();
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor().timer(AGAIN).await;
                        looked(&src);
                        cx.update(|window, _| window.refresh()).ok();
                    })
                    .detach();
            }
            return Some(InlineElement::new(div().child(alt)));
        };
        WAITING.with_borrow_mut(|waiting| waiting.remove(&emoji.src));
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
    crate::render::images::resolve_cached(src, &owner, &folder).filter(|path| {
        crate::paths::images()
            .canonicalize()
            .is_ok_and(|cache| path.starts_with(cache))
    })
}

#[cfg(test)]
mod tests {
    use {
        super::{AWHILE, is_emoji, look_again, looked},
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

    #[test]
    fn a_missing_emoji_is_looked_for_again_for_a_while() {
        let start = std::time::Instant::now();
        let src = "emoji/later.png";
        assert!(look_again(src, start));
        // One look at a time.
        assert!(!look_again(src, start));
        looked(src);
        assert!(look_again(src, start + std::time::Duration::from_secs(1)));
        looked(src);
        assert!(!look_again(
            src,
            start + AWHILE + std::time::Duration::from_secs(1)
        ));
    }
}
