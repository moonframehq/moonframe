//! The shell's menu items, run by their keys or ids (the app-wide menu
//! actions are the hub's).

use super::*;

impl MoonframeApp {
    /// A keystroke no overlay, field or terminal took: a shell menu item
    /// whose shortcut is a single typing key (`j`, `shift-c`) runs, as a
    /// mail client's keys do, but only in the main window and while no
    /// text field or terminal has focus, so typing is never taken. Returns
    /// whether one ran.
    pub(crate) fn menu_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_main(window) || self.inputs.any_focused(window, cx) {
            return false;
        }
        #[cfg(feature = "terminal")]
        if self.terminals.any_focused(window) {
            return false;
        }
        let modifiers = keystroke.modifiers;
        let pressed = moonframe_core::Pressed {
            key: &keystroke.key,
            text: keystroke.key_char.as_deref(),
            ctrl: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
            cmd: modifiers.platform,
        };
        let mac = cfg!(target_os = "macos");
        self.run_menu_item_where(window, cx, |_, item| {
            item.shortcut
                .as_ref()
                .is_some_and(|chord| chord.types_text() && chord.matches(&pressed, mac))
        })
    }

    /// The shell menu item whose (non-typing) shortcut is `keystroke`, in
    /// GPUI's spelling, runs. Returns whether one did.
    pub(super) fn run_menu_key(
        &mut self,
        keystroke: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.run_menu_item_where(window, cx, |_, item| {
            item.shortcut.as_ref().is_some_and(|chord| {
                !chord.types_text() && crate::actions::keystroke(chord) == keystroke
            })
        })
    }

    /// Runs the shell menu item `id`, as a click from the shell would.
    pub(super) fn run_menu_item(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.run_menu_item_where(window, cx, |title, item| item.id(title) == id) {
            tracing::warn!("no menu item {:?} any more", id.replace('\u{1f}', " > "));
        }
    }

    /// Runs the first shell menu item `matches` picks (given its menu's
    /// title). Returns whether there was one.
    fn run_menu_item_where(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        matches: impl Fn(&str, &moonframe_core::MenuCommand) -> bool,
    ) -> bool {
        let action = self.workspace.menus().iter().find_map(|menu| {
            menu.items.iter().find_map(|entry| match entry {
                moonframe_core::MenuEntry::Item(item) if matches(&menu.title, item) => {
                    Some(item.action.clone())
                },
                _ => None,
            })
        });
        match action {
            Some(action) => {
                self.dispatch(SHELL, &action, window, cx);
                true
            },
            None => false,
        }
    }

    /// The About panel: the app's name, icon and version, and what the
    /// shell's `about` says. macOS's own panel there; a toast elsewhere.
    pub(crate) fn show_about(&mut self, cx: &mut Context<Self>) {
        let about = self.workspace.about();
        let name = about.name.clone().unwrap_or_else(crate::title);
        let icon = self.folders.icon();
        if crate::dock::about_panel(&name, &about, icon.as_deref(), "GPUI") {
            return;
        }
        let text = format!(
            "{name} {} ({}): {}",
            crate::dock::shown_version(&about),
            crate::dock::built_on(),
            crate::dock::credits(&about, "GPUI").replace("\n\n", " · "),
        );
        self.show_toast_text(text.into(), cx);
    }
}

/// A theme's name; the stock look's for `None`.
pub(super) fn name_of(scheme: Option<&Scheme>) -> &'static str {
    scheme.map_or(themes::DEFAULT, |scheme| scheme.name)
}

/// The stock look as a theme picker shows it, in dark windows or light
/// ones: gpui-component's colors with the stylesheet's over them.
pub(super) fn stock_preview(
    overrides: &[(Token, moonframe_core::Shade)],
    dark: bool,
) -> themes::Preview {
    let mut preview = if dark {
        themes::STOCK_DARK
    } else {
        themes::STOCK_LIGHT
    };
    for (token, shade) in overrides {
        // `0xRRGGBBAA`, without the alpha.
        let color = if dark {
            shade.dark
        } else {
            shade.light
        } >> 8;
        match token {
            Token::Background => preview.background = color,
            Token::Foreground => preview.foreground = color,
            Token::Accent => preview.accent = color,
            _ => {},
        }
    }
    preview
}

/// Whether the system's appearance is a dark one.
pub(super) fn is_dark(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}
