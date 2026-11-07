//! `select` and `app:edit`: what fields say beyond typing, and take from mods.

use {
    super::*,
    moonframe_core::{Edit, EditChange, SHELL},
};

const NOTES: &str = r#"
return mod {
	init = function() return { said = "nothing" } end,
	update = function(state, msg)
		if msg.changed then state.said = "changed " .. msg.value end
		if msg.selected_it then state.said = "selected " .. msg.selected end
		return state
	end,
	view = function(state)
		return ui.column {
			ui.input {
				id = "notes",
				value = "hello world",
				change = { changed = true },
				select = { selected_it = true },
			},
			ui.text { state.said },
		}
	end,
}"#;

fn notes() -> (TempDir, Runtime<Ui>) {
    let dir = folder(NOTES);
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (600., 200.),
        Env::new(true, 2.),
    );
    settle(&mut window);
    (dir, window)
}

fn select(window: &mut Runtime<Ui>, anchor: usize, focus: usize) {
    let key = format!("{SHELL}:notes");
    window.update(|ui, _| {
        let fields = ui.fields.get_mut();
        fields
            .get_mut(&key)
            .unwrap()
            .edit
            .set_selection(anchor, focus);
    });
    // What each frame's housekeeping does; a test window has none.
    window.update(|ui, _| {
        ui.tell_selections();
    });
    settle(window);
}

fn edit(field: &str) -> Edit {
    Edit {
        field: field.to_owned(),
        change: EditChange::Wrap {
            before: "**".to_owned(),
            after: "**".to_owned(),
        },
        expected: None,
    }
}

#[test]
fn moving_the_selection_says_how_much_is_selected() {
    let (_dir, mut window) = notes();
    select(&mut window, 0, 5);
    assert!(shows(&window, "selected 5"));
}

#[test]
fn an_edit_wraps_the_selection_and_says_so() {
    let (_dir, mut window) = notes();
    select(&mut window, 0, 5);
    window.update(|ui, _| ui.apply_edit(SHELL, &edit("notes")));
    settle(&mut window);
    assert!(shows(&window, "changed **hello** world"));
}

#[test]
fn an_edit_of_no_such_field_is_refused() {
    let (_dir, mut window) = notes();
    window.update(|ui, _| ui.apply_edit(SHELL, &edit("elsewhere")));
    let problems = window.state.workspace.problems();
    assert!(
        problems
            .iter()
            .any(|(_, problem)| problem.to_string().contains("elsewhere")),
        "{problems:?}"
    );
}
