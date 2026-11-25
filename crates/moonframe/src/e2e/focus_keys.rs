//! A column's own keys, taken while focus is inside it: a decision deck
//! whose arrows dismiss and keep only once the reader is in it, ahead of
//! the page's keys, and never while its note field has focus.

use super::*;

const DECK: &str = r##"
return mod {
	keys = { right = "page" },
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		if type(msg) == "table" then
			logged(state, `typed {msg.value}`)
		else
			logged(state, msg)
			if msg == "note" then state.focus = true end
		end
	end,
	view = function(state)
		return shell(state, ui.column { class = "deck", keys = { left = "dismiss", right = "keep" },
			ui.row { class = "card", height = 40, ui.text { "Follow up with Ana" } },
			ui.row { class = "note", msg = "note", ui.text { "Note" } },
			ui.input { id = "note", change = "typed", focus = state.focus },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_column_takes_its_keys_while_focus_is_inside_it(cx: &mut TestAppContext) {
    let mut window = open(cx, DECK);
    // Focus is elsewhere: the page's keys.
    window.keys("right");
    assert_eq!(window.log(), "|page");
    window.click("card");
    window.keys("right");
    window.keys("left");
    assert_eq!(window.log(), "|page|keep|dismiss");
    // Its field has focus: the arrows move the caret there.
    window.click("note");
    window.keys("left");
    assert_eq!(window.log(), "|page|keep|dismiss|note");
}

/// The deck in a dialog with a backdrop.
const DIALOG: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		logged(state, msg)
	end,
	view = function(state)
		return shell(state, ui.overlay { open = true, backdrop = true, width = 400, height = 200,
			content = ui.column { class = "deck", keys = { left = "dismiss", right = "keep" },
				ui.row { class = "card", height = 40, ui.text { "Follow up with Ana" } },
			},
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_column_in_a_dialog_takes_its_keys(cx: &mut TestAppContext) {
    let mut window = open(cx, DIALOG);
    window.click("card");
    window.keys("right");
    assert_eq!(window.log(), "|keep");
}

/// A host view that only takes focus.
struct Field(gpui_kit::FocusHandle);

impl gpui_kit::Render for Field {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{InteractiveElement as _, Styled as _};
        gpui_kit::div()
            .id("field")
            .debug_selector(|| "field".into())
            .track_focus(&self.0)
            .size_full()
    }
}

impl crate::hosted::HostView for Field {
    fn set_props(&mut self, _: &moonframe_core::Data, _: &mut gpui_kit::Context<Self>) {}
}

/// A column with keys, around a row with keys, around the `field` view;
/// both faded.
const AROUND_VIEW: &str = r##"
return mod {
	keys = { right = "page" },
	init = function() return { log = "" } end,
	update = function(state, msg) logged(state, msg) end,
	view = function(state)
		return shell(state, ui.column { class = "deck faded", keys = { right = "keep" }, height = 100,
			ui.row { class = "faded", keys = { left = "back" }, grow = true, ui.view { name = "field" } },
		})
	end,
}
"##;

/// A window on [`AROUND_VIEW`], with `faded` at half opacity, and the
/// focus of its view's control.
fn around_view(cx: &mut TestAppContext) -> (Window<'_>, gpui_kit::FocusHandle) {
    let dir = folder(AROUND_VIEW);
    let style = dir.path().join("style.luau");
    let sheet = std::fs::read_to_string(&style).unwrap().replacen(
        "\trules = {",
        "\trules = {\n\t\t[\".faded\"] = { opacity = 0.5 },",
        1,
    );
    std::fs::write(&style, sheet).unwrap();
    let focus = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut views = Registry::default();
    {
        let focus = focus.clone();
        views.insert("field", move |_, _, cx| {
            let handle = cx.focus_handle();
            *focus.borrow_mut() = Some(handle.clone());
            Field(handle)
        });
    }
    let window = open_with(cx, dir, views);
    let handle = focus.borrow().clone().expect("the view is made");
    (window, handle)
}

#[gpui_kit::test]
fn a_column_takes_its_keys_while_a_view_in_it_has_focus(cx: &mut TestAppContext) {
    let (mut window, handle) = around_view(cx);
    window.keys("right");
    assert_eq!(window.log(), "|page");
    // The view's own control has focus: that is inside the row around it,
    // and the column around that.
    window.cx.update(|window, cx| handle.focus(window, cx));
    window.settle();
    window.keys("left");
    window.keys("right");
    assert_eq!(window.log(), "|page|back|keep");
    // A click in the view gives its control focus, as before.
    window.cx.update(|window, cx| window.blur(cx));
    window.settle();
    window.click("field");
    let focused = window.cx.update(|window, _| handle.is_focused(window));
    assert!(focused, "the view's control has focus");
    window.keys("left");
    assert_eq!(window.log(), "|page|back|keep|back");
    // The view stays in its layer: the app may be replayed under it.
    let app = window.app.clone();
    let replayable = window
        .cx
        .read_entity(&app, |app, _| app.views.layers().replayable());
    assert!(replayable, "the view is drawn in a layer");
}

#[gpui_kit::test]
fn a_view_in_a_layer_fades_with_what_it_is_in(cx: &mut TestAppContext) {
    let (window, _) = around_view(cx);
    let app = window.app.clone();
    let opacities = window
        .cx
        .read_entity(&app, |app, _| app.views.layers().opacities());
    assert_eq!(opacities, [Some(0.25)]);
}
