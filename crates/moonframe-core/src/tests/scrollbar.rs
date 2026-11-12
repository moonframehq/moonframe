use super::*;

#[test]
fn a_scrolling_frame_draws_its_scrollbar_unless_told_not_to() {
    let frame = |source: &str| {
        let Node::Stack(stack) = view_of(source).unwrap() else {
            panic!("expected a column");
        };
        (stack.frame.scroll, stack.frame.scrollbar)
    };
    let shown = r#"return mod { view = function() return ui.column { scroll = true, "a" } end }"#;
    assert_eq!(frame(shown), (crate::Scroll::Along, true));
    let hidden = r#"return mod { view = function() return ui.column { scroll = true, scrollbar = false, "a" } end }"#;
    assert_eq!(frame(hidden), (crate::Scroll::Along, false));

    let wrong = r#"return mod { view = function() return ui.column { scrollbar = "no" } end }"#;
    let error = view_of(wrong).unwrap_err();
    assert!(error.contains("scrollbar"), "{error}");
}
