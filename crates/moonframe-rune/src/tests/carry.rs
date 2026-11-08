//! A row's `carry` onto one that `accept`s it, and `hover`, as their mods
//! hear them.

use super::*;

const BOARD: &str = r#"
return mod {
	init = function() return { said = "nothing", hovered = "no" } end,
	update = function(state, msg)
		if msg.got then state.said = msg.phase .. " " .. tostring(msg.carry.card) .. " from " .. msg.from end
		if msg.near then state.hovered = msg.phase end
		return state
	end,
	view = function(state)
		return ui.column {
			gap = "large",
			ui.row { height = 40, width = 200, shrink = false, carry = { card = 7 }, ui.text "card" },
			ui.row { height = 80, width = 300, shrink = false, accept = { got = true }, ui.text "column" },
			ui.row { height = 40, width = 200, shrink = false, hover = true, pointer = { near = true }, ui.text "hover me" },
			ui.text { state.said },
			ui.text { "hovered " .. state.hovered },
		}
	end,
}"#;

fn board() -> (TempDir, Runtime<Ui>) {
    let dir = folder(BOARD);
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (600., 400.),
        Env::new(true, 2.),
    );
    settle(&mut window);
    (dir, window)
}

fn centre_of(window: &Runtime<Ui>, said: &str) -> (f32, f32) {
    let [x, y, w, h] = rect_of(window, |element| element.text.as_deref() == Some(said));
    (x + w / 2., y + h / 2.)
}

#[test]
fn a_card_dropped_on_a_column_tells_the_column() {
    let (_dir, mut window) = board();
    let (cx, cy) = centre_of(&window, "card");
    let (tx, ty) = centre_of(&window, "column");
    window.pointer_down(cx, cy);
    settle(&mut window);
    window.pointer_move(tx, ty);
    settle(&mut window);
    assert!(
        shows(&window, "over 7 from shell"),
        "it hears the card over it"
    );
    window.pointer_up(tx, ty);
    settle(&mut window);
    assert!(shows(&window, "drop 7 from shell"), "and the drop");
}

#[test]
fn a_card_dropped_elsewhere_tells_nothing_of_a_drop() {
    let (_dir, mut window) = board();
    let (cx, cy) = centre_of(&window, "card");
    window.pointer_down(cx, cy);
    window.pointer_move(cx + 5., cy + 300.);
    window.pointer_up(cx + 5., cy + 300.);
    settle(&mut window);
    assert!(shows(&window, "nothing"));
}

#[test]
fn hovering_a_row_says_so_and_leaving_it_too() {
    let (_dir, mut window) = board();
    let (hx, hy) = centre_of(&window, "hover me");
    window.pointer_move(hx, hy);
    settle(&mut window);
    assert!(shows(&window, "hovered hover"));
    window.pointer_move(hx, hy + 200.);
    settle(&mut window);
    assert!(shows(&window, "hovered leave"));
}
