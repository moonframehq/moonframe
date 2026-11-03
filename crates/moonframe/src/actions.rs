use {
    gpui_kit::{App, KeyBinding, Menu, MenuItem},
    moonframe_core::{AppMenu, Appearance, Chord, MenuEntry},
};

gpui_kit::actions!(moonframe, [
    Quit,
    Reload,
    ToggleTheme,
    Inspect,
    Dismiss,
    BrowseComponents,
    ShowThemes,
    AboutApp,
    BuildApp,
    CheckForUpdates,
    ToggleBetaReleases,
    ToggleSlowAnimations,
    ToggleDeveloperMode,
    UndoChange,
    RedoChange
]);

/// Runs a shell menu item, found by its id (menu title and label).
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = moonframe, no_json)]
pub(crate) struct MenuSend {
    pub(crate) id: String,
}

/// A shell menu shortcut was pressed: the item it runs is looked up when
/// it is pressed, so a shortcut a reload took away or moved runs nothing.
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = moonframe, no_json)]
pub(crate) struct MenuKey {
    pub(crate) keystroke: String,
}

/// Makes the window light, dark, or follow the system (View > Appearance).
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = moonframe, no_json)]
pub(crate) struct SetAppearance {
    pub(crate) appearance: Appearance,
}

thread_local! {
    /// The appearance View > Appearance shows checked.
    static APPEARANCE: std::cell::Cell<Appearance> = const { std::cell::Cell::new(Appearance::System) };
}

/// Shows `appearance` checked in View > Appearance, from the next time the
/// menus are set.
pub(crate) fn show_appearance(appearance: Appearance) {
    APPEARANCE.set(appearance);
}

/// Opens the component browser on one component.
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = moonframe, no_json)]
pub(crate) struct ShowComponent {
    pub(crate) name: String,
}

pub(crate) fn bind_keys(cx: &mut App) {
    // Tools for changing the app, only in Developer Mode: the window's key
    // context says `developer` then, so turning it off frees these keys
    // for the app's own menus.
    cx.bind_keys([
        KeyBinding::new("cmd-r", Reload, Some("MoonframeApp && developer")),
        KeyBinding::new("cmd-alt-i", Inspect, Some("MoonframeApp && developer")),
        KeyBinding::new("cmd-alt-z", UndoChange, Some("MoonframeApp && developer")),
        KeyBinding::new(
            "cmd-alt-shift-z",
            RedoChange,
            Some("MoonframeApp && developer"),
        ),
        KeyBinding::new(
            "cmd-shift-k",
            BrowseComponents,
            Some("MoonframeApp && developer"),
        ),
    ]);
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-shift-l", ToggleTheme, Some("MoonframeApp")),
        KeyBinding::new("cmd-k cmd-t", ShowThemes, Some("MoonframeApp")),
        // Escape in a terminal is the shell's (vim needs it).
        KeyBinding::new("escape", Dismiss, Some("MoonframeApp && !Terminal")),
        KeyBinding::new("escape", Dismiss, Some("Gallery")),
        KeyBinding::new("cmd-w", Dismiss, Some("Gallery")),
        KeyBinding::new("cmd-w", Dismiss, Some("ThemePicker")),
        KeyBinding::new("escape", Dismiss, Some("ModWindow")),
        KeyBinding::new("cmd-w", Dismiss, Some("ModWindow")),
    ]);
}

/// The menu bar: the app menu (About, Check for Updates and Beta Releases
/// when the app names its repository, Quit), the shell's `menus`, then
/// View, into which a shell menu called View merges, and the shell's
/// Window and Help. View's Components… opens the gallery, which lists
/// every component itself.
/// Outside Developer Mode only what the app's users need shows: no
/// Components…, Inspect Element, Reload Modules, Undo and Redo of
/// changes to the app's files, or Slow Animations.
pub(crate) fn menus(shell: &[AppMenu]) -> Vec<Menu> {
    let title = crate::title();
    let mut app_items = vec![MenuItem::action(format!("About {title}"), AboutApp)];
    // An app that names the GitHub repository it is released from.
    if crate::update::source().is_some() {
        app_items.push(MenuItem::action("Check for Updates…", CheckForUpdates));
        app_items.push(
            MenuItem::action("Beta Releases", ToggleBetaReleases).checked(crate::update::beta()),
        );
    }
    // Developer Mode: the tools for changing the app as it runs, which
    // its users can turn on too; the choice is kept.
    app_items.extend([
        MenuItem::separator(),
        MenuItem::action("Developer Mode", ToggleDeveloperMode).checked(!crate::release()),
        MenuItem::separator(),
    ]);
    // Outside a release, the starter builds itself as one; an app on the
    // library bundles with its own `bundle` command.
    if !crate::release() && !crate::identity().hosted {
        app_items.extend([
            MenuItem::action("Build App Bundle", BuildApp),
            MenuItem::separator(),
        ]);
    }
    app_items.push(MenuItem::action(format!("Quit {title}"), Quit));
    let mut bar = vec![Menu::new(title.clone()).items(app_items)];
    let mut view_items = Vec::new();
    // Window and Help go last, after View, as macOS orders them.
    let mut last = Vec::new();
    for menu in shell {
        let items = menu.items.iter().map(|entry| match entry {
            MenuEntry::Separator => MenuItem::separator(),
            MenuEntry::Item(item) => MenuItem::action(item.label.clone(), MenuSend {
                id: item.id(&menu.title),
            }),
        });
        if menu.title == "View" {
            view_items.extend(items);
            view_items.push(MenuItem::separator());
        } else if matches!(menu.title.as_str(), "Window" | "Help") {
            last.push(Menu::new(menu.title.clone()).items(items));
        } else {
            bar.push(Menu::new(menu.title.clone()).items(items));
        }
    }
    let shown = APPEARANCE.get();
    let appearances = [
        ("System", Appearance::System),
        ("Light", Appearance::Light),
        ("Dark", Appearance::Dark),
    ]
    .map(|(label, appearance)| {
        MenuItem::action(label, SetAppearance { appearance }).checked(appearance == shown)
    });
    view_items.extend([
        MenuItem::submenu(Menu::new("Appearance").items(appearances)),
        MenuItem::action("Themes…", ShowThemes),
        MenuItem::action("Toggle Dark Mode", ToggleTheme),
    ]);
    if crate::release() {
        bar.push(Menu::new("View").items(view_items));
        bar.extend(last);
        return bar;
    }
    view_items.extend([
        MenuItem::separator(),
        MenuItem::action("Components…", BrowseComponents),
        MenuItem::action("Reload Modules", Reload),
        MenuItem::action("Undo Last Change", UndoChange),
        MenuItem::action("Redo Change", RedoChange),
        MenuItem::action("Inspect Element", Inspect),
        MenuItem::action("Slow Animations", ToggleSlowAnimations).checked(crate::motion::is_slow()),
    ]);
    bar.push(Menu::new("View").items(view_items));
    bar.extend(last);
    bar
}

/// Binds each shell menu item's `shortcut`, once per key: GPUI keeps
/// bindings for the app's life, so each key sends [`MenuKey`] and the item
/// it runs is found when it is pressed. A key no item has any more after a
/// reload then runs nothing and goes on to whatever else takes it.
pub(crate) fn bind_menu_keys(shell: &[AppMenu], cx: &mut App) {
    thread_local! {
        static BOUND: std::cell::RefCell<std::collections::BTreeSet<String>> =
            const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
    }
    let mut bindings = Vec::new();
    for menu in shell {
        for entry in &menu.items {
            let MenuEntry::Item(item) = entry else {
                continue;
            };
            let Some(chord) = &item.shortcut else {
                continue;
            };
            // A key that types text is no binding (it would take the key
            // from text fields): `menu_keystroke` offers it instead.
            if chord.types_text() {
                continue;
            }
            let keystroke = keystroke(chord);
            if BOUND.with_borrow_mut(|bound| bound.insert(keystroke.clone())) {
                // GPUI takes a key's name for the app's life; each is bound
                // once, so these few strings are all that is kept.
                let name: &'static str = Box::leak(keystroke.clone().into_boxed_str());
                bindings.push(KeyBinding::new(
                    name,
                    MenuKey { keystroke },
                    Some("MoonframeApp"),
                ));
            }
        }
    }
    cx.bind_keys(bindings);
}

/// GPUI's name for `chord`: `secondary` is ⌘ on macOS, Ctrl elsewhere.
pub(crate) fn keystroke(chord: &Chord) -> String {
    let mac = cfg!(target_os = "macos");
    let mut parts = Vec::new();
    if chord.ctrl || (chord.secondary && !mac) {
        parts.push("ctrl");
    }
    if chord.alt {
        parts.push("alt");
    }
    if chord.shift {
        parts.push("shift");
    }
    if chord.cmd || (chord.secondary && mac) {
        parts.push("cmd");
    }
    parts.push(&chord.key);
    parts.join("-")
}

#[cfg(test)]
mod tests {
    use {super::*, gpui_kit::Action, std::sync::atomic::Ordering};

    /// Every action item in `menus`, submenus included, as (name, action).
    fn actions(menus: &[Menu]) -> Vec<(String, &dyn Action)> {
        fn walk<'a>(items: &'a [MenuItem], out: &mut Vec<(String, &'a dyn Action)>) {
            for item in items {
                match item {
                    MenuItem::Action { name, action, .. } => {
                        out.push((name.to_string(), action.as_ref()));
                    },
                    MenuItem::Submenu(menu) => walk(&menu.items, out),
                    _ => {},
                }
            }
        }
        let mut out = Vec::new();
        for menu in menus {
            walk(&menu.items, &mut out);
        }
        out
    }

    fn browses(action: &dyn Action) -> bool {
        action.as_any().is::<BrowseComponents>()
    }

    #[test]
    fn components_is_one_view_item_in_developer_mode_only() {
        let was = crate::RELEASE.load(Ordering::Relaxed);

        crate::RELEASE.store(false, Ordering::Relaxed);
        let bar = menus(&[]);
        assert!(bar.iter().all(|menu| menu.name != "Components"));
        let view: Vec<Menu> = menus(&[])
            .into_iter()
            .filter(|menu| menu.name == "View")
            .collect();
        assert_eq!(view.len(), 1, "one View menu");
        let browsing: Vec<String> = actions(&view)
            .into_iter()
            .filter(|(_, action)| browses(*action))
            .map(|(name, _)| name)
            .collect();
        assert_eq!(browsing, ["Components…"]);
        let everything = actions(&bar);
        assert!(
            everything
                .iter()
                .all(|(_, action)| !action.as_any().is::<ShowComponent>()),
            "no menu item opens one component"
        );
        assert_eq!(
            everything
                .iter()
                .filter(|(_, action)| browses(*action))
                .count(),
            1
        );

        crate::RELEASE.store(true, Ordering::Relaxed);
        let bar = menus(&[]);
        assert!(actions(&bar).iter().all(|(_, action)| !browses(*action)));

        crate::RELEASE.store(was, Ordering::Relaxed);
    }
}
