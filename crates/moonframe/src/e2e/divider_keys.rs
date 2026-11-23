//! A split's divider takes the keyboard once clicked, and a double-click
//! puts the panes back to their starting size.

use {
    super::*,
    gpui_kit::{MouseButton, MouseDownEvent, MouseUpEvent},
};

const SPLIT: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	view = function(state)
		return shell(state, ui.hsplit { id = "people",
			ui.pane { size = 200, min = 100, max = 400, ui.column { class = "list", grow = true, ui.text { "List" } } },
			ui.pane { ui.column { class = "person", grow = true, ui.text { "Person" } } },
		})
	end,
}
"##;

fn width(window: &mut Window<'_>) -> f32 {
    f32::from(window.bounds("list").size.width)
}

/// Where the divider is: on the list pane's right edge.
fn divider(window: &mut Window<'_>) -> Point<Pixels> {
    let list = window.bounds("list");
    point(list.right(), list.center().y)
}

#[gpui_kit::test]
fn a_divider_moves_with_the_keys_and_comes_back_on_a_double_click(cx: &mut TestAppContext) {
    let mut window = open(cx, SPLIT);
    let start = width(&mut window);
    assert!((start - 200.).abs() < 2., "{start}");
    let at = divider(&mut window);
    window.click_at(at);
    window.keys("right");
    assert!(
        (width(&mut window) - (start + 16.)).abs() < 1.,
        "{}",
        width(&mut window)
    );
    window.keys("shift-right");
    assert!(
        (width(&mut window) - (start + 80.)).abs() < 1.,
        "{}",
        width(&mut window)
    );
    window.keys("home");
    assert!(
        (width(&mut window) - 100.).abs() < 2.,
        "{}",
        width(&mut window)
    );
    window.keys("end");
    assert!(
        (width(&mut window) - 400.).abs() < 2.,
        "{}",
        width(&mut window)
    );
    let at = divider(&mut window);
    for event in 1..=2 {
        window.cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
            first_mouse: false,
        });
        window.cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
        });
    }
    window.settle();
    assert!(
        (width(&mut window) - start).abs() < 2.,
        "{}",
        width(&mut window)
    );
}
