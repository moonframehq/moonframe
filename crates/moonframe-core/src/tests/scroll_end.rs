use super::*;

#[test]
fn a_scrolling_stack_keeps_its_ends() {
    let ends = |source: &str| {
        let Node::Stack(stack) = view_of(source).unwrap() else {
            panic!("expected a column");
        };
        stack.ends
    };
    let chat = r#"return mod { view = function() return ui.column { scroll = true, follow = true,
        at_end = "end", scroll_end = { seq = 2 }, "a" } end }"#;
    let Some(chat) = ends(chat) else {
        panic!("ends kept")
    };
    assert_eq!(chat.at_end, Some(Data::from("end")));
    assert_eq!(
        chat.scroll_end,
        Some(Data::Map([("seq".to_owned(), Data::Integer(2))].into()))
    );
    let plain = r#"return mod { view = function() return ui.column { scroll = true, "a" } end }"#;
    assert_eq!(ends(plain), None);
}

#[test]
fn ends_need_a_stack_that_scrolls() {
    for key in ["at_end = 'end'", "scroll_end = 1"] {
        let still =
            format!("return mod {{ view = function() return ui.column {{ {key}, 'a' }} end }}");
        let error = view_of(&still).unwrap_err();
        assert!(error.contains("give it `scroll`"), "{key}: {error}");
    }
}
