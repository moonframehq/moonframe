//! A spinning icon turns in a live view of its own: the frames it asks for
//! replay what stands still around it, and a change still draws.

use super::*;

/// A spinner, a row that never changes, and a row whose class a click flips.
const SPINNER: &str = r#"
return mod {
    init = function() return { log = "", on = false } end,
    update = function(state, msg)
        if msg == "flip" then
            state.on = not state.on
            logged(state, "flip")
        end
    end,
    view = function(state)
        return shell(state, ui.column { grow = true,
            ui.icon { "loader", spin = true },
            ui.row { class = "still", ui.text "a", ui.text "b", ui.text "c" },
            ui.row { class = "flip", msg = "flip", ui.text "Flip" },
            ui.row { class = if state.on then "after" else "before", ui.text "d", ui.text "e", ui.text "f" },
        })
    end,
}"#;

impl Window<'_> {
    /// Delivers the next frame, as the display would, and draws what it asked for.
    fn next_frame(&mut self) {
        self.cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        self.cx.run_until_parked();
    }

    fn replayed(&mut self) -> bool {
        let app = self.app.clone();
        self.cx
            .read_entity(&app, |app, _| app.replayed_last_frame())
    }
}

#[gpui_kit::test]
fn a_spinner_replays_what_stands_still_and_a_change_still_draws(cx: &mut TestAppContext) {
    let mut window = open(cx, SPINNER);
    for _ in 0..3 {
        window.next_frame();
    }
    assert!(window.replayed(), "the spinner's frames replay the rest");
    assert!(window.shows("still"));

    let flip = window.bounds("flip").center();
    window.cx.simulate_mouse_move(flip, None, Modifiers::none());
    window.cx.simulate_click(flip, Modifiers::none());
    window.cx.run_until_parked();
    window.next_frame();
    assert_eq!(window.log(), "|flip");
    assert!(window.shows("after"), "the change is drawn, not replayed");
    assert!(!window.shows("before"));

    for _ in 0..3 {
        window.next_frame();
    }
    assert!(window.replayed(), "and then the spinner replays again");
    assert!(window.shows("after"));
}
