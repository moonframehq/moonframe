//! Keys an open overlay or popover takes (`keys = { escape = ... }`). They
//! come first, before a focused terminal and the app's key bindings, but only
//! for the topmost open one.

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

impl MoonframeApp {
    /// Offers `keystroke` to the topmost open overlay or popover.
    pub(crate) fn layer_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Taken {
        // Escape belongs to the inspector while it is picking or showing
        // its card, which sit above everything.
        if keystroke.key == "escape"
            && (self.inspector.picking || self.inspector.selected.is_some())
        {
            return Taken::Free;
        }
        // The highest priority; of equals, the later in the tree.
        let Some(top) = self
            .key_layers
            .iter()
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
}
