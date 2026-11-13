//! How elements are boxed as on GPUI: a border takes room, so what a
//! bordered element holds sits inside it; a disabled button dims whole.

use super::*;

#[test]
fn what_a_bordered_column_holds_sits_inside_its_border() {
    let main = r#"
return mod {
	styles = {
		[".frame"] = { border = "all", radius = 8 },
		[".padded"] = { border = "all", padding = 4 },
	},
	view = function()
		return ui.column {
			grow = true,
			ui.column { class = "frame", grow = true, ui.column { grow = true, ui.text { "framed" } } },
			ui.column { class = "padded", ui.text { "padded" } },
			ui.column { padding = "large", class = "padded", ui.text { "prop" } },
		}
	end,
}
"#;
    let dir = folder(main);
    let window = open(dir.path());
    let rect = |text: &str| rect_of(&window, |element| element.text.as_deref() == Some(text));
    // The frame's child starts a border's width in from the frame's edge.
    let framed = rect("framed");
    assert!(framed[0] >= 1., "inside the left border: {framed:?}");
    // A look's padding grows by the border.
    let padded = rect("padded");
    assert!(
        (padded[0] - 5.).abs() < 0.01,
        "4 of padding and 1 of border: {padded:?}"
    );
    // So does a frame's, which outranks the look's.
    let prop = rect("prop");
    assert!(
        prop[0] > padded[0],
        "the prop's padding, and the border: {prop:?}"
    );
}

#[test]
fn a_disabled_button_keeps_its_label_color_and_dims_whole() {
    let main = r#"
return mod {
	view = function()
		return ui.column { ui.button { "Send", variant = "primary", disabled = true } }
	end,
}
"#;
    let dir = folder(main);
    let window = open(dir.path());
    let elements = window.elements();
    let label = elements
        .iter()
        .find(|element| element.text.as_deref() == Some("Send"))
        .expect("drawn");
    let workspace = window.state.workspace();
    let palette = theme::Palette {
        dark: window.state.dark,
        overrides: &workspace.stylesheet().overrides,
    };
    let white = palette.token(moonframe_core::Token::AccentForeground);
    assert_eq!(
        label.text_color,
        Some(white),
        "white on the accent, not gray"
    );
    assert!(
        (label.opacity - 0.5).abs() < 0.01,
        "dimmed: {}",
        label.opacity
    );
}
