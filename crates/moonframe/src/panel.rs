//! Panel mode: the app is a menu bar app. It has no Dock icon, and its main
//! window is a panel under one mod's menu bar item (`app:status-item`). A
//! click on the item shows the panel under it; a click anywhere else hides
//! it; closing, minimizing and Cmd-W hide it too, so the app does not quit
//! with its window. Quit stays a command of its own.
//!
//! An app asks with [`Moonframe::panel`](crate::Moonframe::panel). Only macOS
//! has it (and only with the `status-item` feature, which draws the item);
//! elsewhere the window stays a normal one.
//!
//! The item keeps its menu: a right click or a Ctrl-click opens it, as before.
//!
//! The system reopening the running app shows the panel too: a click on a
//! Dock tile kept for it, a double-click in Finder, Spotlight, or `open`.
//!
//! While the mod shows no item (a wrong name, an icon that does not load, a
//! reload without it), nothing could open the panel: the app has its Dock
//! icon back and the window is a normal one, until the item comes back.

#[cfg(all(feature = "status-item", target_os = "macos"))]
mod macos;

use {
    gpui_kit::{App, Application, Window},
    moonframe_core::Command,
};

/// What panel mode does with a window command, in place of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Hide,
    Show,
}

/// What panel mode does with `command`, sent from `window`; `None` leaves
/// it to the window.
pub(crate) fn route(command: Command, window: &Window) -> Option<Route> {
    route_for(command, on(), is(window))
}

/// The panel hides on its own Minimize and Close Window, and any window's
/// Activate and New Window show it. Another window keeps its Minimize and
/// Close Window.
fn route_for(command: Command, on: bool, from_panel: bool) -> Option<Route> {
    match command {
        _ if !on => None,
        Command::Minimize | Command::CloseWindow if from_panel => Some(Route::Hide),
        Command::Activate | Command::NewWindow => Some(Route::Show),
        _ => None,
    }
}

/// Turns panel mode on for the mod `owner`'s item. Before the window opens.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn start(owner: &str, cx: &mut App) {
    macos::start(owner, cx);
}

/// Whether the app runs as a panel: asked for, and its item is there.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn on() -> bool {
    macos::on()
}

/// Whether `window` is the panel, and the app runs as one.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn is(window: &Window) -> bool {
    macos::is(window)
}

/// Makes `window`, the main window, the panel: hidden, and hiding itself when
/// it loses focus. Without its AppKit window, it stays a normal one.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn attach(window: &Window, cx: &mut App) {
    macos::attach(window, cx);
}

/// Shows the panel under its item, in front, and gives it the keys.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn show() {
    macos::show();
}

/// Hides the panel. The window stays open.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn hide() {
    macos::hide();
}

/// Shows the panel when the system reopens the running app. A hidden panel is
/// ordered out, not minimized, so AppKit has no window to bring back by
/// itself. Call it before the application runs; it does nothing while panel
/// mode is off.
pub(crate) fn listen(application: &Application) {
    application.on_reopen(|_| {
        if on() {
            show();
        }
    });
}

/// Once the mods' requests ran: a normal window while the owner shows no
/// item, and a panel again once it does.
#[cfg(all(feature = "status-item", target_os = "macos"))]
pub(crate) fn check(cx: &mut App) {
    macos::check(cx);
}

// Elsewhere there is no panel, and the window is a normal one.
#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn start(_: &str, _: &mut App) {}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn on() -> bool {
    false
}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn is(_: &Window) -> bool {
    false
}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn attach(_: &Window, _: &mut App) {}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn show() {}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn hide() {}

#[cfg(not(all(feature = "status-item", target_os = "macos")))]
pub(crate) fn check(_: &mut App) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_hides_on_its_own_minimize_and_close() {
        for command in [Command::Minimize, Command::CloseWindow] {
            assert_eq!(route_for(command, true, true), Some(Route::Hide));
        }
    }

    #[test]
    fn another_window_keeps_its_minimize_and_close() {
        // A mod's own window: its title bar's buttons act on it.
        for command in [Command::Minimize, Command::CloseWindow] {
            assert_eq!(route_for(command, true, false), None);
        }
    }

    #[test]
    fn activate_and_new_window_show_the_panel_from_any_window() {
        for command in [Command::Activate, Command::NewWindow] {
            assert_eq!(route_for(command, true, true), Some(Route::Show));
            assert_eq!(route_for(command, true, false), Some(Route::Show));
        }
    }

    #[test]
    fn without_panel_mode_every_command_goes_to_the_window() {
        for command in [
            Command::Minimize,
            Command::CloseWindow,
            Command::Activate,
            Command::NewWindow,
            Command::Maximize,
        ] {
            assert_eq!(route_for(command, false, true), None);
        }
    }

    #[test]
    fn other_commands_go_to_the_window() {
        for command in [Command::Maximize, Command::Quit, Command::Reload] {
            assert_eq!(route_for(command, true, true), None);
        }
    }
}
