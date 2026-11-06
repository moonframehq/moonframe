//! A shortcut straight to picking an element for the feedback dialog, which
//! opens with the pick attached. `inspect_shortcut` in `settings.toml` names
//! the key, read at launch, as GPUI keeps a binding for the app's life.

use {super::*, crate::actions::keystroke, gpui_kit::KeyBinding, moonframe_core::Chord};

/// ⇧⌘X on macOS, Ctrl-Shift-X elsewhere.
const DEFAULT: &str = "secondary-shift-x";
/// The mod that hears the pick: `mods/feedback`.
const FEEDBACK: &str = "feedback";

/// The key `saved` names, the default without one, or none for `""`. A name
/// that is no key is reported and the default stands.
fn shortcut(saved: Option<&str>) -> Option<Chord> {
    let name = saved.unwrap_or(DEFAULT).trim();
    if name.is_empty() {
        return None;
    }
    Chord::parse(name)
        .inspect_err(|problem| tracing::warn!("{}: {problem}", settings::INSPECT_SHORTCUT))
        .or_else(|_| Chord::parse(DEFAULT))
        .ok()
}

/// Binds the key. It works outside Developer Mode too, like the dialog's
/// own Inspect button.
pub(super) fn bind(folders: &Folders, cx: &mut App) {
    let saved = folders
        .settings()
        .iter()
        .find_map(|path| settings::get(path, settings::INSPECT_SHORTCUT));
    if let Some(chord) = shortcut(saved.as_deref()) {
        // GPUI takes a key's name for the app's life; this is bound once.
        let name: &'static str = Box::leak(keystroke(&chord).into_boxed_str());
        cx.bind_keys([KeyBinding::new(name, QuickInspect, Some("MoonframeApp"))]);
    }
}

impl MoonframeApp {
    /// Starts picking for the dialog, or stops. An app without the feedback
    /// mod gets the inspector's own card, in Developer Mode only, as the
    /// View menu offers it.
    pub(crate) fn quick_inspect(&mut self, cx: &mut Context<Self>) {
        if self.inspector.picking {
            self.toggle_inspect(cx);
        } else if self.workspace.slot(FEEDBACK).is_some() {
            self.inspect_for(FEEDBACK, cx);
        } else if !crate::release() {
            self.toggle_inspect(cx);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {
        super::{DEFAULT, shortcut},
        moonframe_core::Chord,
    };

    #[test]
    fn the_shortcut_is_the_default_unless_the_settings_name_another() {
        let default = Chord::parse(DEFAULT).ok();
        assert_eq!(shortcut(None), default);
        assert_eq!(shortcut(Some("cmd-alt-p")), Chord::parse("cmd-alt-p").ok());
    }

    #[test]
    fn an_empty_shortcut_turns_it_off_and_a_bad_one_keeps_the_default() {
        assert_eq!(shortcut(Some("")), None);
        assert_eq!(shortcut(Some("hyper-x")), Chord::parse(DEFAULT).ok());
        assert_eq!(shortcut(Some("x")), Chord::parse(DEFAULT).ok());
    }
}
