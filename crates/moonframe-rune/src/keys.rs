//! Keys Rune hands over, offered as `crates/moonframe/src/keys.rs` offers
//! GPUI's: the topmost open overlay or popover first, then the shell's menu
//! shortcuts, then each mod's `keys`.

use {
    crate::Ui,
    moonframe_core::{Action, Data, KeyChord, Keys, MenuEntry, Pressed},
    rune::{Key, KeyEvent, Tx},
};

/// `secondary` means ⌘ here, Ctrl elsewhere.
const MAC: bool = cfg!(target_os = "macos");

/// An open overlay or popover that takes keys, as last drawn.
pub(crate) struct Layer {
    pub(crate) owner: String,
    pub(crate) to: String,
    pub(crate) keys: Keys,
    pub(crate) dismiss: Option<Data>,
}

/// GPUI's name for `key`, which `Chord`s are written in.
fn named(key: &Key) -> Option<String> {
    Some(match key {
        Key::Text(text) if text == " " => "space".to_owned(),
        Key::Text(text) => text.to_lowercase(),
        Key::Command(letter) => letter.to_string(),
        Key::Function(number) => format!("f{number}"),
        Key::Enter => "enter".to_owned(),
        Key::Tab => "tab".to_owned(),
        Key::Escape => "escape".to_owned(),
        Key::Backspace => "backspace".to_owned(),
        Key::Delete => "delete".to_owned(),
        Key::Up => "up".to_owned(),
        Key::Down => "down".to_owned(),
        Key::Left => "left".to_owned(),
        Key::Right => "right".to_owned(),
        Key::Home => "home".to_owned(),
        Key::End => "end".to_owned(),
        Key::PageUp => "pageup".to_owned(),
        Key::PageDown => "pagedown".to_owned(),
        _ => return None,
    })
}

/// `event` as core's [`Pressed`], for the time `run` takes.
pub(crate) fn pressed<T>(event: &KeyEvent, run: impl FnOnce(&Pressed) -> T) -> Option<T> {
    let key = named(&event.key)?;
    let text = match &event.key {
        Key::Text(text) => Some(text.as_str()),
        _ => None,
    };
    Some(run(&Pressed {
        key: &key,
        text,
        ctrl: event.mods.ctrl,
        alt: event.mods.alt,
        shift: event.mods.shift,
        cmd: event.mods.logo,
    }))
}

/// Whether `pressed` is `chord`, modifiers exactly, as a field's `keys` match.
pub(crate) fn is_chord(chord: &KeyChord, pressed: &Pressed) -> bool {
    let (ctrl, cmd) = match (chord.secondary, MAC) {
        (true, true) => (chord.ctrl, true),
        (true, false) => (true, chord.cmd),
        (false, _) => (chord.ctrl, chord.cmd),
    };
    pressed.key == chord.key
        && pressed.ctrl == ctrl
        && pressed.alt == chord.alt
        && pressed.shift == chord.shift
        && pressed.cmd == cmd
}

/// `pressed`'s name as `kbd` and `keys` write it, such as `cmd-shift-p`. Bare
/// characters aren't shortcuts: they would take over typing.
pub(crate) fn chord_name(pressed: &Pressed) -> Option<String> {
    let modified = pressed.ctrl || pressed.alt || pressed.cmd;
    if !modified && pressed.key.chars().count() == 1 {
        return None;
    }
    let held = [
        (pressed.ctrl, "ctrl"),
        (pressed.alt, "alt"),
        (pressed.shift, "shift"),
        (pressed.cmd, "cmd"),
    ];
    let mut parts: Vec<&str> = held
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| *name)
        .collect();
    parts.push(pressed.key);
    Some(parts.join("-"))
}

impl Ui {
    /// Offers a key no control took, in the order the GPUI app does.
    pub(crate) fn key(&mut self, event: &KeyEvent, tx: &mut Tx) -> bool {
        let taken = pressed(event, |pressed| self.offer_layers(pressed)).unwrap_or(false);
        if taken {
            return true;
        }
        // Nothing has the keyboard, and Rune can't give it to an element: the
        // terminal on screen takes keys as a focused one would. An open layer
        // blocks it, as a dialog over a terminal does on GPUI.
        if tx.focused().is_none()
            && self.layers.get_mut().is_empty()
            && let Some(terminal) = self.terminals.in_use()
            && self.terminal_key(&terminal, event, tx)
        {
            return true;
        }
        pressed(event, |pressed| self.offer(pressed, tx)).unwrap_or(false)
    }

    fn offer_layers(&mut self, pressed: &Pressed) -> bool {
        let topmost = self.layers.get_mut().last().and_then(|layer| {
            let msg = layer.keys.message(layer.dismiss.as_ref(), pressed, MAC)?;
            Some((layer.owner.clone(), layer.to.clone(), msg.clone()))
        });
        if let Some((owner, to, msg)) = topmost {
            self.send(&owner, &to, msg);
            return true;
        }
        // Escape leaves a pick, with nothing picked.
        if self.inspecting.asker.is_some() && pressed.key == "escape" {
            self.answer_pick(None);
            return true;
        }
        false
    }

    fn offer(&mut self, pressed: &Pressed, tx: &mut Tx) -> bool {
        let shortcut = self.workspace().menus().iter().find_map(|menu| {
            menu.items.iter().find_map(|entry| match entry {
                MenuEntry::Item(item)
                    if item
                        .shortcut
                        .as_ref()
                        .is_some_and(|chord| chord.matches(pressed, MAC)) =>
                {
                    Some(item.action.clone())
                },
                _ => None,
            })
        });
        if let Some(action) = shortcut {
            self.act(moonframe_core::SHELL, &action, tx);
            return true;
        }
        let hearers: Vec<(String, Data)> = self
            .workspace()
            .mod_keys()
            .filter_map(|(owner, keys)| {
                Some((owner.to_owned(), keys.message(None, pressed, MAC)?.clone()))
            })
            .collect();
        for (owner, msg) in &hearers {
            let action = Action::Send {
                to: None,
                msg: msg.clone(),
            };
            self.act(owner, &action, tx);
        }
        !hearers.is_empty()
    }
}
