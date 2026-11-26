use {
    super::*,
    crate::{ListScroll, ListScrollTo},
};

fn asked(scroll_to: &str) -> Result<Option<ListScrollTo>, String> {
    let source = format!(
        r#"return mod {{ view = function() return ui.list {{ count = 100, row_height = 20, scroll_to = {scroll_to},
            row = function(i) return ui.text {{ tostring(i) }} end }} end }}"#
    );
    match view_of(&source)? {
        Node::List(list) => Ok(list.scroll_to),
        other => panic!("expected a list, got {other:?}"),
    }
}

#[test]
fn a_list_scrolls_to_a_row_or_back_to_a_y() {
    let to = |to, seq| Ok(Some(ListScrollTo { to, seq }));
    assert_eq!(asked("12"), to(ListScroll::Row(12), Data::Nil));
    assert_eq!(
        asked("{ row = 12, seq = 3 }"),
        to(ListScroll::Row(12), Data::Integer(3))
    );
    assert_eq!(asked("{ y = 480 }"), to(ListScroll::Y(480.0), Data::Nil));
    assert_eq!(asked("nil"), Ok(None));
}

#[test]
fn a_list_refuses_a_place_it_cannot_go_to() {
    for (bad, said) in [
        ("0", "a row from 1"),
        ("{ row = 0 }", "a row from 1"),
        ("{ row = 3, y = 10 }", "not both"),
        ("{ seq = 1 }", "takes `row`"),
        ("{ y = -5 }", "y"),
        (r#""top""#, "expected a row"),
    ] {
        let error = asked(bad).unwrap_err();
        assert!(error.contains(said), "{bad}: {error}");
    }
}
