//! What the user selects in a document, noted for `app:text` as they let
//! go of a press that began in it, with the mod that drew it.

use {
    super::Cx,
    gpui_kit::{component::text::TextViewState, *},
    std::{cell::RefCell, collections::HashSet, rc::Rc},
};

/// The document's view state, left by the document as it renders.
pub(super) type Shown = Rc<RefCell<Option<WeakEntity<TextViewState>>>>;

thread_local! {
    /// The documents, by their view state, a press under way began on. A
    /// frame drawn mid-drag draws new layers, so it is kept apart from them.
    static PRESSED: RefCell<HashSet<EntityId>> = RefCell::default();
}

/// A layer over the document that notes its selection when a press that
/// began on it comes up, wherever the pointer then is.
pub(super) fn watch(shown: Shown, owner: &str, cx: Cx) -> impl IntoElement {
    let app = cx.entity().downgrade();
    let owner = owner.to_owned();
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, (), window, _| {
            let Some(state) = shown.borrow().clone() else {
                return;
            };
            let id = state.entity_id();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, _| {
                if phase.bubble() {
                    PRESSED.with_borrow_mut(|pressed| {
                        if bounds.contains(&event.position) {
                            pressed.insert(id);
                        } else {
                            pressed.remove(&id);
                        }
                    });
                }
            });
            let (app, owner) = (app.clone(), owner.clone());
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if !phase.bubble() || !PRESSED.with_borrow_mut(|pressed| pressed.remove(&id)) {
                    return;
                }
                let Some(state) = state.upgrade() else {
                    return;
                };
                // Once the release is done with: the view settles its
                // selection on it too.
                let (app, owner) = (app.clone(), owner.clone());
                cx.defer(move |cx| {
                    let text = state.read(cx).selected_text();
                    app.update(cx, |app, _| app.remember_selection(&owner, &text))
                        .ok();
                });
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}
