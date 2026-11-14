//! `scrolled` and `scroll_at` on a scrolling document, as on GPUI: how far
//! down its text the reader is, as a fraction from 0 (the top) to 1 (the end
//! at the bottom of the view), told as the reader moves and set when the mod
//! asks. Where the text sits in its scroll, from the last layout, says how
//! far it is scrolled. A move as the text changes length is the text, not
//! the reader, and the move a `scroll_at` asked for is the mod's own: neither
//! is told. [`ScrollFollow`] tells the reader's moves from the mod's.

use {
    moonframe_core::{Data, Markdown, SCROLL_JITTER, ScrollAt, ScrollFollow},
    std::hash::{DefaultHasher, Hash, Hasher},
};

/// What a document keeps of its scrolling.
#[derive(Default)]
pub(crate) struct Scrolling {
    follow: ScrollFollow,
    /// How tall its text was last seen, in points.
    height: Option<f32>,
    /// The text it was last drawn with, hashed: the layout a draw reads is
    /// the last frame's, so it is not yet of a text that just changed.
    text: Option<u64>,
}

/// What one look at a document found and asks for.
#[derive(Default)]
pub(super) struct Looked {
    /// A place to scroll to at once, in points down the text.
    pub(super) scroll: Option<f32>,
    /// The message to send.
    pub(super) tell: Option<Data>,
    /// A `scroll_at` waits for this text to be laid out: draw again.
    pub(super) again: bool,
}

impl Scrolling {
    /// Looks at a document whose scroll and text were laid out at `view` and
    /// `text` (`[x, y, width, height]` in the window): takes `markdown`'s
    /// `scroll_at`, and says whether the reader moved it.
    pub(super) fn look(
        &mut self,
        markdown: &Markdown,
        view: Option<[f32; 4]>,
        text: Option<[f32; 4]>,
    ) -> Looked {
        self.follow.take(markdown.scroll_at);
        let mut hasher = DefaultHasher::new();
        markdown.text.hash(&mut hasher);
        let hash = hasher.finish();
        let new_text = self.text.replace(hash).is_some_and(|before| before != hash);
        let (Some([_, top, _, shown]), Some([_, y, _, height])) = (view, text) else {
            return Looked::default();
        };
        let offset = top - y;
        let max = (height - shown).max(0.);
        let mut looked = Looked::default();
        if self.follow.pending() {
            if new_text {
                // Its layout is of the old text: wait for the new one.
                looked.again = true;
                return looked;
            }
            looked.scroll = self.follow.scroll(offset, max);
        }
        let grew = self
            .height
            .replace(height)
            .is_some_and(|before| (height - before).abs() >= SCROLL_JITTER);
        let reader = !new_text && !grew;
        if let (Some(fraction), Some(msg)) =
            (self.follow.moved(offset, max, reader), &markdown.scrolled)
        {
            looked.tell = Some(ScrollAt::message(msg, fraction));
        }
        looked
    }
}
