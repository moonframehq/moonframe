//! A row's or column's own `keys`, taken while the keyboard is inside it:
//! a click anywhere in it gives it focus, as does a control in it that
//! takes focus, and while focus is there (and no text field has it) its
//! keys send their messages, ahead of the page's own `keys`.
//!
//! A host view in it is drawn in a layer over the app, outside the stack's
//! element: the layer wraps the view in a focus of its own, which the stack
//! counts as inside it.

use {
    super::{Cx, Painter, Place},
    gpui_kit::{App, FocusHandle, Window, WindowId},
    moonframe_core::Keys,
    std::{cell::RefCell, collections::HashMap},
};

/// Each window's kept focus, by place, with the frame that last drew it.
type Handles = HashMap<Option<WindowId>, HashMap<String, (FocusHandle, u64)>>;

thread_local! {
    /// Each stack's focus, and each host view's in one, by its window and
    /// place, so it stays the same handle from frame to frame while drawn.
    static HANDLES: RefCell<Handles> = RefCell::new(HashMap::new());
}

/// A stack with keys a frame drew: innermost first, as children are drawn
/// before the stack around them.
#[derive(Clone)]
pub(crate) struct FocusKeys {
    pub(crate) focus: FocusHandle,
    /// The focus of each host view drawn in it in a layer: focus there is
    /// focus inside the stack.
    pub(crate) views: Vec<FocusHandle>,
    /// The mod that drew it, whose `update` gets the messages.
    pub(crate) owner: String,
    pub(crate) keys: Keys,
    /// The paint priority of the layer it is drawn in: 0 for the window,
    /// an overlay's inside one, whose backdrop keeps keys from those below.
    pub(crate) layer: usize,
}

impl FocusKeys {
    /// Whether focus is inside the stack, or in a host view drawn in it.
    pub(crate) fn holds_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
            || self
                .views
                .iter()
                .any(|view| view.contains_focused(window, cx))
    }
}

/// A frame of `window` is drawn: the handles it no longer drew go (rows
/// scrolled away in a list, a closed tab).
pub(crate) fn drawn(window: WindowId) {
    let frame = super::viewer::frame(window);
    HANDLES.with_borrow_mut(|handles| keep(handles, Some(window), frame));
}

/// Windows other than `open` closed: their handles go.
pub(crate) fn closed(open: &[WindowId]) {
    HANDLES.with_borrow_mut(|handles| {
        handles.retain(|window, _| window.is_none_or(|window| open.contains(&window)));
    });
}

fn keep(handles: &mut Handles, window: Option<WindowId>, frame: u64) {
    if let Some(places) = handles.get_mut(&window) {
        places.retain(|_, (_, drawn)| *drawn == frame);
    }
}

/// The focus kept for `path` in `window`, noted as drawn in `frame`.
pub(super) fn kept(window: Option<WindowId>, path: &str, frame: u64, cx: Cx) -> FocusHandle {
    HANDLES.with_borrow_mut(|handles| {
        let places = handles.entry(window).or_default();
        if let Some((focus, drawn)) = places.get_mut(path) {
            *drawn = frame;
            return focus.clone();
        }
        let focus = cx.focus_handle();
        places.insert(path.to_owned(), (focus.clone(), frame));
        focus
    })
}

impl Painter<'_, '_> {
    /// The focus the stack at `place` tracks, kept with its `keys` and the
    /// focus of the host `views` drawn in it for the keystrokes of this
    /// window; none for a stack without keys, or in a preview.
    pub(super) fn focus_keys(
        &mut self,
        keys: &Keys,
        views: Vec<FocusHandle>,
        place: &Place<'_>,
        cx: Cx,
    ) -> Option<FocusHandle> {
        if keys.is_empty() || self.preview {
            return None;
        }
        let focus = kept(self.window, &place.path, self.frame, cx);
        self.focus_keys.push(FocusKeys {
            focus: focus.clone(),
            views,
            owner: place.owner.to_owned(),
            keys: keys.clone(),
            layer: self.layer,
        });
        Some(focus)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{Handles, keep},
        gpui_kit::TestAppContext,
        std::collections::HashMap,
    };

    #[gpui_kit::test]
    fn a_stack_no_longer_drawn_lets_go_of_its_focus(cx: &mut TestAppContext) {
        let mut handles: Handles = HashMap::new();
        let focus = cx.update(|cx| cx.focus_handle());
        let places = handles.entry(None).or_default();
        places.insert("row[1]".to_owned(), (focus.clone(), 2));
        places.insert("row[2]".to_owned(), (focus, 1));
        keep(&mut handles, None, 2);
        let places = &handles[&None];
        assert!(places.contains_key("row[1]") && !places.contains_key("row[2]"));
    }
}
