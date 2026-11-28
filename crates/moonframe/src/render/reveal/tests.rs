use {
    super::{MAX_TRIES, Pending, revealed, waits},
    gpui_kit::{Bounds, Pixels, point, px, size},
};

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

const VIEW: (f32, f32) = (100., 200.);

fn shown(top: f32, height: f32, offset: f32) -> f32 {
    let view = bounds(0., VIEW.0, 300., VIEW.1);
    let target = bounds(0., top, 300., height);
    let max = point(px(0.), px(1000.));
    f32::from(revealed(view, target, point(px(0.), px(offset)), max).y)
}

#[test]
fn what_shows_stays_where_it_is() {
    assert_eq!(shown(150., 40., -60.), -60.);
}

#[test]
fn what_is_below_comes_up_to_the_bottom_edge() {
    // 420..460 in a box that ends at 300: 160 more.
    assert_eq!(shown(420., 40., -60.), -220.);
}

#[test]
fn what_is_above_comes_down_to_the_top_edge() {
    assert_eq!(shown(40., 40., -200.), -140.);
}

#[test]
fn something_taller_than_the_box_shows_its_start() {
    assert_eq!(shown(350., 500., 0.), -250.);
}

#[test]
fn it_never_scrolls_past_either_end() {
    assert_eq!(shown(2000., 40., 0.), -1000.);
    let view = bounds(0., 0., 300., 200.);
    let wide = revealed(
        view,
        bounds(400., 0., 50., 20.),
        point(px(0.), px(0.)),
        point(px(0.), px(0.)),
    );
    assert_eq!(wide, point(px(0.), px(0.)));
}

#[test]
fn a_missing_target_draws_again_only_while_the_stack_has_no_size() {
    let pending = Pending::key("row-7".into());
    // Laid out without it: wait for the next draw, whenever that is.
    assert!(!waits(&pending, false));
    // Not laid out yet: draw again, a bounded number of times.
    let again = (0..MAX_TRIES + 10)
        .filter(|_| waits(&pending, true))
        .count();
    assert_eq!(again, MAX_TRIES as usize);
    let child = Pending::child(3);
    child.done.set(true);
    assert!(!waits(&child, true));
}
