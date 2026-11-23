//! Holding the text being read where it is when a scrolling document's text
//! changes. The view keeps its place as a block and an offset into it, and
//! resets to the top when a parse changes how many blocks there are, so an
//! edit above the reader (a live preview's source being typed in) would move
//! or lose what they were reading.
//!
//! Before the text changes, the place is noted as bytes into the source and
//! carried through the edit: kept where the text before or after it is
//! unchanged, and taken to the edit's start when inside it. Once the new
//! parse is laid out, the view goes back to the block holding those bytes.
//! An append keeps every block where it was, so it holds nothing.

use {
    super::outline::Outline,
    gpui_kit::{component::text::TextViewState, *},
    std::sync::Arc,
};

/// Where the reader was, in the text the document shows now.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hold {
    /// Bytes into the source of the top block's start.
    at: usize,
    /// How far into that block the view was scrolled.
    offset: Pixels,
    /// Whether the block is unchanged, so `offset` still means the same.
    exact: bool,
}

/// What a scrolling document keeps to hold its place across edits.
#[derive(Default)]
pub(super) struct Holding {
    hold: Option<Hold>,
    /// The structure of the text the document shows, once read off the UI
    /// thread; `None` while it is read or after an append.
    outline: Option<Arc<Outline>>,
    reading: Option<Task<()>>,
}

impl Holding {
    /// Whether a place waits to be gone back to.
    pub(super) fn waiting(&self) -> bool {
        self.hold.is_some()
    }

    /// Notes the place before the shown text `old` becomes `new`, and says
    /// whether `new` should be read for the next edit.
    pub(super) fn changes(&mut self, old: &str, new: &str, list: &ListState) -> bool {
        let outline = self.outline.take();
        if new.starts_with(old) {
            // An append splices blocks after the reader's, and a streamed
            // answer would read its whole text on every word.
            return false;
        }
        if let Some(hold) = &mut self.hold {
            *hold = hold.through(old, new);
            return true;
        }
        let top = list.logical_scroll_top();
        if top.item_ix == 0 && top.offset_in_item <= px(0.) {
            return true;
        }
        // Only the first edit after an append or a launch reads here.
        let outline = outline.unwrap_or_else(|| Arc::new(Outline::parse(old)));
        if let Some(at) = outline.blocks().get(top.item_ix) {
            let hold = Hold {
                at: *at,
                offset: top.offset_in_item,
                exact: true,
            };
            self.hold = Some(hold.through(old, new));
        }
        true
    }

    /// Reads `text`, the document's new text, off the UI thread; `done`
    /// runs on the UI thread once it is read.
    pub(super) fn read<T: AsMut<Self> + 'static>(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<T>,
        done: impl FnOnce(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) {
        let parsed = cx.background_spawn(async move { Outline::parse(&text) });
        self.reading = Some(cx.spawn_in(window, async move |this, cx| {
            let outline = parsed.await;
            this.update_in(cx, |this, window, cx| {
                let holding: &mut Self = this.as_mut();
                holding.outline = Some(Arc::new(outline));
                holding.reading = None;
                done(this, window, cx);
            })
            .ok();
        }));
    }

    /// Goes back to the held place, once the new text has been read and
    /// laid out. Says whether it moved the view.
    pub(super) fn apply(&mut self, state: &Entity<TextViewState>, cx: &mut App) -> bool {
        let (Some(hold), Some(outline)) = (self.hold, self.outline.as_ref()) else {
            return false;
        };
        self.hold = None;
        let item_ix = outline.block_at(hold.at);
        let exact = hold.exact && outline.blocks().get(item_ix) == Some(&hold.at);
        let list = state.read(cx).list_state().clone();
        list.scroll_to(ListOffset {
            item_ix,
            offset_in_item: if exact {
                hold.offset
            } else {
                px(0.)
            },
        });
        state.update(cx, |_, cx| cx.notify());
        true
    }
}

impl Hold {
    /// This place in `new`, the text `old` became.
    fn through(self, old: &str, new: &str) -> Self {
        let (prefix, suffix) = unchanged(old, new);
        let (at, exact) = if self.at < prefix {
            (self.at, self.exact)
        } else if self.at >= old.len() - suffix {
            (self.at + new.len() - old.len(), self.exact)
        } else {
            (prefix, false)
        };
        Self { at, exact, ..self }
    }
}

/// How many bytes `old` and `new` share at their start and their end, on
/// character boundaries, not overlapping in either. The end is matched
/// first: text written above a block that begins as it does (`# One` above
/// `# Two`) is then new text, and the block keeps its place under it.
fn unchanged(old: &str, new: &str) -> (usize, usize) {
    let mut suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - suffix) {
        suffix -= 1;
    }
    let room = old.len().min(new.len()) - suffix;
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(prefix) {
        prefix -= 1;
    }
    (prefix, suffix)
}

#[cfg(test)]
mod tests {
    use {
        super::{Hold, unchanged},
        gpui_kit::px,
    };

    fn held(at: usize) -> Hold {
        Hold {
            at,
            offset: px(4.),
            exact: true,
        }
    }

    #[test]
    fn text_written_above_moves_the_place_down_with_it() {
        let old = "# One\n\nbody\n\n# Two\n";
        let new = "intro\n\n# One\n\nbody\n\n# Two\n";
        let at = old.find("# Two").unwrap_or_default();
        assert_eq!(held(at).through(old, new), Hold {
            at: new.find("# Two").unwrap_or_default(),
            ..held(0)
        });
    }

    #[test]
    fn a_heading_written_above_one_like_it_leaves_that_one_held() {
        let old = "# Two\n";
        let new = "# One\n\n# Two\n";
        assert_eq!(held(0).through(old, new), held("# One\n\n".len()));
    }

    #[test]
    fn text_written_below_leaves_the_place() {
        let old = "# One\n\nbody\n\n# Two\n";
        let new = "# One\n\nbody\n\nmore\n\n# Two\n";
        assert_eq!(held(0).through(old, new), held(0));
    }

    #[test]
    fn a_rewritten_block_holds_where_it_starts_without_its_offset() {
        let old = "# One\n\nold block\n\n# Two\n";
        let new = "# One\n\nnew text\n\n# Two\n";
        let moved = held("# One\n\n".len()).through(old, new);
        assert_eq!(moved.at, "# One\n\n".len());
        assert!(!moved.exact);
    }

    #[test]
    fn shared_ends_stop_at_character_boundaries() {
        assert_eq!(unchanged("aé", "aè"), (1, 0));
        assert_eq!(unchanged("éa", "èa"), (0, 1));
        assert_eq!(unchanged("abc", "abc"), (0, 3));
        assert_eq!(unchanged("aa", "aaa"), (0, 2));
    }
}
