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
    std::{cell::RefCell, collections::HashMap, path::PathBuf, sync::Arc, time::Duration},
};

const EMOJI: &str = "moonframe-emoji";
/// How soon a missing emoji is looked for again, as a provider may be
/// downloading it; each look waits twice as long as the one before, up to
/// [`LONGEST`], for as long as it is drawn.
const AGAIN: Duration = Duration::from_secs(1);
const LONGEST: Duration = Duration::from_secs(30);

/// A missing emoji a window waits for.
struct Waiting {
    /// How long the next look waits.
    wait: Duration,
    /// A look is on its way.
    due: bool,
}

thread_local! {
    /// Emoji not there yet, by address and window. Each window waiting
    /// draws itself again.
    static WAITING: RefCell<HashMap<(String, WindowId), Waiting>> =
        RefCell::new(HashMap::new());
}

/// How long to wait before looking for the missing emoji `src` again in
/// `window`, drawn there: none while a look is already on its way.
fn look_again(src: &str, window: WindowId) -> Option<Duration> {
    WAITING.with_borrow_mut(|waiting| {
        let key = (src.to_owned(), window);
        let waiting = waiting.entry(key).or_insert(Waiting {
            wait: AGAIN,
            due: false,
        });
        if waiting.due {
            return None;
        }
        waiting.due = true;
        let wait = waiting.wait;
        waiting.wait = (wait * 2).min(LONGEST);
        Some(wait)
    })
}

/// The look for `src` in `window` is done: the frame it draws asks for the
/// next one, if it still shows the emoji missing.
fn looked(src: &str, window: WindowId) {
    WAITING.with_borrow_mut(|waiting| {
        if let Some(waiting) = waiting.get_mut(&(src.to_owned(), window)) {
            waiting.due = false;
        }
    });
}

/// A while after a look: no frame asked for another, so nothing shows `src`
/// missing in `window` any more (its document or window is gone), and it
/// is forgotten.
fn forget_unless_asked(src: &str, window: WindowId) {
    WAITING.with_borrow_mut(|waiting| {
        let key = (src.to_owned(), window);
        if waiting.get(&key).is_some_and(|waiting| !waiting.due) {
            waiting.remove(&key);
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
            let id = window.window_handle().window_id();
            if let Some(wait) = look_again(&emoji.src, id) {
                let src = emoji.src.clone();
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor().timer(wait).await;
                        looked(&src, id);
                        cx.update(|window, _| window.refresh()).ok();
                        cx.background_executor().timer(AGAIN).await;
                        forget_unless_asked(&src, id);
                    })
                    .detach();
            }
            return Some(InlineElement::new(div().child(alt)));
        };
        if WAITING.with_borrow(|waiting| !waiting.is_empty()) {
            let found = (emoji.src.clone(), window.window_handle().window_id());
            WAITING.with_borrow_mut(|waiting| waiting.remove(&found));
        }
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
        super::{AGAIN, WAITING, WindowId, forget_unless_asked, is_emoji, look_again, looked},
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
    fn a_missing_emoji_is_looked_for_again_ever_less_often() {
        let src = "emoji/later.png";
        let (one, two) = (WindowId::from(1), WindowId::from(2));
        assert_eq!(look_again(src, one), Some(AGAIN));
        // One look at a time in a window; each window looks for itself.
        assert_eq!(look_again(src, one), None);
        assert_eq!(look_again(src, two), Some(AGAIN));
        let waits: Vec<u64> = (0..7)
            .filter_map(|_| {
                looked(src, one);
                look_again(src, one).map(|wait| wait.as_secs())
            })
            .collect();
        // However long it has been missing, it is still looked for.
        assert_eq!(waits, [2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn an_emoji_no_frame_draws_after_a_look_is_forgotten() {
        let (gone, shown) = ("emoji/gone.png", "emoji/shown.png");
        let window = WindowId::from(1);
        for src in [gone, shown] {
            assert_eq!(look_again(src, window), Some(AGAIN));
            looked(src, window);
        }
        // Only the frame after the look drew `shown`, asking for the next.
        assert_eq!(look_again(shown, window), Some(AGAIN * 2));
        for src in [gone, shown] {
            forget_unless_asked(src, window);
        }
        WAITING.with_borrow(|waiting| {
            assert!(!waiting.contains_key(&(gone.to_owned(), window)));
            assert!(waiting.contains_key(&(shown.to_owned(), window)));
        });
        // It goes on waiting longer each time.
        looked(shown, window);
        assert_eq!(look_again(shown, window), Some(AGAIN * 4));
    }
}
