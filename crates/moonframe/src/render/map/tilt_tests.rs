use {
    super::{Camera, Tilt},
    crate::render::map::view::Frame,
    moonframe_core::world,
};

fn frame(bearing: f64) -> Frame {
    Frame {
        center: world(35.68, 139.76),
        zoom: 15.0,
        size: (800.0, 600.0),
        bearing,
    }
}

/// gpui-maps' camera looking where `frame` does, tilted `pitch` degrees:
/// its zoom counts 512-pixel tiles, so it is one less, and its bearing is
/// in radians.
fn camera(frame: &Frame, pitch: f32) -> Camera {
    let [lat, lon] = frame.middle();
    Camera {
        center_lon: lon,
        center_lat: lat,
        zoom: frame.zoom - 1.0,
        pitch: pitch.to_radians(),
        bearing: (frame.bearing as f32).to_radians(),
        viewport_width_px: frame.size.0 as f32,
        viewport_height_px: frame.size.1 as f32,
        fov_y: Camera::DEFAULT_FOV_Y,
    }
}

const PLACES: [[f64; 2]; 4] = [[35.685, 139.76], [35.68, 139.77], [35.675, 139.755], [
    35.683, 139.752,
]];

#[test]
fn a_flat_camera_puts_each_place_where_the_frame_does() {
    for bearing in [0.0, 30.0, 290.0] {
        let frame = frame(bearing);
        let tilt = Tilt(Some(camera(&frame, 0.0)));
        for [lat, lon] in PLACES {
            let flat = frame.at(lat, lon);
            let Some(drawn) = tilt.at(&frame, lat, lon) else {
                panic!("{lat},{lon} is on screen");
            };
            assert!(
                (flat.0 - drawn.0).abs() < 0.5 && (flat.1 - drawn.1).abs() < 0.5,
                "bearing {bearing}: {flat:?} by the frame, {drawn:?} by the camera"
            );
        }
    }
}

#[test]
fn a_tilted_map_keeps_its_middle_and_draws_the_far_side_nearer_it() {
    let frame = frame(0.0);
    let tilt = Tilt(Some(camera(&frame, 50.0)));
    let [lat, lon] = frame.middle();
    let Some(middle) = tilt.at(&frame, lat, lon) else {
        panic!("the middle is on screen");
    };
    assert!(
        (middle.0 - 400.0).abs() < 0.5 && (middle.1 - 300.0).abs() < 0.5,
        "{middle:?}"
    );
    // North is up the box, away from the eye: foreshortened.
    let flat = frame.at(lat + 0.003, lon);
    let Some(far) = tilt.at(&frame, lat + 0.003, lon) else {
        panic!("just north of the middle is on screen");
    };
    assert!(
        far.1 < 300.0 && far.1 > flat.1,
        "flat {flat:?}, tilted {far:?}"
    );
}

#[test]
fn a_tilted_route_is_its_corners_and_shows_one_world() {
    let frame = frame(0.0);
    let tilt = Tilt(Some(camera(&frame, 40.0)));
    let corners: Vec<_> = PLACES
        .iter()
        .filter_map(|&[lat, lon]| tilt.at(&frame, lat, lon))
        .collect();
    assert_eq!(tilt.route(&frame, &PLACES), [corners]);
    assert_eq!(
        tilt.copies(&frame, (-5000.0, 0.0), (5000.0, 600.0), 0.0),
        0..=0
    );
}

#[test]
fn a_tilted_route_breaks_where_it_passes_the_eye() {
    let frame = frame(0.0);
    let tilt = Tilt(Some(camera(&frame, 60.0)));
    let [lat, lon] = frame.middle();
    // Far south, behind a camera looking north.
    let behind = [lat - 60.0, lon];
    assert_eq!(tilt.at(&frame, behind[0], behind[1]), None);
    let route = [PLACES[0], PLACES[1], behind, PLACES[2], PLACES[3]];
    let at = |[lat, lon]: [f64; 2]| tilt.at(&frame, lat, lon);
    let pieces = tilt.route(&frame, &route);
    assert_eq!(pieces, [
        [at(PLACES[0]), at(PLACES[1])].map(Option::unwrap).to_vec(),
        [at(PLACES[2]), at(PLACES[3])].map(Option::unwrap).to_vec(),
    ]);
    // Past the eye at either end leaves no empty piece.
    assert_eq!(tilt.route(&frame, &[behind, PLACES[0], behind]).len(), 1);
}
