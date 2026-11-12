use {
    super::click,
    moonframe_core::{Action, Data},
};

#[test]
fn a_click_says_what_it_sends_on_one_line() {
    let action = Action::Send {
        to: Some("status".to_owned()),
        msg: Data::map([("open", Data::from("settings")), ("at", Data::Integer(2))]),
    };
    assert_eq!(
        click(&action),
        "a click sends { at = 2, open = \"settings\", } to status's update"
    );
    assert_eq!(
        click(&Action::Window("devtools".to_owned())),
        "a click opens the mod devtools in its own window"
    );
}
