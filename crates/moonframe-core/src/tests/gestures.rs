//! A row's trackpad gestures and the messages they send.

use {
    super::*,
    crate::{GesturePhase, Gestures, Held, Swipe, Travel},
    std::collections::BTreeMap,
};

const AT: Travel = Travel {
    x: 0.25,
    y: 0.5,
    dx: 0.0,
    dy: 0.0,
    width: 200.0,
    height: 100.0,
};

fn gestures() -> Gestures {
    Gestures {
        rotate: Some(Data::from("turned")),
        smart_zoom: Some(Data::from("tapped")),
        swipe: Some(Data::from("swiped")),
        force: Some(Data::from("forced")),
    }
}

fn fields(msg: Option<Data>) -> BTreeMap<String, Data> {
    match msg.unwrap() {
        Data::Map(fields) => fields,
        other => panic!("expected a table, got {other:?}"),
    }
}

#[test]
fn a_row_reads_its_gestures() {
    let Node::Stack(row) = view_of(
        r#"return mod { view = function() return ui.row {
            rotate = "turned", smart_zoom = "tapped", swipe = "swiped", force = "forced", to = "other",
        } end }"#,
    )
    .unwrap() else {
        panic!("expected a row");
    };
    let pointer = row.pointer.as_deref().unwrap();
    assert_eq!(pointer.gestures, gestures());
    assert_eq!(pointer.to.as_deref(), Some("other"));
    assert!(!pointer.is_empty());
}

#[test]
fn each_gesture_says_what_happened_and_where() {
    let gestures = gestures();
    let rotate = fields(gestures.rotate_message(&AT, 12.345, GesturePhase::Move, Held::default()));
    assert_eq!(rotate["angle"], Data::Number(12.35));
    assert_eq!(rotate["phase"], Data::from("move"));
    assert_eq!(rotate["x"], Data::Number(0.25));
    assert!(!rotate.contains_key("dx"));

    let swipe = fields(gestures.swipe_message(&AT, Swipe::Back, Held::default()));
    assert_eq!(swipe["direction"], Data::from("back"));

    let tap = fields(gestures.smart_zoom_message(
        &AT,
        Held {
            shift: true,
            ..Held::default()
        },
    ));
    assert_eq!(tap["width"], Data::Integer(200));
    assert_eq!(tap["shift"], Data::Bool(true));

    let force = fields(gestures.force_message(&AT, 0.5, Held::default()));
    assert_eq!(force["pressure"], Data::Number(0.5));
}

#[test]
fn without_a_listener_a_gesture_sends_nothing() {
    let none = Gestures::default();
    assert!(none.is_empty());
    assert!(
        none.rotate_message(&AT, 1.0, GesturePhase::End, Held::default())
            .is_none()
    );
    assert!(none.smart_zoom_message(&AT, Held::default()).is_none());
    assert!(
        none.swipe_message(&AT, Swipe::Up, Held::default())
            .is_none()
    );
    assert!(none.force_message(&AT, 1.0, Held::default()).is_none());
}
