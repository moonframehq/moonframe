//! `app:text`: the text a mod or a region draws, and the most recent text
//! the user selected in this app's windows, back to the mod that asked.
//! The selection is noted as the user lets go of the pointer, so it stays
//! known after a click into a field clears it on screen.

use {
    super::*,
    moonframe_core::{Selected, TextRequest},
    std::{cell::RefCell, collections::HashMap},
};

thread_local! {
    /// The last selection made in each app (each main window and the mod
    /// windows it opened), by the app's entity.
    static SELECTED: RefCell<HashMap<EntityId, Selected>> = RefCell::default();
}

/// Notes that the user selected `text` in what `owner` draws, in `app`.
pub(crate) fn remember(app: EntityId, owner: &str, text: String) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    SELECTED.with_borrow_mut(|selected| {
        selected.insert(app, Selected {
            text: text.to_owned(),
            owner: owner.to_owned(),
        })
    });
}

impl MoonframeApp {
    /// Answers the mod `from`'s `app:text`.
    pub(super) fn text_request(
        &mut self,
        from: &str,
        request: TextRequest,
        cx: &mut Context<Self>,
    ) {
        let text = request
            .of
            .as_ref()
            .and_then(|of| self.workspace.drawn_text(of));
        let id = cx.entity_id();
        let selected = SELECTED.with_borrow(|selected| selected.get(&id).cloned());
        self.deliver(from, request.answer(text, selected), cx);
    }
}
