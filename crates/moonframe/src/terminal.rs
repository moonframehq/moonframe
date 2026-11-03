//! Terminal sessions are optional; window-wide key routing is always present.

#[cfg(feature = "terminal")]
mod sessions;
#[cfg(feature = "terminal")]
pub(crate) use sessions::{Selection, Spot, TerminalView, Terminals};

use {
    crate::{app::MoonframeApp, keys::Taken},
    gpui_kit::*,
};

/// Hands keystrokes to the topmost open overlay or popover that takes them,
/// then to the focused text field (its `keys`), then to the focused terminal
/// (unless an overlay with a backdrop covers it), then to a shell menu item
/// whose shortcut is a single typing key (`j`), before key bindings see
/// them.
pub(crate) fn intercept_keys(cx: &mut Context<MoonframeApp>) -> Subscription {
    let app = cx.entity().downgrade();
    cx.intercept_keystrokes(move |event, window, cx| {
        // Cancel before overlays, fields or a secondary window dismiss.
        // Releasing the ghost also clears every entered drop target.
        if event.keystroke.key == "escape" && cx.stop_active_drag(window) {
            cx.stop_propagation();
            return;
        }
        let taken = app
            .update(cx, |app, cx| {
                match app.layer_keystroke(&event.keystroke, window, cx) {
                    Taken::Sent => true,
                    // A field inside the covering overlay keeps its keys.
                    Taken::Blocked => app.input_keystroke(&event.keystroke, window, cx),
                    Taken::Free => {
                        app.input_keystroke(&event.keystroke, window, cx)
                            || app.terminal_keystroke(&event.keystroke, window, cx)
                            || app.mod_keystroke(&event.keystroke, window, cx)
                            || app.menu_keystroke(&event.keystroke, window, cx)
                    },
                }
            })
            .unwrap_or(false);
        if taken {
            cx.stop_propagation();
        }
    })
}

#[cfg(not(feature = "terminal"))]
impl MoonframeApp {
    fn terminal_keystroke(&mut self, _: &Keystroke, _: &Window, _: &mut Context<Self>) -> bool {
        false
    }
}
