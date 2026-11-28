use {
    super::{Kept, first_showing, kept_in_place},
    gpui_kit::px,
    std::time::Instant,
};

fn kept(below: f32, offset: f32) -> Kept {
    Kept {
        key: "row-11".into(),
        below: px(below),
        offset: px(offset),
        seen: Instant::now(),
    }
}

#[test]
fn a_kept_child_that_moved_down_is_followed() {
    // It was 10 below the column's top (at 100) scrolled 200, so laid out at
    // 310; three rows of 40 came above: laid out at 430, 120 further down.
    let row = kept(10.0, -200.0);
    assert_eq!(
        kept_in_place(&row, px(430.0), px(100.0), px(-200.0)),
        Some(px(-320.0))
    );
    // Where it was: nothing to do.
    assert_eq!(kept_in_place(&row, px(310.0), px(100.0), px(-200.0)), None);
}

#[test]
fn a_wheel_since_moves_the_kept_child_with_everything() {
    // Wheeled 50 further down since: it is where the wheel put it, and rows
    // arriving above keep it there.
    let row = kept(10.0, -200.0);
    assert_eq!(kept_in_place(&row, px(310.0), px(100.0), px(-250.0)), None);
    assert_eq!(
        kept_in_place(&row, px(430.0), px(100.0), px(-250.0)),
        Some(px(-370.0))
    );
}

#[test]
fn the_first_child_showing_is_kept_only_while_scrolled_down() {
    let laid = || {
        [("a", 100.0), ("b", 140.0), ("c", 180.0)]
            .map(|(key, top)| (key.to_owned(), px(top), px(40.0)))
    };
    let now = Instant::now();
    assert_eq!(
        first_showing(laid(), (px(100.0), px(300.0)), px(0.0), now),
        None
    );
    // Scrolled 50: "a" (100..140) is gone above, "b" (140..180, now 90..130)
    // shows from 10 above the top.
    let showing = first_showing(laid(), (px(100.0), px(300.0)), px(-50.0), now).unwrap();
    assert_eq!(showing.key, "b");
    assert_eq!(showing.below, px(-10.0));
    assert_eq!(showing.offset, px(-50.0));
}

#[test]
fn a_keyed_child_below_the_box_is_not_kept() {
    // The box shows 100..200; scrolled 50, unkeyed rows fill it and the
    // first keyed child lies at 260..300 (unscrolled 310): not showing.
    let laid = [("far".to_owned(), px(310.0), px(40.0))];
    assert_eq!(
        first_showing(laid, (px(100.0), px(200.0)), px(-50.0), Instant::now()),
        None
    );
}
