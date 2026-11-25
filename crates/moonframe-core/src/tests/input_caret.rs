use {super::*, crate::Caret};

#[test]
fn a_field_takes_a_caret_at_either_end_or_some_bytes_in() {
    let caret = |value: &str| {
        let source = format!(
            "return mod {{ view = function() return ui.input {{ value = 'héllo', caret = {value} }} end }}"
        );
        match view_of(&source) {
            Ok(Node::Input(input)) => Ok(input.caret),
            Ok(_) => panic!("expected an input"),
            Err(error) => Err(error),
        }
    };
    assert_eq!(caret("'end'"), Ok(Some(Caret::End)));
    assert_eq!(caret("'start'"), Ok(Some(Caret::Start)));
    assert_eq!(caret("3"), Ok(Some(Caret::At(3))));
    assert_eq!(caret("nil"), Ok(None));
    for bad in ["'middle'", "-1", "1.5"] {
        assert!(caret(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_caret_lands_on_a_character_inside_the_text() {
    let text = "héllo";
    assert_eq!(Caret::Start.offset(text), 0);
    assert_eq!(Caret::End.offset(text), text.len());
    // Inside the two bytes of `é`, it moves back before it.
    assert_eq!(Caret::At(2).offset(text), 1);
    assert_eq!(Caret::At(3).offset(text), 3);
    assert_eq!(Caret::At(99).offset(text), text.len());
}
