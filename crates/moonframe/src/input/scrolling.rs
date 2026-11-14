//! `scrolled` and `scroll_at` on a multiline field: how far down its text it
//! is scrolled, as a fraction from 0 (the top) to 1 (the last line at the
//! bottom of the field), told as it moves and set when the mod asks. The
//! room a code editor leaves under its last line reads as 1.
//!
//! Any move of the text is told (the wheel, the scrollbar, the caret
//! carrying it along as the user types) but the one a `scroll_at` asked
//! for, so two fields that follow each other do not echo.

use {
    super::{Editor, Inputs},
    gpui_kit::*,
    moonframe_core::{Action, Input, ScrollAt},
};

/// How often a field is looked at, after a `scroll_at`, before its text is
/// taken to have gone elsewhere: the engine scrolls at its next layout.
const PATIENCE: u8 = 8;

/// Pixels of a move too small to tell.
const JITTER: f32 = 0.5;

/// What a field keeps of its scrolling.
#[derive(Default)]
pub(super) struct Scrolling {
    /// The last `scroll_at` taken, and its fraction until it is carried out.
    taken: Option<ScrollAt>,
    pending: Option<f32>,
    /// The offset a `scroll_at` asked for, until the field shows it, and
    /// the looks it has left.
    expect: Option<(f32, u8)>,
    /// The offset last seen, in pixels down the text.
    seen: Option<f32>,
}

impl Scrolling {
    /// Takes the field's `scroll_at`, and scrolls there once it is laid out.
    pub(super) fn take(&mut self, input: &Input, editor: &Editor, cx: &mut App) {
        if input.scroll_at != self.taken {
            self.taken = input.scroll_at;
            self.pending = input.scroll_at.map(|at| at.fraction);
        }
        self.apply(editor, cx);
    }

    fn apply(&mut self, editor: &Editor, cx: &mut App) {
        let Some(fraction) = self.pending else {
            return;
        };
        let Some((_, max)) = editor.scrolled(cx) else {
            return;
        };
        self.pending = None;
        let y = max * fraction;
        self.expect = Some((y, PATIENCE));
        editor.scroll_down(y, cx);
    }

    /// The fraction to tell, if the text moved since last seen other than
    /// where a `scroll_at` took it.
    pub(super) fn moved(&mut self, editor: &Editor, cx: &mut App) -> Option<f32> {
        self.apply(editor, cx);
        let (offset, max) = editor.scrolled(cx)?;
        if let Some((y, looks)) = self.expect {
            if (offset - y).abs() < JITTER || looks == 0 {
                self.expect = None;
                self.seen = Some(offset);
            } else {
                self.expect = Some((y, looks - 1));
            }
            return None;
        }
        let before = self.seen.replace(offset)?;
        ((offset - before).abs() >= JITTER).then(|| ScrollAt::fraction(offset, max))
    }
}

impl Editor {
    /// How far down its text the field is scrolled and the most it can be,
    /// in pixels; `None` until it is laid out.
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
        let fraction = field.scrolling.moved(&field.editor, cx)?;
        let msg = field.wiring.scrolled.as_ref()?;
        Some((field.wiring.owner.clone(), Action::Send {
            to: field.wiring.to.clone(),
            msg: ScrollAt::message(msg, fraction),
        }))
    }
}
