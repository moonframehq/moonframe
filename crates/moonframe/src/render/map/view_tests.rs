use {
    super::{
        Frame, Pace, STILL_ZOOMING, TELL_EVERY, Tell, Turn, ahead, current, frame, glide_zoom,
        heading, late, pinch, settled, tell, world, zoom,
    },
    gpui_kit::{Point, TouchPhase, px},
    moonframe_core::{Look, Map, MapProvider},
    std::time::{Duration, Instant},
};

/// A map the view drives, following `move` with `center` and `zoom`.
fn following(center: [f64; 2], at: f32) -> Map {
    Map {
        look: Look::default(),
        provider: MapProvider::OpenStreetMap,
        key: None,
        center: Some(center),
        zoom: Some(at),
        bearing: None,
        pins: Vec::new(),
        routes: Vec::new(),
        moved: None,
        select: None,
        routed: None,
        to: None,
        width: None,
        height: None,
        grow: true,
        glide: true,
    }
}

#[test]
fn a_zoom_glide_keeps_the_place_under_the_pointer_all_the_way() {
    let from = Frame {
        center: world(37.79, -122.40),
        zoom: 14.0,
        size: (800.0, 600.0),
        bearing: 0.0,
    };
    let to = from.zoomed(1.0, 100.0, 50.0, 20.0);
    let under = from.world_at(100.0, 50.0);
    for progress in [0.25, 0.5, 0.75] {
        let there = from
            .toward(&to, Pace::Zoom.eased(progress))
            .world_at(100.0, 50.0);
        assert!((there.0 - under.0).abs() < 1e-9 && (there.1 - under.1).abs() < 1e-9);
    }
}

#[test]
fn a_click_glides_and_a_second_adds_to_the_first() {
    let (key, middle) = ("clicked", Point::new(px(400.), px(300.)));
    let map = following([37.79, -122.40], 14.0);
    let now = Instant::now();
    let drawn = frame(&map, key, (800.0, 600.0), now, |pace| pace).0;
    glide_zoom(key, drawn, 1.0, middle, 20.0);
    let on_the_way = frame(&map, key, (800.0, 600.0), Instant::now(), |pace| pace).0;
    assert!(on_the_way.zoom < 14.5, "it glides: {}", on_the_way.zoom);
    let second = glide_zoom(key, on_the_way, 1.0, middle, 20.0);
    assert!((second.zoom - 16.0).abs() < 1e-9);
    assert!(heading(key).is_some_and(|heading| heading.zoom < 16.0));
    let ended = now + Duration::from_secs(1);
    let arrived = frame(&map, key, (800.0, 600.0), ended, |pace| pace).0;
    assert!((arrived.zoom - 16.0).abs() < 1e-9);
    assert_eq!(heading(key), None);
}

#[test]
fn a_pinch_let_go_while_moving_coasts_on_and_a_held_one_does_not() {
    let middle = Point::new(px(400.), px(300.));
    let map = following([37.79, -122.40], 14.0);
    let (key, held) = ("coasting", "held");
    let drawn = frame(&map, key, (800.0, 600.0), Instant::now(), |pace| pace).0;
    pinch(key, drawn, 0.05, TouchPhase::Started, middle, 20.0);
    let before = pinch(key, drawn, 0.05, TouchPhase::Moved, middle, 20.0);
    let released = pinch(key, drawn, 0.0, TouchPhase::Ended, middle, 20.0);
    assert!(released.zoom > before.zoom + 0.1, "{}", released.zoom);
    assert!(heading(key).is_none(), "the next frame sets out");
    frame(&map, key, (800.0, 600.0), Instant::now(), |pace| pace);
    assert!(heading(key).is_some());

    let drawn = frame(&map, held, (800.0, 600.0), Instant::now(), |pace| pace).0;
    pinch(held, drawn, 0.0, TouchPhase::Started, middle, 20.0);
    let released = pinch(held, drawn, 0.0, TouchPhase::Ended, middle, 20.0);
    assert_eq!(released.zoom, drawn.zoom);
}

#[test]
fn the_fingers_take_a_glide_from_where_it_shows() {
    let (key, middle) = ("caught", Point::new(px(400.), px(300.)));
    let map = following([37.79, -122.40], 14.0);
    let drawn = frame(&map, key, (800.0, 600.0), Instant::now(), |pace| pace).0;
    glide_zoom(key, drawn, 2.0, middle, 20.0);
    let on_the_way = frame(&map, key, (800.0, 600.0), Instant::now(), |pace| pace).0;
    assert_eq!(current(key, on_the_way), on_the_way);
}

#[test]
fn a_view_following_a_gesture_does_not_hold_it_back() {
    let (key, step) = ("following", 0.05);
    let mut map = following([37.79, -122.40], 14.0);
    let mut now = Instant::now();
    let (mut drawn, _) = frame(&map, key, (800.0, 600.0), now, |pace| pace);
    for _ in 0..40 {
        let from = current(key, drawn);
        let zoomed = zoom(key, from, step, Point::new(px(400.), px(300.)), 20.0);
        now += Duration::from_millis(8);
        // What `tell` sends, and the view gives back; between two tellings
        // the view still shows the older place.
        let told = match tell(key, zoomed, now) {
            Tell::Now => Some(zoomed),
            Tell::After(wait) if wait <= Duration::from_millis(8) => late(key, now),
            _ => None,
        };
        if let Some(told) = told {
            let (center, at) = told.told();
            map = following(center, at as f32);
        }
        drawn = frame(&map, key, (800.0, 600.0), now, |pace| pace).0;
    }
    assert!(
        (drawn.zoom - 16.0).abs() < 0.02,
        "40 steps of {step} from 14 reached {}",
        drawn.zoom
    );
}

#[test]
fn a_moving_map_tells_its_view_seldom_and_always_ends_with_the_latest() {
    let key = "telling";
    let map = following([37.79, -122.40], 14.0);
    let now = Instant::now();
    let (drawn, _) = frame(&map, key, (800.0, 600.0), now, |pace| pace);
    let at = |zoom| Frame { zoom, ..drawn };
    assert_eq!(tell(key, at(14.1), now), Tell::Now);
    let soon = now + Duration::from_millis(30);
    assert_eq!(
        tell(key, at(14.2), soon),
        Tell::After(TELL_EVERY - Duration::from_millis(30))
    );
    assert_eq!(tell(key, at(14.3), soon), Tell::Held);
    let due = now + TELL_EVERY;
    assert_eq!(late(key, due).map(|frame| frame.zoom), Some(14.3));
    assert_eq!(late(key, due), None);
    assert_eq!(tell(key, at(14.4), due + TELL_EVERY), Tell::Now);
}

#[test]
fn an_order_from_the_view_cancels_what_was_held_back() {
    let key = "ordered";
    let map = following([37.79, -122.40], 14.0);
    let now = Instant::now();
    let (drawn, _) = frame(&map, key, (800.0, 600.0), now, |pace| pace);
    let _ = tell(
        key,
        Frame {
            zoom: 14.1,
            ..drawn
        },
        now,
    );
    let _ = tell(
        key,
        Frame {
            zoom: 14.2,
            ..drawn
        },
        now,
    );
    let ordered = following([48.86, 2.33], 12.0);
    let _ = frame(&ordered, key, (800.0, 600.0), now, |pace| pace);
    assert_eq!(late(key, now + TELL_EVERY), None);
}

#[test]
fn a_zoom_looks_ahead_to_the_next_level_while_it_goes_on() {
    let key = "ahead";
    let map = following([37.79, -122.40], 14.2);
    let (drawn, _) = frame(&map, key, (800.0, 600.0), Instant::now(), |pace| pace);
    let level = |zoom: f64| zoom.round() as u8;
    let middle = Point::new(px(400.), px(300.));

    assert!(ahead(key, Instant::now(), level).is_none(), "not zooming");
    let zoomed = zoom(key, drawn, 0.05, middle, 20.0);
    let there = ahead(key, Instant::now(), level).map(|frame| frame.zoom);
    assert!(there.is_some_and(|zoom| level(zoom) == 15 && zoom < zoomed.zoom + 0.4));

    let back = zoom(key, zoomed, -0.05, middle, 20.0);
    let there = ahead(key, Instant::now(), level).map(|frame| frame.zoom);
    assert!(there.is_some_and(|zoom| level(zoom) == 13 && zoom < back.zoom));

    let later = Instant::now() + STILL_ZOOMING * 2;
    assert!(ahead(key, later, level).is_none(), "it stopped");
}

#[test]
fn zooming_keeps_the_place_under_the_pointer() {
    let frame = Frame {
        center: world(37.79, -122.40),
        zoom: 14.0,
        size: (800.0, 600.0),
        bearing: 0.0,
    };
    let under = frame.world_at(100.0, 50.0);
    let zoomed = frame.zoomed(1.5, 100.0, 50.0, 20.0);
    let after = zoomed.world_at(100.0, 50.0);
    assert!((under.0 - after.0).abs() < 1e-12 && (under.1 - after.1).abs() < 1e-12);
    assert_eq!(zoomed.zoom, 15.5);
}

#[test]
fn a_glide_eases_to_its_end_and_refuses_long_trips() {
    let from = Frame {
        center: world(37.79, -122.40),
        zoom: 14.0,
        size: (800.0, 600.0),
        bearing: 0.0,
    };
    let to = Frame {
        center: world(37.80, -122.41),
        zoom: 15.0,
        ..from
    };
    assert_eq!(from.toward(&to, 0.0), from);
    assert!(from.toward(&to, 1.0).same_as(&to));
    let half = from.toward(&to, Pace::Trip.eased(0.5));
    assert!(half.zoom > 14.5 && half.zoom < 15.0);
    assert!(from.glides_to(&to));
    let far = Frame {
        center: world(40.71, -74.0),
        ..to
    };
    assert!(!from.glides_to(&far));
    assert!(from.same_as(&from.toward(&from, Pace::Trip.eased(0.5))));
}

#[test]
fn a_glide_turns_the_short_way_round_past_north() {
    let from = Frame {
        center: world(37.79, -122.40),
        zoom: 14.0,
        size: (800.0, 600.0),
        bearing: 350.0,
    };
    let to = Frame {
        bearing: 10.0,
        ..from
    };
    assert!(!from.same_as(&to));
    assert!(from.toward(&to, 1.0).same_as(&to));
    let half = from.toward(&to, 0.2).bearing;
    assert!(!(10.0..350.0).contains(&half), "{half}");
}

#[test]
fn a_glide_takes_the_short_way_over_the_dateline() {
    let from = Frame {
        center: world(0.0, 179.99),
        zoom: 10.0,
        size: (800.0, 600.0),
        bearing: 0.0,
    };
    let to = Frame {
        center: world(0.0, -179.99),
        ..from
    };
    assert!(from.glides_to(&to));
    let half = from.toward(&to, 0.2);
    assert!((half.center.0 - to.center.0).abs() < 1e-3);
}

#[test]
fn a_place_lands_where_the_frame_says() {
    let frame = Frame {
        center: world(37.79, -122.40),
        zoom: 14.0,
        size: (800.0, 600.0),
        bearing: 0.0,
    };
    let (left, top) = frame.at(37.79, -122.40);
    assert!((left - 400.0).abs() < 1e-3 && (top - 300.0).abs() < 1e-3);
    let [lat, lon] = frame.middle();
    assert!((lat - 37.79).abs() < 1e-9 && (lon + 122.40).abs() < 1e-9);
}

#[test]
fn overlays_follow_tiles_across_the_dateline_and_repeated_worlds() {
    let frame = Frame {
        center: world(0.0, 179.9),
        zoom: 10.0,
        size: (800.0, 600.0),
        bearing: 0.0,
    };
    let (x, y) = frame.at(0.0, -179.9);
    let expected = 400.0 + 0.2 / 360.0 * frame.span();
    assert!((f64::from(x) - expected).abs() < 0.001);
    assert_eq!(y, 300.0);
    for worlds in [-3.0, 2.0] {
        let panned = Frame {
            center: (frame.center.0 + worlds, frame.center.1),
            ..frame
        };
        assert_eq!(panned.at(0.0, -179.9), (x, y));
    }
    let route = frame.route(&[[0.0, 179.9], [0.0, -179.9]]);
    assert_eq!(route[0], (400.0, 300.0));
    assert_eq!(route[1], (x, y));
}

#[test]
fn a_dateline_route_draws_at_both_edges_without_crossing_greenwich() {
    let frame = Frame {
        center: world(0.0, 0.0),
        zoom: 1.0,
        size: (512.0, 400.0),
        bearing: 0.0,
    };
    let route = frame.route(&[[0.0, 179.0], [0.0, -179.0]]);
    assert!(route[0].0 > 500.0 && route[1].0 > 512.0);
    assert!(route[1].0 - route[0].0 < 3.0);
    let copies: Vec<_> = frame.copies(route[0], route[1], 4.0).collect();
    assert_eq!(copies, [-1, 0]);
    // At zoom zero, all three visible copies of a pin are drawn.
    let wide = Frame {
        zoom: 0.0,
        size: (800.0, 400.0),
        ..frame
    };
    let (x, _) = wide.at(0.0, 0.0);
    let at = wide.at(0.0, 0.0);
    assert_eq!(at.0, x);
    assert_eq!(wide.copies(at, at, 14.0).collect::<Vec<_>>(), [-1, 0, 1]);
}

fn turned(bearing: f64) -> Frame {
    Frame {
        center: world(37.79, -122.40),
        zoom: 14.0,
        size: (800.0, 600.0),
        bearing,
    }
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn east_up_puts_north_on_the_left() {
    let frame = turned(90.0);
    let (left, top) = frame.at(37.80, -122.40);
    assert!(left < 300.0 && (top - 300.0).abs() < 0.01, "{left}, {top}");
    let (left, top) = frame.at(37.79, -122.39);
    assert!((left - 400.0).abs() < 0.01 && top < 200.0, "{left}, {top}");
}

#[test]
fn a_place_and_its_pixel_agree_at_any_bearing() {
    for bearing in [0.0, 17.0, 90.0, 133.5, 270.0, 359.0] {
        let frame = turned(bearing);
        let (left, top) = frame.at(37.80, -122.41);
        let (x, y) = frame.world_at(f64::from(left), f64::from(top));
        let (want_x, want_y) = world(37.80, -122.41);
        assert!((x - want_x).abs() < 1e-7 && (y - want_y).abs() < 1e-7);
    }
}

#[test]
fn a_drag_keeps_the_place_under_the_pointer_when_turned() {
    for bearing in [0.0, 30.0, 200.0] {
        let frame = turned(bearing);
        let under = frame.world_at(100.0, 50.0);
        let dragged = frame.dragged(35.0, -20.0);
        let after = dragged.world_at(135.0, 30.0);
        assert!((under.0 - after.0).abs() < 1e-9 && (under.1 - after.1).abs() < 1e-9);
    }
}

#[test]
fn zooming_keeps_the_place_under_the_pointer_when_turned() {
    let frame = turned(75.0);
    let under = frame.world_at(100.0, 50.0);
    let after = frame.zoomed(1.5, 100.0, 50.0, 20.0).world_at(100.0, 50.0);
    assert!((under.0 - after.0).abs() < 1e-12 && (under.1 - after.1).abs() < 1e-12);
}

#[test]
fn the_world_box_covers_every_corner_of_a_turned_box() {
    let frame = turned(45.0);
    let (low, high) = frame.world_box();
    for corner in [(0.0, 0.0), (800.0, 0.0), (0.0, 600.0), (800.0, 600.0)] {
        let (x, y) = frame.world_at(corner.0, corner.1);
        assert!(low.0 <= x && x <= high.0 && low.1 <= y && y <= high.1);
    }
    let north = turned(0.0).world_box();
    assert!(high.0 - low.0 > north.1.0 - north.0.0);
}

#[test]
fn world_copies_of_a_place_lie_along_the_turned_east() {
    // A quarter turn: copies repeat up and down the box, not across it.
    let frame = Frame {
        center: world(0.0, 0.0),
        zoom: 0.0,
        size: (300.0, 800.0),
        bearing: 90.0,
    };
    let at = frame.at(0.0, 0.0);
    assert_eq!(frame.copies(at, at, 14.0).collect::<Vec<_>>(), [-1, 0, 1]);
    let (x, y) = frame.copy_offset(1);
    assert!(close((f64::from(x), f64::from(y)), (0.0, -frame.span())));
    // Off the box's side, no copy along its length brings it in.
    let aside = (at.0 + 1000.0, at.1);
    assert_eq!(frame.copies(aside, aside, 14.0).count(), 0);
}

#[test]
fn a_pinch_s_small_turn_does_not_tip_the_map() {
    let mut turn = Turn::default();
    assert_eq!(turn.by(3.0), 0.0);
    assert_eq!(turn.by(-2.0), 0.0);
    assert!(!turn.following);
    assert_eq!(turn.by(5.5), 0.0);
    assert!(turn.following);
    assert_eq!(turn.by(1.5), 1.5);
}

#[test]
fn a_turn_left_close_to_north_settles_there() {
    assert_eq!(settled(3.0), 0.0);
    assert_eq!(settled(358.0), 0.0);
    assert_eq!(settled(10.0), 10.0);
    assert_eq!(settled(350.0), 350.0);
}
