//! Picking an element for an agent or a mod, and what is done with the pick.

use super::*;

impl MoonframeApp {
    /// Starts picking an element, or stops.
    pub(crate) fn toggle_inspect(&mut self, cx: &mut Context<Self>) {
        self.inspector.picking = !self.inspector.picking;
        self.inspector.hovered = None;
        if !self.inspector.picking {
            // Stopped without a pick: the mod that asked hears so.
            self.answer_asker(None);
        }
        cx.notify();
    }

    /// Starts picking for the mod `from`, which hears the pick (or that
    /// there was none) on `app/inspect` instead of a card opening.
    pub(super) fn inspect_for(&mut self, from: &str, cx: &mut Context<Self>) {
        // A preview starts nothing.
        if from.starts_with(moonframe_core::DEMO) {
            return;
        }
        // A pick another mod started ends for it first, unanswered.
        if self
            .inspector
            .asker
            .as_deref()
            .is_some_and(|asker| asker != from)
        {
            self.answer_asker(None);
        }
        self.inspector.selected = None;
        self.inspector.asker = Some(from.to_owned());
        self.inspector.picking = true;
        self.inspector.hovered = None;
        // It hears that picking started, to get out of the way (a dialog).
        let started = std::collections::BTreeMap::from([
            ("asker".to_owned(), Data::String(from.to_owned())),
            ("picking".to_owned(), Data::Bool(true)),
        ]);
        self.workspace
            .app_topic("inspect", from, Data::Map(started));
        cx.notify();
    }

    /// Tells the mod that started the pick what was picked, if any:
    /// `{ asker, picking = false, picked, element?, mod?, at?, report? }`
    /// on `app/inspect`.
    fn answer_asker(&mut self, picked: Option<&Placed>) {
        let Some(asker) = self.inspector.asker.take() else {
            return;
        };
        let mut fields = std::collections::BTreeMap::from([
            ("asker".to_owned(), Data::String(asker.clone())),
            ("picking".to_owned(), Data::Bool(false)),
            ("picked".to_owned(), Data::Bool(picked.is_some())),
        ]);
        if let Some(picked) = picked {
            let pick = inspect::pick(picked, &self.workspace);
            fields.insert("element".to_owned(), Data::String(pick.element));
            fields.insert("mod".to_owned(), Data::String(pick.owner));
            fields.insert("at".to_owned(), Data::String(pick.at));
            fields.insert("report".to_owned(), Data::String(pick.report));
        }
        self.workspace
            .app_topic("inspect", &asker, Data::Map(fields));
    }

    /// Selects `picked` (or nothing) and saves it where agents read it.
    pub(crate) fn select(&mut self, picked: Option<Placed>, cx: &mut Context<Self>) {
        self.inspector.picking = false;
        self.inspector.hovered = None;
        let path = paths::selection();
        let saved = match &picked {
            Some(picked) => path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, inspect::report(picked, &self.workspace))),
            None => match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            },
        };
        match saved {
            Ok(()) if picked.is_some() => tracing::info!("selection saved to {}", path.display()),
            Ok(()) => {},
            Err(error) => {
                tracing::warn!("cannot save the selection to {}: {error}", path.display())
            },
        }
        if self.inspector.asker.is_some() {
            // Picked for a mod: it shows the pick itself, so no card.
            self.answer_asker(picked.as_ref());
            self.inspector.selected = None;
        } else {
            self.inspector.selected = picked;
        }
        cx.notify();
    }

    /// Copies the selection for an agent and closes its card. The saved
    /// file stays, so an agent told "here" still finds it.
    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(selected) = self.inspector.selected.take() {
            let report = inspect::report(&selected, &self.workspace);
            self.copy(report, cx);
        }
        cx.notify();
    }

    pub(crate) fn copy(&mut self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_toast("Copied", cx);
    }

    /// Esc: stops picking, else clears the selection.
    pub(super) fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.inspector.picking {
            self.toggle_inspect(cx);
        } else if self.inspector.selected.is_some() {
            self.select(None, cx);
        }
    }
}
