//! `scrolled` and `scroll_at` on a multiline field: how far down its text it
//! is scrolled, as a fraction from 0 (the top) to 1 (the last line at the
//! bottom of the field), told as it moves and set when the mod asks. The
//! room a code editor leaves under its last line reads as 1.
//!
//! Any move of the text is told (the wheel, the scrollbar, the caret
//! carrying it along as the user types) but the one a `scroll_at` asked
//! for, so two fields that follow each other do not echo. [`ScrollFollow`]
//! tells the two apart.

use {
    super::{Editor, Inputs},
    gpui_kit::*,
    moonframe_core::{Action, Input, ScrollAt, ScrollFollow},
};

impl Editor {
    /// Takes the field's `scroll_at`, and scrolls there once it is laid out.
    pub(super) fn take_scroll(&self, follow: &mut ScrollFollow, input: &Input, cx: &mut App) {
        follow.take(input.scroll_at);
        self.follow(follow, cx);
    }

    /// Carries out a waiting `scroll_at`, once the field is laid out.
    fn follow(&self, follow: &mut ScrollFollow, cx: &mut App) {
        if !follow.pending() {
            return;
        }
        let Some((offset, max)) = self.scrolled(cx) else {
            return;
        };
        if let Some(y) = follow.scroll(offset, max) {
            self.scroll_down(y, cx);
        }
    }

    /// How far down its text the field is scrolled and the most it can be
    /// while its last line is in view, in pixels; `None` until it is laid
    /// out. `input_bounds` leaves out the field's padding, so `max` is
    /// where gpui-base's own clamp puts the last line at the bottom; it
    /// scrolls further only into a code editor's room under that line.
    fn scrolled(&self, cx: &App) -> Option<(f32, f32)> {
        on_state!(self, |state| {
            let state = state.read(cx);
            let line = state.line_height()?;
            let view = state.input_bounds().size.height;
            if view <= px(0.) {
                return None;
            }
            let rows = state.display_map().wrap_row_count();
            let max = (line * rows as f32 - view).max(px(0.));
            Some((-state.scroll_offset().y.as_f32(), max.as_f32()))
        })
    }

    /// Scrolls the text `y` pixels down from its top, at the next layout.
    fn scroll_down(&self, y: f32, cx: &mut App) {
        on_state!(self, |state| state.update(cx, |state, cx| {
            let x = state.scroll_offset().x;
            state.set_scroll_offset(point(x, px(-y)), cx);
        }))
    }
}

impl Inputs {
    /// What the field at `key` sends of a move of its text since it was last
    /// looked at, from the mod that drew it.
    pub(super) fn scrolled(&self, key: &str, cx: &mut App) -> Option<(String, Action)> {
        let mut registry = self.0.borrow_mut();
        let field = registry.fields.get_mut(key)?;
        if !field.wiring.multiline {
            return None;
        }
        field.editor.follow(&mut field.scrolling, cx);
        // A field that tells nothing need not be looked at.
        let Some(msg) = field.wiring.scrolled.as_ref() else {
            field.scrolling.forget();
            return None;
        };
        let (offset, max) = field.editor.scrolled(cx)?;
        let fraction = field.scrolling.moved(offset, max, true)?;
        Some((field.wiring.owner.clone(), Action::Send {
            to: field.wiring.to.clone(),
            msg: ScrollAt::message(msg, fraction),
        }))
    }
}
