use {
    gpui_kit::{App, KeyBinding, Menu, MenuItem},
    moonframe_core::Component,
    std::collections::BTreeMap,
};

gpui_kit::actions!(moonframe, [
    Quit,
    Reload,
    ToggleTheme,
    Inspect,
    Dismiss,
    BrowseComponents,
    ShowThemes
]);

/// Opens the component browser on one component.
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = moonframe, no_json)]
pub(crate) struct ShowComponent {
    pub(crate) name: String,
}

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-r", Reload, Some("MoonframeApp")),
        KeyBinding::new("cmd-shift-l", ToggleTheme, Some("MoonframeApp")),
        KeyBinding::new("cmd-alt-i", Inspect, Some("MoonframeApp")),
        KeyBinding::new("cmd-shift-k", BrowseComponents, Some("MoonframeApp")),
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

/// The menu bar; the Components menu lists `components` by category.
pub(crate) fn menus(components: &[Component]) -> Vec<Menu> {
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
    vec![
        Menu::new("Moonframe").items([MenuItem::action("Quit", Quit)]),
        Menu::new("View").items([
            MenuItem::action("Reload Modules", Reload),
            MenuItem::action("Themes…", ShowThemes),
            MenuItem::action("Toggle Dark Mode", ToggleTheme),
            MenuItem::separator(),
            MenuItem::action("Inspect Element", Inspect),
        ]),
        Menu::new("Components").items(
            [
                MenuItem::action("Browse Components…", BrowseComponents),
                MenuItem::separator(),
            ]
            .into_iter()
            .chain(listed),
        ),
    ]
}
