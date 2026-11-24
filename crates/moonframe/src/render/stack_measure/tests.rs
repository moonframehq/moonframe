use {
    super::Reading,
    gpui_kit::{Bounds, Pixels, point, px, size},
    moonframe_core::{Data, Direction},
};

fn boxes(row: &[(f32, f32)]) -> Vec<Bounds<Pixels>> {
    row.iter()
        .map(|(left, width)| Bounds::new(point(px(*left), px(10.)), size(px(*width), px(20.))))
        .collect()
}

/// A row 300 wide at x = 100, of five 80-wide children with no gap: 400
/// of content, so 100 to scroll.
fn strip(scrolled: f32) -> Reading {
    let bounds = Bounds::new(point(px(100.), px(10.)), size(px(300.), px(20.)));
    let children = boxes(&[
        (100., 80.),
        (180., 80.),
        (260., 80.),
        (340., 80.),
        (420., 80.),
    ]);
    Reading::of(
        Direction::Row,
        bounds,
        point(px(100.), px(0.)),
        point(px(-scrolled), px(0.)),
        &children,
    )
}

#[test]
fn a_row_that_overflows_says_by_how_much_and_what_fits() {
    let at_start = strip(0.);
    assert_eq!(
        (at_start.width, at_start.content_width, at_start.x),
        (300, 400, 0)
    );
    // 100 to 180, 180 to 260, 260 to 340: three end inside the box.
    assert_eq!(at_start.fit, 3);
    // The fourth shows in part.
    assert_eq!((at_start.first, at_start.last), (1, 4));
}

#[test]
fn scrolling_moves_what_shows_but_not_what_fits() {
    let at_end = strip(100.);
    assert_eq!((at_end.x, at_end.fit), (100, 3));
    // Scrolled 100: the box shows 200 to 500 of the row's own coordinates.
    assert_eq!((at_end.first, at_end.last), (2, 5));
}

#[test]
fn a_column_reads_along_its_own_axis_and_an_empty_one_shows_nothing() {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(200.), px(100.)));
    let rows: Vec<Bounds<Pixels>> = (0..4)
        .map(|row| Bounds::new(point(px(0.), px(row as f32 * 40.)), size(px(200.), px(40.))))
        .collect();
    let reading = Reading::of(
        Direction::Column,
        bounds,
        point(px(0.), px(60.)),
        point(px(0.), px(-50.)),
        &rows,
    );
    assert_eq!(
        (reading.height, reading.content_height, reading.y),
        (100, 160, 50)
    );
    assert_eq!((reading.fit, reading.first, reading.last), (2, 2, 4));
    let empty = Reading::of(
        Direction::Column,
        bounds,
        point(px(0.), px(0.)),
        point(px(0.), px(0.)),
        &[],
    );
    assert_eq!((empty.fit, empty.first, empty.last), (0, 0, 0));
}

#[test]
fn half_a_pixel_of_rounding_is_not_overflow() {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(160.), px(20.)));
    let children = boxes(&[(0., 80.), (80., 80.4)]);
    let reading = Reading::of(
        Direction::Row,
        bounds,
        point(px(0.), px(0.)),
        point(px(0.), px(0.)),
        &children,
    );
    assert_eq!((reading.fit, reading.last), (2, 2));
}

#[test]
fn the_message_is_the_mods_own_with_the_reading_added() {
    let msg = Data::Map(
        [("strip".to_owned(), Data::Bool(true))]
            .into_iter()
            .collect(),
    );
    let Data::Map(told) = strip(0.).message(&msg) else {
        panic!("a map")
    };
    assert_eq!(told.get("strip"), Some(&Data::Bool(true)));
    assert_eq!(told.get("content_width"), Some(&Data::Number(400.)));
    assert_eq!(told.get("fit"), Some(&Data::Number(3.)));
}
