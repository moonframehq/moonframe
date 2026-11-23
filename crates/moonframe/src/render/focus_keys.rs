//! A row's or column's own `keys`, taken while the keyboard is inside it:
//! a click anywhere in it gives it focus, as does a control in it that
//! takes focus, and while focus is there (and no text field has it) its
//! keys send their messages, ahead of the page's own `keys`.

use {
    super::{Cx, Painter, Place},
    gpui_kit::FocusHandle,
    moonframe_core::Keys,
    std::{cell::RefCell, collections::HashMap},
};

thread_local! {
    /// Each stack's focus, by its window and place, so it stays the same
    /// handle from frame to frame.
    static HANDLES: RefCell<HashMap<String, FocusHandle>> = RefCell::new(HashMap::new());
}

/// A stack with keys a frame drew: innermost first, as children are drawn
/// before the stack around them.
#[derive(Clone)]
pub(crate) struct FocusKeys {
    pub(crate) focus: FocusHandle,
    /// The mod that drew it, whose `update` gets the messages.
    pub(crate) owner: String,
    pub(crate) keys: Keys,
}

impl Painter<'_, '_> {
    /// The focus the stack at `place` tracks, kept with its `keys` for the
    /// keystrokes of this window; none for a stack without keys, or in a
    /// preview.
    pub(super) fn focus_keys(
        &mut self,
        keys: &Keys,
        place: &Place<'_>,
        cx: Cx,
    ) -> Option<FocusHandle> {
        if keys.is_empty() || self.preview {
            return None;
        }
        let key = format!("{:?}/{}", self.window, place.path);
        let focus = HANDLES.with_borrow_mut(|handles| {
            handles
                .entry(key)
                .or_insert_with(|| cx.focus_handle())
                .clone()
        });
        self.focus_keys.push(FocusKeys {
            focus: focus.clone(),
            owner: place.owner.to_owned(),
            keys: keys.clone(),
        });
        Some(focus)
    }
}
