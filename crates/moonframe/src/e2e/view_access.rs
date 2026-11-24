//! `moonframe.view`'s acts in a window: a fill goes through the drawn
//! field, which shows it and sends its `change`, and a key on the field
//! sends what the field holds, as a person's typing would.

use {
    super::*,
    moonframe_core::{SHELL, ViewAct},
};

const FORM: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		if type(msg) == "table" and msg.msg == "typed" then
			logged(state, "typed:" .. msg.value)
		elseif type(msg) == "table" and msg.msg == "go" then
			logged(state, "go:" .. msg.value)
		else
			logged(state, tostring(msg))
		end
	end,
	view = function(state)
		return shell(state, ui.column {
			ui.input { id = "q", placeholder = "Search", change = "typed", submit = "go" },
			ui.button { "Clear", msg = "clear" },
		})
	end,
}
"##;

impl Window<'_> {
    /// The ref of the first element of the shell's view with `role` and `name`.
    fn reference(&mut self, role: &str, name: &str) -> String {
        let app = self.app.clone();
        self.cx.read_entity(&app, |app, cx| {
            let tree = app.read_view(SHELL, &|_| true, cx).unwrap();
            tree.nodes
                .iter()
                .find(|node| node.role == role && node.name == name)
                .unwrap_or_else(|| panic!("no {role} {name:?} in\n{}", tree.lines()))
                .reference
                .clone()
        })
    }

    fn act(&mut self, reference: &str, act: ViewAct) {
        let app = self.app.clone();
        self.cx
            .update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.act_on_view(SHELL, &|_| true, reference, &act, window, cx)
                })
            })
            .unwrap();
        self.settle();
    }
}

#[gpui_kit::test]
fn a_fill_types_in_the_field_and_a_key_sends_what_it_holds(cx: &mut TestAppContext) {
    let mut window = open(cx, FORM);
    let field = window.reference("textbox", "Search");
    window.act(&field, ViewAct::Fill("pears".to_owned()));
    let app = window.app.clone();
    let shown = window
        .cx
        .read_entity(&app, |app, cx| app.inputs.text(SHELL, "q", cx));
    assert_eq!(shown.as_deref(), Some("pears"));
    // The tree shows what the field holds, which the mod never passed back.
    let app = window.app.clone();
    let lines = window.cx.read_entity(&app, |app, cx| {
        app.read_view(SHELL, &|_| true, cx).unwrap().lines()
    });
    assert!(
        lines.contains(r#"textbox "Search" value="pears""#),
        "{lines}"
    );
    assert_eq!(window.log(), "|typed:pears");
    window.act(&field, ViewAct::Press("enter".to_owned()));
    assert_eq!(window.log(), "|typed:pears|go:pears");
    let clear = window.reference("button", "Clear");
    window.act(&clear, ViewAct::Click);
    assert_eq!(window.log(), "|typed:pears|go:pears|clear");
}
