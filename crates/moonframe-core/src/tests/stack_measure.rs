use super::*;

#[test]
fn a_scrolling_stack_takes_measure_and_sends_it_where_to_says() {
    let measured = |source: &str| {
        let Node::Stack(stack) = view_of(source).unwrap() else {
            panic!("expected a row");
        };
        stack.measure
    };
    let own = r#"return mod { view = function() return ui.row { scroll = true, measure = { strip = true }, "a" } end }"#;
    let Some(Action::Send {
        to: None,
        msg: Data::Map(msg),
    }) = measured(own)
    else {
        panic!("measure kept")
    };
    assert_eq!(msg.get("strip"), Some(&Data::Bool(true)));
    let other = r#"return mod { view = function() return ui.row { scroll = "x", scrollbar = false, measure = "pills", to = "news", "a" } end }"#;
    assert!(matches!(measured(other), Some(Action::Send { to: Some(to), .. }) if to == "news"));
    let none = r#"return mod { view = function() return ui.row { scroll = true, "a" } end }"#;
    assert_eq!(measured(none), None);
}

#[test]
fn measure_needs_a_stack_that_scrolls() {
    let still =
        r#"return mod { view = function() return ui.column { measure = "list", "a" } end }"#;
    let error = view_of(still).unwrap_err();
    assert!(
        error.contains("measure") && error.contains("scroll"),
        "{error}"
    );
}
