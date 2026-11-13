//! Picking an element for an agent or a mod, and what is done with the pick.

use {
    super::*,
    std::sync::atomic::{AtomicBool, Ordering},
};

impl MoonframeApp {
    /// Starts picking an element, or stops.
    pub(crate) fn toggle_inspect(&mut self, cx: &mut Context<Self>) {
        self.inspector.picking = !self.inspector.picking;
        self.inspector.unhover();
        if self.inspector.picking {
            // A new pick replaces the last: its outline and card go now,
            // not once something else is clicked.
            self.forget_selection();
        } else {
            // Stopped without a pick: the mod that asked hears so.
            self.answer_asker(None);
        }
        cx.notify();
    }

    /// Starts picking for the mod `from`, which hears the pick (or that
    /// there was none) on `app/inspect` once the card's choice is confirmed.
    pub(super) fn inspect_for(&mut self, from: &str, cx: &mut Context<Self>) {
        // A preview starts nothing.
        if from.starts_with(moonframe_core::DEMO) {
            return;
        }
        // The Developer tools hear every pick while they are open.
        if from == moonframe_core::DEVTOOLS {
            if !self.inspector.picking {
                self.toggle_inspect(cx);
            }
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
        self.forget_selection();
        self.inspector.asker = Some(from.to_owned());
        self.inspector.picking = true;
        self.inspector.unhover();
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
            let pick = inspect::pick(picked, &self.workspace, Some(&asker));
            fields.insert("element".to_owned(), Data::String(pick.element));
            fields.insert("mod".to_owned(), Data::String(pick.owner));
            fields.insert("at".to_owned(), Data::String(pick.at));
            fields.insert("report".to_owned(), Data::String(pick.report));
        }
        self.workspace
            .app_topic("inspect", &asker, Data::Map(fields));
    }

    /// Drops the current selection, on screen and where agents read it.
    fn forget_selection(&mut self) {
        self.inspector.leaf = None;
        if self.inspector.selected.take().is_some() {
            self.save_selection(None);
        }
    }

    /// Selects `picked` (or nothing) and saves it where agents read it.
    pub(crate) fn select(&mut self, picked: Option<Placed>, cx: &mut Context<Self>) {
        self.inspector.leaf = picked.as_ref().map(|picked| self.inspector.deepest(picked));
        self.inspector.picking = false;
        self.inspector.unhover();
        self.save_selection(
            picked
                .as_ref()
                .map(|picked| inspect::report(picked, &self.workspace)),
        );
        if let Some(picked) = &picked
            && self.inspector.asker.is_none()
        {
            self.tell_devtools(picked);
        }
        self.inspector.selected = picked;
        if self.inspector.selected.is_some() {
            self.reveal_selection();
        } else {
            // A mod waiting on the card hears that nothing was chosen.
            self.answer_asker(None);
        }
        cx.notify();
    }

    /// Hands the card's selection to the mod that asked for a pick, and
    /// closes the card.
    pub(crate) fn use_selection(&mut self, cx: &mut Context<Self>) {
        let picked = self.inspector.selected.take();
        self.inspector.leaf = None;
        self.answer_asker(picked.as_ref());
        cx.notify();
    }

    /// Keys while picking: ↑ targets the node around the one under the
    /// pointer (a cell's row, a row's list), ↓ steps back in, Enter picks
    /// the target. Enter on the card of a pick a mod asked for hands it
    /// over. Only the main window shows the picker; other windows keep
    /// their keys. Its own bindings wait until picking ends.
    pub(crate) fn picking_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = keystroke.modifiers;
        let confirming = self.inspector.asker.is_some() && self.inspector.selected.is_some();
        if !(self.inspector.picking || confirming)
            || !self.is_main(window)
            || modifiers.control
            || modifiers.alt
            || modifiers.platform
            || modifiers.function
        {
            return false;
        }
        if confirming {
            if keystroke.key != "enter" {
                return false;
            }
            self.use_selection(cx);
            return true;
        }
        let pointer = window.mouse_position();
        match keystroke.key.as_str() {
            "up" | "down" => {
                if self.inspector.step(pointer, keystroke.key == "up") {
                    cx.notify();
                }
            },
            "enter" => {
                if let Some(target) = self.inspector.hovered.clone() {
                    self.select(Some(target.placed), cx);
                }
            },
            _ => return false,
        }
        true
    }

    /// Selects instead the node drawn at `path`, one around or inside the
    /// selection, as its card's trail offers; a row scrolled out of the
    /// window since cannot be.
    pub(crate) fn select_path(&mut self, path: &str, cx: &mut Context<Self>) {
        match self.inspector.placed(path) {
            Some(placed) => self.select(Some(placed), cx),
            None => self.show_toast("Not on screen any more", cx),
        }
    }

    /// Saves the selection's report again when what it says has changed
    /// since: the mod's state, what it read from `app`, the element's text
    /// or place. An agent reading it later finds the window as it is.
    /// It is made again only once the window drew since, and never over a
    /// pick another window saved after this one's. Whether the selection
    /// changed, so its card is drawn again.
    pub(crate) fn refresh_selection(&mut self) -> bool {
        let frame = self.inspector.frame();
        if self.inspector.selected.is_none()
            || self.inspector.superseded
            || self.inspector.reported == Some(frame)
        {
            return false;
        }
        self.inspector.reported = Some(frame);
        // The element as last drawn, so Copy for agent and a mod's pick
        // hand over what the file says too.
        let Some(selected) = self
            .inspector
            .selected
            .as_ref()
            .and_then(|selected| self.inspector.placed(&selected.path))
            .or_else(|| self.inspector.selected.clone())
        else {
            return false;
        };
        let report = inspect::report(&selected, &self.workspace);
        self.inspector.selected = Some(selected);
        if self.inspector.saved.as_ref() == Some(&report) {
            return false;
        }
        // Another window picked since: its pick is the one agents read.
        if self.inspector.saved.is_some() && !holds(self.inspector.saved.as_deref()) {
            self.inspector.superseded = true;
            self.inspector.saved = None;
            return false;
        }
        self.inspector.saved = write_selection(Some(&report)).then_some(report);
        true
    }

    /// Saves `report` where agents read the selection, a new pick of this
    /// window's, or removes what is there when there is none and it is
    /// still this window's. A save that fails is not remembered, so the
    /// next refresh tries again.
    fn save_selection(&mut self, report: Option<String>) {
        let superseded = std::mem::take(&mut self.inspector.superseded);
        self.inspector.reported = None;
        if report.is_none() && (superseded || !holds(self.inspector.saved.as_deref())) {
            self.inspector.saved = None;
            return;
        }
        self.inspector.saved = write_selection(report.as_deref())
            .then_some(report)
            .flatten();
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
    pub(crate) fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.inspector.picking {
            self.toggle_inspect(cx);
        } else if self.inspector.selected.is_some() {
            self.select(None, cx);
        }
    }
}

/// Whether the last save of the selection failed.
static FAILING: AtomicBool = AtomicBool::new(false);

/// Whether it saved.
fn write_selection(report: Option<&str>) -> bool {
    let path = selection_file();
    let saved = match report {
        Some(report) => path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| write_private(&path, report)),
        None => match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        },
    };
    match saved {
        Ok(()) => {
            FAILING.store(false, Ordering::Relaxed);
            // Written again as the window changes: not worth a line each time.
            if report.is_some() {
                tracing::debug!("selection saved to {}", path.display());
            }
            true
        },
        Err(error) => {
            // Tried again at each refresh: told once until it works again.
            if !FAILING.swap(true, Ordering::Relaxed) {
                tracing::warn!("cannot save the selection to {}: {error}", path.display());
            }
            false
        },
    }
}

/// Where agents read the selection. Each test thread keeps its own, so
/// tests drawn at once do not take each other's picks for another window's.
pub(crate) fn selection_file() -> PathBuf {
    #[cfg(test)]
    #[allow(clippy::expect_used)]
    {
        thread_local! {
            static FILE: tempfile::TempDir = tempfile::tempdir().expect("a folder for the selection");
        }
        FILE.with(|folder| folder.path().join("selection.md"))
    }
    #[cfg(not(test))]
    paths::selection()
}

/// Whether the selection file holds `saved`, what this window last wrote
/// there: no other window saved its own pick, or removed it, since. With
/// nothing saved, a file left over is not this window's to remove.
fn holds(saved: Option<&str>) -> bool {
    saved.is_some_and(|saved| {
        std::fs::read_to_string(selection_file()).is_ok_and(|text| text == saved)
    })
}

/// Writes `text` to `path` readable by this user only: the report holds a
/// mod's state, which can be anything the user typed. It goes to a file
/// beside it first, then takes its place, so a reader never finds it
/// half written.
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{}.tmp", std::process::id()));
    let temporary = path.with_file_name(name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options.open(&temporary).and_then(|mut file| {
        // One left over from before kept its old mode: tighten it too.
        #[cfg(unix)]
        file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
        file.write_all(text.as_bytes())
    });
    let moved = written.and_then(|()| std::fs::rename(&temporary, path));
    if moved.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    moved
}
