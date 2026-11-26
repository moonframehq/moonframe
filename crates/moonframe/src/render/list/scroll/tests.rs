use {
    super::{Anchor, Built, Pad, anchor, anchored, target},
    moonframe_core::{ListScroll, RowHeights},
};

const ROW: RowHeights = RowHeights::Same(20.0);

fn built<'a>(start: usize, keys: &[&'a str]) -> Built<'a> {
    Built {
        start,
        keys: keys.iter().map(|key| Some(*key)).collect(),
    }
}

#[test]
fn a_row_comes_into_view_only_as_far_as_needed() {
    // Rows 20 high, a box 100 high scrolled 200: rows 11 to 15 show.
    let to = |row| {
        target(
            ListScroll::Row(row),
            200.0,
            Some(100.0),
            Pad::default(),
            &ROW,
            1000,
        )
    };
    assert_eq!(to(12), 200.0);
    assert_eq!(to(5), 80.0);
    assert_eq!(to(30), 500.0);
    // Past the last row: the last.
    assert_eq!(to(5000), 19_900.0);
    // A y is gone to as it is.
    assert_eq!(
        target(
            ListScroll::Y(4321.0),
            200.0,
            None,
            Pad::default(),
            &ROW,
            1000
        ),
        4321.0
    );
}

#[test]
fn a_row_taller_than_the_box_shows_its_top() {
    let tall = RowHeights::each([20.0, 300.0, 20.0]);
    assert_eq!(
        target(
            ListScroll::Row(2),
            0.0,
            Some(100.0),
            Pad::default(),
            &tall,
            3
        ),
        20.0
    );
}

#[test]
fn the_first_row_showing_is_followed_while_scrolled_down() {
    let rows = built(10, &["k10", "k11", "k12", "k13"]);
    assert_eq!(anchor(0.0, Pad::default(), &rows, &ROW, 100), None);
    assert_eq!(
        anchor(225.0, Pad::default(), &rows, &ROW, 100),
        Some(Anchor {
            key: "k11".into(),
            index: 11,
            top: 220.0,
            count: 100
        })
    );
    // A row without a key is not followed.
    let plain = Built {
        start: 10,
        keys: vec![None, None],
    };
    assert_eq!(anchor(225.0, Pad::default(), &plain, &ROW, 100), None);
}

#[test]
fn rows_arriving_above_keep_the_followed_row_in_its_place() {
    let followed = Anchor {
        key: "k11".into(),
        index: 11,
        top: 220.0,
        count: 100,
    };
    // Three rows came above: it is the 15th now, 60 further down.
    let now = built(10, &["new", "new2", "new3", "k10", "k11"]);
    assert_eq!(anchored(&followed, 225.0, &now, &ROW, 103), Some(285.0));
    // Where it was: nothing to do.
    let same = built(10, &["k10", "k11"]);
    assert_eq!(anchored(&followed, 225.0, &same, &ROW, 100), None);
    // Out of the rows built after many arrived: as many rows down.
    let far = built(10, &["a", "b"]);
    assert_eq!(anchored(&followed, 225.0, &far, &ROW, 150), Some(1225.0));
    // Gone, with fewer rows: left where it is.
    assert_eq!(anchored(&followed, 225.0, &far, &ROW, 90), None);
}

#[test]
fn padding_above_the_rows_moves_where_they_are() {
    let pad = Pad {
        top: 20.0,
        bottom: 0.0,
    };
    // Row 50 under 20 of padding ends at 1020: 920 down in a box 100 high.
    assert_eq!(
        target(ListScroll::Row(50), 0.0, Some(100.0), pad, &ROW, 1000),
        920.0
    );
    // Scrolled 245, 225 into the rows: row 12 shows first.
    let rows = built(10, &["k10", "k11", "k12"]);
    assert_eq!(
        anchor(245.0, pad, &rows, &ROW, 100).map(|anchor| anchor.index),
        Some(11)
    );
}
