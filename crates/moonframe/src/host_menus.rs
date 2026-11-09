//! A host app's own menu bar. An app that keeps its own menus (one whose
//! window Moonframe lays out, but whose commands are its own) hands Moonframe
//! a builder; the bar is then the host's, with the app's shell menus and
//! Moonframe's Developer menu added before Window and Help. Moonframe's own
//! app, appearance and theme items are left out, as the host has its own.

use {
    crate::actions::{developer_menu, shell_menu},
    gpui_kit::{App, Menu},
    moonframe_core::AppMenu,
    std::{cell::RefCell, rc::Rc},
};

/// Builds the host's menus from the app's current state.
pub(crate) type Build = Rc<dyn Fn(&App) -> Vec<Menu>>;

thread_local! {
    static HOST: RefCell<Option<Build>> = const { RefCell::new(None) };
    /// The shell menus the bar was last built with, for [`refresh`].
    static SHELL: RefCell<Vec<AppMenu>> = const { RefCell::new(Vec::new()) };
}

/// Makes `build` the source of the bar from now on.
pub(crate) fn set(build: Build) {
    HOST.with_borrow_mut(|host| *host = Some(build));
}

/// Installs the bar for `shell`: Moonframe's own, or the host's.
pub(crate) fn install(shell: &[AppMenu], cx: &mut App) {
    SHELL.with_borrow_mut(|kept| kept.clone_from(&shell.to_vec()));
    let host = HOST.with_borrow(Clone::clone);
    let bar = match host {
        Some(build) => merge(build(cx), shell),
        None => crate::actions::menus(shell),
    };
    cx.set_menus(bar);
}

/// Builds the bar again, as the host asks when what its menus show changed
/// (a checked item, a language).
pub fn refresh_menus(cx: &mut App) {
    let shell = SHELL.with_borrow(Clone::clone);
    install(&shell, cx);
}

/// The host's bar with the shell's menus and the Developer menu before
/// Window and Help, as macOS orders them.
fn merge(mut host: Vec<Menu>, shell: &[AppMenu]) -> Vec<Menu> {
    let at = host
        .iter()
        .position(|menu| matches!(menu.name.as_ref(), "Window" | "Help"))
        .unwrap_or(host.len());
    let added: Vec<Menu> = shell
        .iter()
        .map(shell_menu)
        .chain(developer_menu())
        .collect();
    host.splice(at..at, added);
    host
}

#[cfg(test)]
mod tests {
    use {super::*, gpui_kit::MenuItem};

    fn names(bar: &[Menu]) -> Vec<String> {
        bar.iter().map(|menu| menu.name.to_string()).collect()
    }

    #[test]
    fn the_hosts_menus_lead_and_window_and_help_stay_last() {
        let host = vec![
            Menu::new("Sheprd").items([MenuItem::separator()]),
            Menu::new("Edit").items([MenuItem::separator()]),
            Menu::new("Window").items([MenuItem::separator()]),
            Menu::new("Help").items([MenuItem::separator()]),
        ];
        let bar = names(&merge(host, &[]));
        assert_eq!(&bar[..2], ["Sheprd", "Edit"]);
        assert_eq!(&bar[bar.len() - 2..], ["Window", "Help"]);
        // Moonframe's own app menu is not added beside the host's.
        assert!(!bar.iter().any(|name| name == &crate::title()));
    }
}
