//! `app:text`: the text a mod or a region draws, and the most recent text
//! the user selected in this app's windows, back to the mod that asked.
//! The selection is noted as the user lets go of the pointer, so it stays
//! known after a click into a field clears it on screen.

use {
    super::*,
    moonframe_core::{Selected, TextRequest},
};

impl MoonframeApp {
    /// Notes that the user selected `text` in what `owner` draws.
    #[cfg_attr(not(feature = "markdown"), allow(dead_code))]
    pub(crate) fn remember_selection(&mut self, owner: &str, text: &str) {
        let text = text.trim();
        if !text.is_empty() {
            self.selected = Some(Selected {
                text: text.to_owned(),
                owner: owner.to_owned(),
            });
        }
    }

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
        let answer = request.answer(text, self.selected.clone());
        self.deliver(from, answer, cx);
    }
}
