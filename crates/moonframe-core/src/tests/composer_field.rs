//! A chat's composer: `enter = "submit"` and `history` on `ui.input`.

use {super::*, crate::Input};

fn input(element: &str) -> Result<Input, String> {
    match view_of(&format!(
        "return mod {{ view = function() return {element} end }}"
    ))? {
        Node::Input(input) => Ok(input),
        other => Err(format!("not an input: {other:?}")),
    }
}

#[test]
fn enter_submits_a_multiline_field_when_asked() {
    let field = input("ui.input { multiline = true, enter = 'submit', submit = 'send' }").unwrap();
    assert!(field.enter_submits);
    assert!(
        !input("ui.input { multiline = true }")
            .unwrap()
            .enter_submits
    );
    assert!(
        !input("ui.input { multiline = true, enter = 'newline' }")
            .unwrap()
            .enter_submits
    );
    let one_line = input("ui.input { enter = 'submit' }").unwrap_err();
    assert!(
        one_line.contains("`enter` goes with `multiline = true`"),
        "{one_line}"
    );
    let unknown = input("ui.input { multiline = true, enter = 'send' }").unwrap_err();
    assert!(unknown.contains("\"submit\" or \"newline\""), "{unknown}");
}

#[test]
fn history_keeps_the_newest_entries_oldest_first() {
    let field = input("ui.input { history = { 'one', 'two' } }").unwrap();
    assert_eq!(field.history, ["one", "two"]);
    let field = input(
        "ui.input { history = (function() local h = {} for i = 1, 250 do h[i] = tostring(i) end \
         return h end)() }",
    )
    .unwrap();
    assert_eq!(field.history.len(), Input::MAX_HISTORY);
    assert_eq!(field.history.first().map(String::as_str), Some("51"));
    assert_eq!(field.history.last().map(String::as_str), Some("250"));
    assert!(input("ui.input { history = 'one' }").is_err());
}
