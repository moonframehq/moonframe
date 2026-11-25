use super::*;

#[test]
fn a_stack_hears_presses_outside_it_where_to_says() {
    let outside = |source: &str| {
        let Node::Stack(stack) = view_of(source).unwrap() else {
            panic!("expected a column");
        };
        stack.outside
    };
    let own =
        r#"return mod { view = function() return ui.column { outside = "minimize", "a" } end }"#;
    assert!(matches!(
        outside(own),
        Some(Action::Send { to: None, msg: Data::String(msg) }) if msg == "minimize"
    ));
    let other = r#"return mod { view = function() return ui.row { outside = { close = true }, to = "ask", "a" } end }"#;
    assert!(matches!(outside(other), Some(Action::Send { to: Some(to), .. }) if to == "ask"));
    let none = r#"return mod { view = function() return ui.row { "a" } end }"#;
    assert_eq!(outside(none), None);
}
