//! Keys an open overlay or popover takes (`keys = { escape = ... }`). They
//! come first, before a focused terminal and the app's key bindings, but only
//! for the topmost open one in the window the key was typed in.

use {
    crate::app::MoonframeApp,
    gpui_kit::*,
    moonframe_core::{Action, Pressed},
};

/// What the topmost layer did with a keystroke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Taken {
    /// It sent a message; nothing else sees the key.
    Sent,
    /// Not one of its keys, but it blocks the window behind (a backdrop):
    /// a terminal there must not get it; key bindings still may.
    Blocked,
    /// Nothing open takes it.
    Free,
}

/// `secondary` means ⌘ here, Ctrl elsewhere.
const MAC: bool = cfg!(target_os = "macos");

/// Shared key routing also runs in builds without a terminal: text fields,
/// overlays and mod shortcuts must keep working there.
pub(crate) fn intercept(cx: &mut Context<MoonframeApp>) -> Subscription {
    let app = cx.entity().downgrade();
    cx.intercept_keystrokes(move |event, window, cx| {
        if event.keystroke.key == "escape" && cx.stop_active_drag(window) {
            cx.stop_propagation();
            return;
        }
        let taken = app
            .update(cx, |app, cx| {
                match app.layer_keystroke(&event.keystroke, window, cx) {
                    Taken::Sent => true,
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

impl MoonframeApp {
    /// Offers `keystroke` to the topmost open overlay or popover.
    pub(crate) fn layer_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Taken {
        // While picking, 1 to 5 choose how it measures.
        if self.measure_keystroke(keystroke, cx) {
            return Taken::Sent;
        }
        // Escape belongs to the inspector while it is picking or showing
        // its card, which sit above everything: before a focused terminal
        // or field, which would keep it from the app's key bindings.
        if keystroke.key == "escape"
            && (self.inspector.picking || self.inspector.selected.is_some())
        {
            self.dismiss(cx);
            return Taken::Sent;
        }
        // The highest priority in this window; of equals, the later in the
        // tree.
        let Some(top) = self
            .key_layers
            .get(&window.window_handle().window_id())
            .into_iter()
            .flatten()
            .max_by_key(|layer| layer.priority)
            .cloned()
        else {
            return Taken::Free;
        };
        let modifiers = keystroke.modifiers;
        let pressed = Pressed {
            key: &keystroke.key,
            text: keystroke.key_char.as_deref(),
            ctrl: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
            cmd: modifiers.platform,
        };
        match top.keys.message(top.dismiss.as_ref(), &pressed, MAC) {
            Some(msg) => {
                let action = Action::Send {
                    to: top.to.clone(),
                    msg: msg.clone(),
                };
                self.dispatch(&top.owner, &action, window, cx);
                Taken::Sent
            },
            None if top.modal => Taken::Blocked,
            None => Taken::Free,
        }
    }

    /// Offers `keystroke` to the `keys` mods declare (`mod { keys = ... }`),
    /// once nothing nearer took it: every mod, and every mounted copy of
    /// one, whose keys take it hears it. A focused text field keeps all
    /// its keys (copy, select all, undo), so mods hear none while one has
    /// focus; and a shell menu item's shortcut stays the menu's.
    pub(crate) fn mod_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.inputs.focused(window, cx).is_some() {
            return false;
        }
        let modifiers = keystroke.modifiers;
        let pressed = Pressed {
            key: &keystroke.key,
            text: keystroke.key_char.as_deref(),
            ctrl: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
            cmd: modifiers.platform,
        };
        // The shell's menus come first: their shortcuts are GPUI bindings,
        // which see the key only after this.
        let menu_takes = self.workspace.menus().iter().any(|menu| {
            menu.items.iter().any(|entry| match entry {
                moonframe_core::MenuEntry::Item(item) => item
                    .shortcut
                    .as_ref()
                    .is_some_and(|chord| chord.matches(&pressed, MAC)),
                moonframe_core::MenuEntry::Separator => false,
            })
        });
        if menu_takes {
            return false;
        }
        let hearers: Vec<(String, moonframe_core::Data)> = self
            .workspace
            .mod_keys()
            .filter_map(|(owner, keys)| {
                let msg = keys.message(None, &pressed, MAC)?;
                Some((owner.to_owned(), msg.clone()))
            })
            .collect();
        for (owner, msg) in &hearers {
            let action = Action::Send {
                to: None,
                msg: msg.clone(),
            };
            self.dispatch(owner, &action, window, cx);
        }
        !hearers.is_empty()
    }
}
