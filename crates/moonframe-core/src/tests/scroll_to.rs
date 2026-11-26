use {super::*, crate::ScrollTo};

fn asked(scroll_to: &str) -> Result<Option<ScrollTo>, String> {
    let source = format!(
        r#"return mod {{ view = function() return ui.column {{ scroll = true, scroll_to = {scroll_to}, "a" }} end }}"#
    );
    match view_of(&source)? {
        Node::Stack(stack) => Ok(stack.scroll_to),
        other => panic!("expected a column, got {other:?}"),
    }
}

#[test]
fn scroll_to_takes_a_child_a_key_or_a_key_asked_again() {
    assert_eq!(asked("3"), Ok(Some(ScrollTo::Child(3))));
    assert_eq!(
        asked(r#""row-7""#),
        Ok(Some(ScrollTo::Key {
            key: "row-7".into(),
            seq: Data::Nil
        }))
    );
    assert_eq!(
        asked(r#"{ key = "row-7", seq = 2 }"#),
        Ok(Some(ScrollTo::Key {
            key: "row-7".into(),
            seq: Data::Integer(2)
        }))
    );
    assert_eq!(asked("nil"), Ok(None));
}

#[test]
fn scroll_to_refuses_what_names_nothing() {
    for (bad, said) in [
        ("0", "positive child index"),
        (r#""two words""#, "an id is"),
        ("{ seq = 1 }", "names the row or column"),
        (r#"{ key = "a", line = 1 }"#, "line"),
        ("true", "a child index, a key"),
    ] {
        let error = asked(bad).unwrap_err();
        assert!(error.contains(said), "{bad}: {error}");
    }
}
