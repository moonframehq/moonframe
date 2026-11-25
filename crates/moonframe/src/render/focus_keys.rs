//! A row's or column's own `keys`, taken while the keyboard is inside it:
//! a click anywhere in it gives it focus, as does a control in it that
//! takes focus, and while focus is there (and no text field has it) its
//! keys send their messages, ahead of the page's own `keys`.

use {
    super::{Cx, Painter, Place},
    gpui_kit::{FocusHandle, WindowId},
    moonframe_core::Keys,
    std::{cell::RefCell, collections::HashMap},
};

/// Each window's stacks' focus, by place.
type Handles = HashMap<Option<WindowId>, HashMap<String, FocusHandle>>;

thread_local! {
    /// Each stack's focus, by its window and place, so it stays the same
    /// handle from frame to frame while the stack is drawn.
    static HANDLES: RefCell<Handles> = RefCell::new(HashMap::new());
}

/// A stack with keys a frame drew: innermost first, as children are drawn
/// before the stack around them.
#[derive(Clone)]
pub(crate) struct FocusKeys {
    pub(crate) focus: FocusHandle,
    /// The mod that drew it, whose `update` gets the messages.
    pub(crate) owner: String,
    pub(crate) keys: Keys,
    /// Its place, which its handle is kept by.
    path: String,
    /// The paint priority of the layer it is drawn in: 0 for the window,
    /// an overlay's inside one, whose backdrop keeps keys from those below.
    pub(crate) layer: usize,
}

/// A frame of `window` is drawn, with the stacks in `drawn`: the handles of
/// those it no longer drew go (rows scrolled away in a list, a closed tab).
pub(crate) fn drawn(window: WindowId, drawn: &[FocusKeys]) {
    HANDLES.with_borrow_mut(|handles| keep(handles, Some(window), drawn));
}

/// Windows other than `open` closed: their handles go.
pub(crate) fn closed(open: &[WindowId]) {
    HANDLES.with_borrow_mut(|handles| {
        handles.retain(|window, _| window.is_none_or(|window| open.contains(&window)));
    });
}

fn keep(handles: &mut Handles, window: Option<WindowId>, drawn: &[FocusKeys]) {
    if let Some(places) = handles.get_mut(&window) {
        places.retain(|path, _| drawn.iter().any(|stack| stack.path == *path));
    }
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
        let focus = HANDLES.with_borrow_mut(|handles| {
            handles
                .entry(self.window)
                .or_default()
                .entry(place.path.clone())
                .or_insert_with(|| cx.focus_handle())
                .clone()
        });
        self.focus_keys.push(FocusKeys {
            focus: focus.clone(),
            owner: place.owner.to_owned(),
            keys: keys.clone(),
            path: place.path.clone(),
            layer: self.layer,
        });
        Some(focus)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{FocusKeys, Handles, keep},
        gpui_kit::TestAppContext,
        std::collections::HashMap,
    };

    #[gpui_kit::test]
    fn a_stack_no_longer_drawn_lets_go_of_its_focus(cx: &mut TestAppContext) {
        let mut handles: Handles = HashMap::new();
        let stack = cx.update(|cx| FocusKeys {
            focus: cx.focus_handle(),
            owner: "list".to_owned(),
            keys: Default::default(),
            path: "row[1]".to_owned(),
            layer: 0,
        });
        let places = handles.entry(None).or_default();
        places.insert("row[1]".to_owned(), stack.focus.clone());
        places.insert("row[2]".to_owned(), stack.focus.clone());
        keep(&mut handles, None, &[stack]);
        let places = &handles[&None];
        assert!(places.contains_key("row[1]") && !places.contains_key("row[2]"));
    }
}
