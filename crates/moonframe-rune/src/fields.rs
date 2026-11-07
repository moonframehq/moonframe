//! What text fields tell mods and take from them beyond typing: `select`,
//! sent as the selection moves, and `app:edit`, a change around it.

use {
    crate::Ui,
    moonframe_core::{Data, Edit, Error, Input},
};

/// Who hears a field's message, and what it is.
pub(crate) type Hearing = (String, String, Data);

impl Ui {
    /// Sends `select` for each field whose selection moved since the last look.
    pub(crate) fn tell_selections(&mut self) -> bool {
        let mut told = Vec::new();
        for field in self.fields.get_mut().values_mut() {
            let now = (field.edit.range(), field.edit.focus);
            if field.selection == now {
                continue;
            }
            field.selection = now;
            if let Some((owner, to, msg)) = &field.select {
                let ((start, end), caret) = now;
                let message = Input::selection_message(msg, &field.edit.text, caret, end - start);
                told.push((owner.clone(), to.clone(), message));
            }
        }
        let any = !told.is_empty();
        for (owner, to, message) in told {
            self.send(&owner, &to, message);
        }
        any
    }

    /// Carries out `app:edit` from the mod `from` on its field `edit.field`:
    /// one undoable change, leaving selected what the edit does, with the
    /// keyboard there. The field sends its `change` as for any edit.
    pub(crate) fn apply_edit(&mut self, from: &str, edit: &Edit) {
        let key = format!("{from}:{}", edit.field);
        let Some(field) = self.fields.get_mut().get_mut(&key) else {
            let problem = format!(
                "`app:edit`: no field with `id = \"{}\"` drawn by `{from}`",
                edit.field
            );
            self.workspace.refuse(from, Error::new(problem));
            return;
        };
        let (start, end) = field.edit.range();
        if !edit.applies(&field.edit.text, start..end) {
            return;
        }
        let splice = edit.splice(&field.edit.text, start..end);
        field
            .edit
            .set_selection(splice.range.start, splice.range.end);
        field.edit.insert(&splice.text);
        field
            .edit
            .set_selection(splice.select.start, splice.select.end);
        field.edit.focused = true;
        let said = field.change.clone().map(|(owner, to, msg)| {
            let caret = field.edit.focus;
            let message = Input::message_at(&msg, &field.edit.text, caret);
            (owner, to, message)
        });
        if let Some((owner, to, message)) = said {
            self.send(&owner, &to, message);
        }
    }
}
