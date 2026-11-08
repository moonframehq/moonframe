use {
    super::*,
    crate::{MapProvider, node::Map},
};

fn map_of(props: &str) -> Result<Map, String> {
    let source = format!("return mod {{ view = function() return ui.map {{ {props} }} end }}");
    match view_of(&source)? {
        Node::Map(map) => Ok(map),
        other => Err(format!("expected a map, got {other:?}")),
    }
}

#[test]
fn a_map_reads_its_provider_place_pins_and_routes() {
    let map = map_of(
        r#"provider = "carto-light", center = { 37.79, -122.40 }, zoom = 14,
           pins = { { lat = 37.787, lon = -122.404, label = "1" }, { lat = 37.79, lon = -122.39 } },
           routes = { { points = { 37.787, -122.404, 37.79, -122.39 }, color = "success", dashed = true } },
           move = "moved", select = { msg = "pin" }, grow = true"#,
    )
    .unwrap();
    assert_eq!(map.provider, MapProvider::CartoLight);
    assert_eq!(map.center, Some([37.79, -122.40]));
    assert_eq!(map.zoom, Some(14.0));
    assert_eq!(map.bearing, None);
    assert_eq!(map_of("bearing = -45").unwrap().bearing, Some(-45.0));
    assert_eq!(map.pins.len(), 2);
    assert_eq!(map.pins[0].label.as_deref(), Some("1"));
    assert!(!map.pins[0].pulse);
    let here =
        map_of(r##"pins = { { lat = 37.33, lon = -122.03, color = "#0a84ff", pulse = true } }"##)
            .unwrap();
    assert!(here.pins[0].pulse);
    assert!(!map.glide && map_of("glide = true").unwrap().glide);
    assert_eq!(map.routes[0].points, vec![[37.787, -122.404], [
        37.79, -122.39
    ]]);
    assert!(map.routes[0].dashed && map.routes[0].color.is_some());
    assert!(map.moved.is_some() && map.select.is_some() && map.grow);
    // OpenStreetMap is the default, and needs no key.
    assert_eq!(map_of("").unwrap().provider, MapProvider::OpenStreetMap);
}

#[test]
fn a_map_refuses_what_it_cannot_show() {
    for (props, expected) in [
        (r#"provider = "google""#, "unknown `google`"),
        (r#"provider = "stadia-smooth""#, "needs an API key"),
        ("center = { 91, 0 }", "`center` is { lat, lon }"),
        ("routes = { { points = { 1, 2, 3 } } }", "lat, lon pairs"),
        ("routes = { { color = \"accent\" } }", "one of them"),
        (
            "routes = { { points = { 1, 2, 3, 4 }, via = { 1, 2, 3, 4 } } }",
            "one of them",
        ),
        (
            "routes = { { points = { 1, 2, 3, 4 }, mode = \"car\" } }",
            "how a `via` route travels",
        ),
        (
            r#"pins = { { lat = 1, lon = 2, shape = "star" } }"#,
            "unknown `star`",
        ),
        (
            "routes = { { points = { 1, 2, 3, 200 } } }",
            "is not a place",
        ),
        (
            r#"pins = { { lat = 1, lon = 2, label = "long" } }"#,
            "three characters",
        ),
        (
            r#"key = "has space", provider = "maptiler-streets""#,
            "without spaces",
        ),
        ("zoom = 40", "zoom"),
        ("pins = { { lat = 1, lon = 2, pulse = 1 } }", "pulse"),
        ("glide = 1", "glide"),
        ("bearing = 400", "bearing"),
    ] {
        let error = map_of(props).unwrap_err();
        assert!(error.contains(expected), "{props}: {error}");
    }
    let routed = map_of(
        r#"routes = { { via = { 37.78, -122.40, 37.79, -122.39 }, mode = "bike" } }, routed = "done",
           pins = { { lat = 1, lon = 2, icon = "coffee", shape = "balloon", selected = true } }"#,
    )
    .unwrap();
    let route = &routed.routes[0];
    assert_eq!((route.via.len(), route.points.len()), (2, 0));
    assert_eq!(route.mode, crate::TravelMode::Bike);
    assert_eq!(route.places(), &route.via[..]);
    assert!(routed.routed.is_some());
    let pin = &routed.pins[0];
    assert_eq!(pin.shape, Some(crate::PinShape::Balloon));
    assert!(pin.selected && pin.icon.is_some());
    let keyed = map_of(r#"provider = "maptiler-streets", key = "abc123""#).unwrap();
    assert_eq!(keyed.key.as_deref(), Some("abc123"));
}

#[test]
fn a_turned_map_frames_along_its_own_edges() {
    // Two places a long way apart east to west: north up the box is wide
    // enough, turned a quarter the box is tall enough for less.
    let map = map_of("pins = { { lat = 0, lon = -40 }, { lat = 0, lon = 40 } }").unwrap();
    let (center, north_up) = map.framing(1000.0, 200.0, 0.0, 0.0);
    let (turned_center, turned) = map.framing(1000.0, 200.0, 0.0, 90.0);
    assert!(turned < north_up, "{turned} < {north_up}");
    assert!((center[0] - turned_center[0]).abs() < 1e-9);
    assert!((center[1] - turned_center[1]).abs() < 1e-9);
}
