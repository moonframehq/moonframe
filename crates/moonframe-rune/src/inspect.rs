//! Inspect Element, for a mod that asks (`command = "inspect"`): the next
//! click picks what lies under it, and the mod hears on `app/inspect` what
//! was picked. While picking, a layer over the window takes the pointer; it
//! goes before the pick, so the pick finds what is under it.

use {
    crate::Ui,
    moonframe_core::{DEMO, Data},
    rune::{Picked, Tx},
    std::{collections::BTreeMap, fmt::Write as _},
};

/// What a node records of itself while picking, by its element's key.
#[derive(Clone)]
pub(crate) struct Crumb {
    pub(crate) element: String,
    pub(crate) owner: String,
    pub(crate) line: Option<u32>,
}

/// The pick under way.
#[derive(Default)]
pub(crate) struct Inspecting {
    /// The mod that started it, while the layer is up.
    pub(crate) asker: Option<String>,
    /// Where the click landed, and the frames left until the layer is gone.
    click: Option<((f32, f32), u8)>,
}

impl Inspecting {
    /// Whether the click has come, so the layer is to go.
    pub(crate) fn clicked(&self) -> bool {
        self.click.is_some()
    }
}

impl Ui {
    /// Starts picking for the mod `from`, as `inspect_for` does on GPUI.
    pub(crate) fn inspect_for(&mut self, from: &str) {
        // A preview starts nothing.
        if from.starts_with(DEMO) {
            return;
        }
        self.inspecting.asker = Some(from.to_owned());
        let started = BTreeMap::from([
            ("asker".to_owned(), Data::String(from.to_owned())),
            ("picking".to_owned(), Data::Bool(true)),
        ]);
        self.workspace_mut()
            .app_topic("inspect", from, Data::Map(started));
    }

    /// The click that ends the pick, at `(x, y)` in the window.
    pub(crate) fn pick_at(&mut self, at: (f32, f32)) {
        self.inspecting.click = Some((at, 2));
        if let Some(wake) = &self.waker {
            wake();
        }
    }

    /// Each frame: asks what is under the click, once the layer has gone.
    pub(crate) fn settle_pick(&mut self, tx: &mut Tx) {
        match self.inspecting.click {
            Some(((x, y), 0)) => {
                self.inspecting.click = None;
                let any = rune::Pick::new().transparent(true);
                tx.element_at_with(
                    x,
                    y,
                    any,
                    |ui: &mut Ui, picked: Option<Picked>, _: &mut Tx| {
                        ui.answer_pick(picked.as_ref());
                    },
                );
            },
            Some((at, frames)) => {
                self.inspecting.click = Some((at, frames - 1));
                if let Some(wake) = &self.waker {
                    wake();
                }
            },
            None => {},
        }
    }

    /// Tells the mod that asked what was picked, if anything: `{ asker,
    /// picking = false, picked, element?, mod?, at?, report? }`.
    pub(crate) fn answer_pick(&mut self, picked: Option<&Picked>) {
        let Some(asker) = self.inspecting.asker.take() else {
            return;
        };
        let crumb = picked.and_then(|picked| self.crumb_of(picked));
        let mut fields = BTreeMap::from([
            ("asker".to_owned(), Data::String(asker.clone())),
            ("picking".to_owned(), Data::Bool(false)),
            ("picked".to_owned(), Data::Bool(crumb.is_some())),
        ]);
        if let Some(crumb) = crumb {
            let source = self
                .workspace()
                .slot(&crumb.owner)
                .and_then(|slot| slot.source())
                .map(ToOwned::to_owned);
            let file =
                source.map_or_else(|| crumb.owner.clone(), |path| path.display().to_string());
            let at = match crumb.line {
                Some(line) => format!("{file}:{line}"),
                None => file,
            };
            let mut report = String::from("Selected in Moonframe (View > Inspect Element):\n");
            let _ = writeln!(report, "- element: {}", crumb.element);
            let _ = writeln!(report, "- mod: {}, drawn by {at}", crumb.owner);
            if let Some(context) = self.workspace.view_context_for(&crumb.owner, Some(&asker)) {
                report.push_str(&context.report(&crumb.owner));
            }
            fields.insert("element".to_owned(), Data::String(crumb.element));
            fields.insert("mod".to_owned(), Data::String(crumb.owner));
            fields.insert("at".to_owned(), Data::String(at));
            fields.insert("report".to_owned(), Data::String(report));
        }
        self.workspace_mut()
            .app_topic("inspect", &asker, Data::Map(fields));
    }

    /// The node `picked` drew, or the nearest around it that keeps a crumb.
    fn crumb_of(&self, picked: &Picked) -> Option<Crumb> {
        let crumbs = self.crumbs.borrow();
        if let Some(found) = picked.key().and_then(|key| crumbs.get(key)) {
            return Some(found.clone());
        }
        let mut path = picked.path();
        loop {
            if let Some(found) = crumbs.get(path) {
                return Some(found.clone());
            }
            path = &path[..path.rfind('/')?];
        }
    }
}
