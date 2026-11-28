//! A menu item checked by the shell's state follows that state whatever
//! changed it: a provider's data as much as a click.

use {
    super::*,
    moonframe_core::{Data, MenuEntry},
};

/// A shell whose View menu ticks "On" while the `mode` provider says so.
const SHELL: &str = r#"
return mod {
    init = function() return { log = "", mode = "off" } end,
    subscribe = { "provider/mode" },
    menus = { { title = "View", items = {
        { label = "On", msg = "on", checked = { mode = "on" } },
    } } },
    update = function(state, msg)
        if type(msg) == "table" and msg.topic then state.mode = msg.value end
    end,
    view = function(state) return shell(state, ui.text { state.mode }) end,
}"#;

fn checked(window: &mut Window<'_>) -> bool {
    window.app.read_with(window.cx, |app, _| {
        app.workspace
            .menus()
            .iter()
            .flat_map(|menu| &menu.items)
            .any(|entry| matches!(entry, MenuEntry::Item(item) if item.checked))
    })
}

#[gpui_kit::test]
fn a_providers_data_ticks_the_menu_at_once(cx: &mut TestAppContext) {
    let mut window = open(cx, SHELL);
    assert!(!checked(&mut window));
    let app = window.app.clone();
    app.update(window.cx, |app, cx| {
        app.published("mode", Data::from("on"), cx);
        // The bar was built again as the data came, not at the next click.
        assert!(!app.workspace.take_menus_changed());
    });
    assert!(checked(&mut window));
}
