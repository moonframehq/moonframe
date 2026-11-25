//! `app.window` carries the window's size, for a view that lays itself
//! out narrow under a breakpoint.

use super::*;

#[gpui_kit::test]
fn a_view_reads_the_windows_size(cx: &mut TestAppContext) {
    let main = r#"
return mod {
	view = function(_, app)
		return ui.text { `log:{app.window.width}x{app.window.height}` }
	end,
}"#;
    let mut window = open(cx, main);
    let size = window.cx.update(|window, _| window.viewport_size());
    let whole = |length: Pixels| f32::from(length).round() as u32;
    assert_eq!(
        window.log(),
        format!("{}x{}", whole(size.width), whole(size.height))
    );
}
