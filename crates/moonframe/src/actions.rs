use {
    gpui_kit::{App, KeyBinding, Menu, MenuItem},
    moonframe_core::{AppMenu, Chord, Component, MenuEntry},
    std::collections::BTreeMap,
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
    ToggleSlowAnimations
]);

/// Runs a shell menu item, found by its id (menu title and label).
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = moonframe, no_json)]
pub(crate) struct MenuSend {
    pub(crate) id: String,
}

/// Opens the component browser on one component.
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = moonframe, no_json)]
pub(crate) struct ShowComponent {
    pub(crate) name: String,
}

pub(crate) fn bind_keys(cx: &mut App) {
    if !crate::release() {
        // Tools for building the app, which a release leaves out.
        cx.bind_keys([
            KeyBinding::new("cmd-r", Reload, Some("MoonframeApp")),
            KeyBinding::new("cmd-alt-i", Inspect, Some("MoonframeApp")),
            KeyBinding::new("cmd-shift-k", BrowseComponents, Some("MoonframeApp")),
        ]);
    }
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

/// The menu bar: the app menu (About, Quit), the shell's `menus`, then
/// View, into which a shell menu called View merges, the Components menu
/// listing `components` by category, and the shell's Window and Help. A release shows only what the
/// app's users need: no Components menu, Inspect Element, Reload Modules,
/// or Slow Animations.
pub(crate) fn menus(components: &[Component], shell: &[AppMenu]) -> Vec<Menu> {
    let title = crate::title();
    let mut app_items = vec![
        MenuItem::action(format!("About {title}"), AboutApp),
        MenuItem::separator(),
    ];
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
    view_items.extend([
        MenuItem::action("Themes…", ShowThemes),
        MenuItem::action("Toggle Dark Mode", ToggleTheme),
    ]);
    if crate::release() {
        bar.push(Menu::new("View").items(view_items));
        bar.extend(last);
        return bar;
    }
    let mut categories: BTreeMap<&str, Vec<&Component>> = BTreeMap::new();
    for component in components {
        categories
            .entry(&component.category)
            .or_default()
            .push(component);
    }
    let listed = categories.into_iter().map(|(category, members)| {
        MenuItem::submenu(
            Menu::new(category.to_owned()).items(members.into_iter().map(|component| {
                MenuItem::action(component.title.clone(), ShowComponent {
                    name: component.name.clone(),
                })
            })),
        )
    });
    view_items.extend([
        MenuItem::separator(),
        MenuItem::action("Reload Modules", Reload),
        MenuItem::action("Inspect Element", Inspect),
        MenuItem::action("Slow Animations", ToggleSlowAnimations).checked(crate::motion::is_slow()),
    ]);
    bar.extend([
        Menu::new("View").items(view_items),
        Menu::new("Components").items(
            [
                MenuItem::action("Browse Components…", BrowseComponents),
                MenuItem::separator(),
            ]
            .into_iter()
            .chain(listed),
        ),
    ]);
    bar.extend(last);
    bar
}

/// Binds each shell menu item's `shortcut` to it, once per key and item:
/// GPUI keeps bindings for the app's life, and an item is found again by
/// its id after a reload.
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
            let keystroke = keystroke(chord);
            let id = item.id(&menu.title);
            let fresh =
                BOUND.with_borrow_mut(|bound| bound.insert(format!("{keystroke}\u{1f}{id}")));
            if fresh {
                // GPUI takes a key's name for the app's life; each is bound
                // once, so these few strings are all that is kept.
                let keystroke: &'static str = Box::leak(keystroke.into_boxed_str());
                bindings.push(KeyBinding::new(
                    keystroke,
                    MenuSend { id },
                    Some("MoonframeApp"),
                ));
            }
        }
    }
    cx.bind_keys(bindings);
}

/// GPUI's name for `chord`: `secondary` is ⌘ on macOS, Ctrl elsewhere.
fn keystroke(chord: &Chord) -> String {
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
