//! A row's or column's own `keys`, taken while the keyboard is inside it:
//! a click anywhere in it gives it focus, as does a control in it that
//! takes focus, and while focus is there (and no text field has it) its
//! keys send their messages, ahead of the page's own `keys`.

use {
    super::{Cx, Painter, Place},
    gpui_kit::FocusHandle,
    moonframe_core::Keys,
    std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        time::{Duration, Instant},
    },
};

/// How long a stack not drawn keeps its focus: its window closed, or it
/// scrolled away in a list.
const FORGET: Duration = Duration::from_secs(60);

thread_local! {
    /// Each stack's focus, by its window and place, so it stays the same
    /// handle from frame to frame, and when it was last drawn.
    static HANDLES: RefCell<HashMap<String, (FocusHandle, Instant)>> =
        RefCell::new(HashMap::new());
    /// When handles not drawn for a while were last let go of.
    static PRUNED: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Lets go of the handles of stacks not drawn for [`FORGET`], at most once
/// a second.
fn prune(handles: &mut HashMap<String, (FocusHandle, Instant)>, now: Instant) {
    if PRUNED
        .get()
        .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_secs(1))
    {
        return;
    }
    PRUNED.set(Some(now));
    handles.retain(|_, (_, seen)| now.saturating_duration_since(*seen) < FORGET);
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
        let now = Instant::now();
        let focus = HANDLES.with_borrow_mut(|handles| {
            prune(handles, now);
            let (focus, seen) = handles
                .entry(key)
                .or_insert_with(|| (cx.focus_handle(), now));
            *seen = now;
            focus.clone()
        });
        self.focus_keys.push(FocusKeys {
            focus: focus.clone(),
            owner: place.owner.to_owned(),
            keys: keys.clone(),
        });
        Some(focus)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{FORGET, prune},
        gpui_kit::TestAppContext,
        std::{collections::HashMap, time::Duration},
    };

    #[gpui_kit::test]
    fn a_stack_not_drawn_for_long_lets_go_of_its_focus(cx: &mut TestAppContext) {
        let now = std::time::Instant::now();
        let mut handles = HashMap::new();
        cx.update(|cx| {
            handles.insert("drawn".to_owned(), (cx.focus_handle(), now));
            handles.insert("gone".to_owned(), (cx.focus_handle(), now));
        });
        let later = now + FORGET + Duration::from_secs(1);
        if let Some((_, seen)) = handles.get_mut("drawn") {
            *seen = later;
        }
        prune(&mut handles, later);
        assert!(handles.contains_key("drawn") && !handles.contains_key("gone"));
    }
}
