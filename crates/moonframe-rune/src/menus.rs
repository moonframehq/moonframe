//! The menu bar. Rune's items take a plain `fn`, so each is a numbered slot
//! that reads what it does from the shell's menus as they stand now.

use {
    crate::Ui,
    moonframe_core::{Action, MenuEntry, SHELL},
    rune::{AppHandle, MenuItem, Tx},
};

/// The most items the shell's menus can have.
const SLOTS: usize = 64;

/// A menu item, in whichever window is in front: a mod's window hands the
/// main one what it does not do itself (see `Ui::act`).
fn fire<const SLOT: usize>(ui: &mut Ui, tx: &mut Tx) {
    let action = ui.shared.menu_actions.borrow().get(SLOT).cloned();
    if let Some(action) = action {
        ui.act(SHELL, &action, tx);
    }
}

macro_rules! slots {
    ($($slot:literal)*) => {
        [$(fire::<$slot> as fn(&mut Ui, &mut Tx)),*]
    };
}

const FIRE: [fn(&mut Ui, &mut Tx); SLOTS] = slots![
    0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
    32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63
];

impl Ui {
    /// The shell's menus as Rune's items, keeping what each does.
    fn menu_items(&mut self) -> Vec<MenuItem<Ui>> {
        let mut actions: Vec<Action> = Vec::new();
        let workspace = self.workspace();
        let title = workspace
            .about()
            .name
            .unwrap_or_else(|| moonframe_packages::identity().title.clone());
        // First in the app menu, where the system's own About was.
        let mut items = vec![MenuItem::new(format!("About {title}"), show_about).id("about")];
        for menu in workspace.menus() {
            for entry in &menu.items {
                match entry {
                    MenuEntry::Separator => {
                        items.push(MenuItem::separator().menu(menu.title.clone()));
                    },
                    MenuEntry::Item(item) => {
                        let Some(slot) = FIRE.get(actions.len()) else {
                            tracing::warn!("the menus have more than {SLOTS} items");
                            continue;
                        };
                        items.push(
                            MenuItem::new(item.label.clone(), *slot)
                                .id(format!("{}/{}", menu.title, item.label))
                                .menu(menu.title.clone()),
                        );
                        actions.push(item.action.clone());
                    },
                }
            }
        }
        drop(workspace);
        *self.shared.menu_actions.borrow_mut() = actions;
        items
    }

    /// Puts the shell's menus in the menu bar, if they changed.
    pub(crate) fn sync_menus(&mut self) {
        let menus = self.workspace().menus().to_vec();
        if self.shown_menus.as_ref() == Some(&menus) {
            return;
        }
        let Some(app) = self.app.clone() else {
            return;
        };
        self.shown_menus = Some(menus);
        // Each window has its own items, all doing the same: the bar shows
        // the front window's.
        let mut handles = self.shared.handles();
        if handles.is_empty() {
            handles.push(app);
        }
        for handle in handles {
            let items = self.menu_items();
            set(&handle, items);
        }
    }
}

/// The About panel, with what draws this build.
fn show_about(ui: &mut Ui, _: &mut Tx) {
    let about = ui.workspace().about();
    let name = about
        .name
        .clone()
        .unwrap_or_else(|| moonframe_packages::identity().title.clone());
    crate::dock::about_panel(&name, &about, ui.icon.as_deref(), "Rune");
}

/// Takes the system's own About out of the app menu, which knows neither the
/// app's version nor what draws it; true once it is gone.
#[cfg(target_os = "macos")]
pub(crate) fn drop_system_about() -> bool {
    use {
        objc2::{MainThreadMarker, sel},
        objc2_app_kit::NSApplication,
    };
    let Some(main) = MainThreadMarker::new() else {
        return false;
    };
    let Some(menu) = NSApplication::sharedApplication(main)
        .mainMenu()
        .and_then(|bar| bar.itemAtIndex(0))
        .and_then(|item| item.submenu())
    else {
        return false;
    };
    for index in 0..menu.numberOfItems() {
        if let Some(item) = menu.itemAtIndex(index)
            && item.action() == Some(sel!(orderFrontStandardAboutPanel:))
        {
            menu.removeItem(&item);
            return true;
        }
    }
    false
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn drop_system_about() -> bool {
    true
}

fn set(app: &AppHandle<Ui>, items: Vec<MenuItem<Ui>>) {
    // Windows and Linux have no native bar to put them in.
    if let Err(problem) = app.set_menu(items) {
        tracing::debug!("no menu bar: {problem:?}");
    }
}
