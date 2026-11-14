//! `scrolled` and `scroll_at` on a scrolling document, as on GPUI: how far
//! down its text the reader is, as a fraction from 0 (the top) to 1 (the end
//! at the bottom of the view), told as the reader moves and set when the mod
//! asks. Where the text sits in its scroll, from the last layout, says how
//! far it is scrolled. A move as the text changes length is the text, not
//! the reader, and the move a `scroll_at` asked for is the mod's own: neither
//! is told.

use moonframe_core::{Data, Markdown, ScrollAt};

/// How many views a `scroll_at` may take to show before the document is
/// taken to have gone elsewhere.
const PATIENCE: u8 = 8;

/// Points of a move too small to tell.
const JITTER: f32 = 0.5;

/// Where a document was last seen scrolled, in points: how far down, and
/// how tall its text is.
#[derive(Clone, Copy)]
struct Seen {
    offset: f32,
    height: f32,
}

/// What a document keeps of its scrolling.
#[derive(Default)]
pub(crate) struct Scrolling {
    /// The last `scroll_at` taken, and its fraction until it is carried out.
    taken: Option<ScrollAt>,
    pending: Option<f32>,
    /// The offset a `scroll_at` asked for, until it shows, and the views it
    /// has left.
    expect: Option<(f32, u8)>,
    seen: Option<Seen>,
}

/// What one look at a document found and asks for.
#[derive(Default)]
pub(super) struct Looked {
    /// A place to scroll to at once, in points down the text.
    pub(super) scroll: Option<f32>,
    /// The message to send.
    pub(super) tell: Option<Data>,
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
        if markdown.scroll_at != self.taken {
            self.taken = markdown.scroll_at;
            self.pending = markdown.scroll_at.map(|at| at.fraction);
        }
        let (Some([_, top, _, shown]), Some([_, y, _, height])) = (view, text) else {
            return Looked::default();
        };
        let now = Seen {
            offset: top - y,
            height,
        };
        let max = (height - shown).max(0.);
        let mut looked = Looked::default();
        if let Some(fraction) = self.pending.take() {
            let offset = max * fraction;
            self.expect = Some((offset, PATIENCE));
            looked.scroll = Some(offset);
            return looked;
        }
        if let Some((offset, views)) = self.expect {
            if (now.offset - offset).abs() < JITTER || views == 0 {
                self.expect = None;
                self.seen = Some(now);
            } else {
                self.expect = Some((offset, views - 1));
            }
            return looked;
        }
        let Some(before) = self.seen.replace(now) else {
            return looked;
        };
        let moved = (now.offset - before.offset).abs() >= JITTER
            && (now.height - before.height).abs() < JITTER;
        if let (true, Some(msg)) = (moved, &markdown.scrolled) {
            looked.tell = Some(ScrollAt::message(msg, ScrollAt::fraction(now.offset, max)));
        }
        looked
    }
}
