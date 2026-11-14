//! `scrolled` and `scroll_at` on a scrolling document: how far down its text
//! the reader is, as a fraction from 0 (the top) to 1 (the end at the
//! bottom of the view), told as the reader moves and set when the mod asks.
//!
//! The document's list keeps where it is as a block and an offset into it.
//! A move of that place, with the same blocks, is the reader scrolling (the
//! wheel, the scrollbar, a jump to a heading). A new parse that changes the
//! blocks moves it too, and a document whose text changes is not the reader
//! moving, so then it is only noted. Nor is a move the mod asked for: the
//! list scrolls at once, so [`ScrollFollow`] notes it as landed there.

use {
    gpui_kit::{component::text::TextViewState, *},
    moonframe_core::{Data, Markdown, ScrollAt, ScrollFollow},
};

/// Where a document was last seen scrolled.
#[derive(Clone, Copy, PartialEq)]
struct Seen {
    /// The list's blocks: a new count is a new parse, not a scroll.
    blocks: usize,
    top: ListOffset,
    fraction: f32,
}

/// What a document keeps of its scrolling between frames.
#[derive(Default)]
pub(super) struct Scrolling {
    msg: Option<Data>,
    follow: ScrollFollow,
    seen: Option<Seen>,
}

/// What became of a waiting `scroll_at`.
#[derive(PartialEq)]
pub(super) enum Applied {
    /// Nothing waited, or it was carried out.
    Done,
    /// The document has no height yet, so it waits for one: the next draw
    /// that gives it one tries again, and no frame is asked for meanwhile.
    NoRoom,
}

impl Scrolling {
    /// Takes this frame's `scrolled` and `scroll_at`.
    pub(super) fn take(&mut self, markdown: &Markdown) {
        self.msg.clone_from(&markdown.scrolled);
        self.follow.take(markdown.scroll_at);
    }

    /// Whether a `scroll_at` waits for the document to be laid out.
    pub(super) fn pending(&self) -> bool {
        self.follow.pending()
    }

    /// Carries out a waiting `scroll_at`, once the list has a size.
    pub(super) fn apply(&mut self, state: &Entity<TextViewState>, cx: &mut App) -> Applied {
        if !self.follow.pending() {
            return Applied::Done;
        }
        let list = state.read(cx).list_state().clone();
        if list.viewport_bounds().size.height <= px(0.) {
            return Applied::NoRoom;
        }
        let max = list.max_offset_for_scrollbar().y.as_f32();
        let offset = -list.scroll_px_offset_for_scrollbar().y.as_f32();
        if let Some(y) = self.follow.scroll(offset, max) {
            list.set_offset_from_scrollbar(point(px(0.), px(-y)));
            self.follow.landed(y);
        }
        // The mod's own move: not one to tell it of.
        self.seen = Some(seen(&list));
        state.update(cx, |_, cx| cx.notify());
        Applied::Done
    }

    /// The message to send if the reader moved the document since it was
    /// last seen.
    pub(super) fn moved(&mut self, state: &Entity<TextViewState>, cx: &App) -> Option<Data> {
        let now = seen(state.read(cx).list_state());
        let before = self.seen.replace(now)?;
        let msg = self.msg.as_ref()?;
        let moved =
            before.blocks == now.blocks && before.top != now.top && before.fraction != now.fraction;
        moved.then(|| ScrollAt::message(msg, now.fraction))
    }
}

fn seen(list: &ListState) -> Seen {
    let max = list.max_offset_for_scrollbar().y;
    let offset = -list.scroll_px_offset_for_scrollbar().y;
    Seen {
        blocks: list.item_count(),
        top: list.logical_scroll_top(),
        fraction: ScrollAt::fraction(offset.as_f32(), max.as_f32()),
    }
}
