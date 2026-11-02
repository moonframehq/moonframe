//! A row's `pointer`, `wheel`, `drop`, `range` and `cursor`, a frame's
//! `area`, a zoomed `ui.image`, and a mod's `keys`.

use {
    super::*,
    crate::{Area, Axis, Cursor, Held, Pointer, PointerPhase, Pressed, Range, Travel},
    std::collections::BTreeMap,
};

fn row_of(source: &str) -> Result<crate::Stack, String> {
    match view_of(&format!(
        "return mod {{ view = function() return {source} end }}"
    ))? {
        Node::Stack(stack) => Ok(stack),
        other => Err(format!("expected a row, got {other:?}")),
    }
}

fn travel(x: f32, dx: f32) -> Travel {
    Travel {
        x,
        y: 0.5,
        dx,
        dy: 0.0,
        width: 200.0,
        height: 20.0,
    }
}

fn fields(msg: Data) -> BTreeMap<String, Data> {
    match msg {
        Data::Map(fields) => fields,
        other => panic!("expected a table, got {other:?}"),
    }
}

#[test]
fn a_row_reads_its_pointer_keys() {
    let row = row_of(
        r#"ui.row { pointer = { set = "contrast" }, to = "editor", hover = true,
            range = { min = -50, max = 50, step = 1, value = 12 }, wheel = "zoom",
            drop = "dropped", cursor = "ew-resize",
            area = { left = 0.25, top = 0, width = 0.5 } }"#,
    )
    .unwrap();
    let pointer = row.pointer.as_deref().unwrap();
    assert_eq!(pointer.to.as_deref(), Some("editor"));
    assert!(pointer.hover);
    assert_eq!(pointer.cursor, Some(Cursor::EwResize));
    assert_eq!(pointer.wheel, Some(Data::from("zoom")));
    assert_eq!(pointer.drop, Some(Data::from("dropped")));
    assert_eq!(
        pointer.range,
        Some(Range {
            min: -50.0,
            max: 50.0,
            step: Some(1.0),
            value: 12.0,
            speed: None,
            axis: Axis::X,
        })
    );
    assert_eq!(
        row.frame.area,
        Some(Area {
            left: Some(0.25),
            top: Some(0.0),
            width: Some(0.5),
            height: None,
        })
    );
    // A row without them hears clicks only.
    assert!(row_of("ui.row {}").unwrap().pointer.is_none());
}

#[test]
fn pointer_keys_that_cannot_work_are_refused() {
    for (source, problem) in [
        (
            "ui.row { range = { min = 0, max = 1 } }",
            "give `pointer` too",
        ),
        ("ui.row { hover = true }", "give `pointer` too"),
        (
            "ui.row { pointer = 1, range = { min = 0 } }",
            "needs `min` and `max`",
        ),
        (
            "ui.row { pointer = 1, range = { min = 0, max = 1, speed = 2 } }",
            "give the current `value`",
        ),
        ("ui.row { pointer = 1, cursor = \"hand\" }", "hand"),
        ("ui.row { to = \"x\" }", "`to` goes with"),
    ] {
        let error = row_of(source).unwrap_err();
        assert!(error.contains(problem), "{source}: {error}");
    }
    // `to` goes with `pointer`, `wheel` or `drop` alone.
    assert!(row_of("ui.row { wheel = 1, to = \"x\" }").is_ok());
}

#[test]
fn a_press_message_carries_where_and_how_far() {
    let pointer = Pointer {
        press: Some(Data::Map(BTreeMap::from([(
            "set".to_owned(),
            Data::from("contrast"),
        )]))),
        ..Pointer::default()
    };
    let held = Held {
        shift: true,
        ..Held::default()
    };
    let msg = fields(
        pointer
            .press_message(PointerPhase::Move, &travel(0.1, 12.5), "left", 1, held)
            .unwrap(),
    );
    assert_eq!(msg["set"], Data::from("contrast"));
    assert_eq!(msg["phase"], Data::from("move"));
    assert_eq!(msg["x"], Data::Number(0.1));
    assert_eq!(msg["dx"], Data::Number(12.5));
    assert_eq!(msg["width"], Data::Integer(200));
    assert_eq!(msg["button"], Data::from("left"));
    assert_eq!(msg["shift"], Data::Bool(true));
    assert!(!msg.contains_key("alt"), "only the keys held");
    assert!(!msg.contains_key("value"), "no range, no value");
    // A plain message is kept as `msg`.
    let plain = Pointer {
        press: Some(Data::from("pad")),
        ..Pointer::default()
    };
    let msg = plain.press_message(
        PointerPhase::Hover,
        &travel(0.5, 0.0),
        "",
        0,
        Held::default(),
    );
    let msg = fields(msg.unwrap());
    assert_eq!(msg["msg"], Data::from("pad"));
    assert!(!msg.contains_key("button"), "a hover has no button");
}

#[test]
fn a_range_turns_a_drag_into_a_value() {
    let slider = Range {
        min: -50.0,
        max: 50.0,
        step: Some(1.0),
        value: 0.0,
        speed: None,
        axis: Axis::X,
    };
    assert_eq!(slider.value(&travel(0.5, 0.0)), 0.0);
    assert_eq!(slider.value(&travel(0.62, 0.0)), 12.0);
    // Past the ends it stays at them.
    assert_eq!(slider.value(&travel(1.4, 0.0)), 50.0);
    assert_eq!(slider.value(&travel(-0.3, 0.0)), -50.0);
    // Steps leave no float dust.
    let fine = Range {
        min: 0.0,
        max: 1.0,
        step: Some(0.1),
        ..slider
    };
    assert_eq!(fine.value(&travel(0.31, 0.0)), 0.3);
    // A scrub counts pixels from the value at the press.
    let scrub = Range {
        min: 10.0,
        max: 10_000.0,
        step: Some(1.0),
        value: 300.0,
        speed: Some(2.0),
        axis: Axis::X,
    };
    assert_eq!(scrub.value(&travel(0.0, 25.0)), 350.0);
    assert_eq!(scrub.value(&travel(0.0, -1000.0)), 10.0);
    // Up is more on a vertical range.
    let fader = Range {
        axis: Axis::Y,
        ..slider
    };
    let high = Travel {
        y: 0.0,
        ..travel(0.0, 0.0)
    };
    assert_eq!(fader.value(&high), 50.0);
    // The value comes in the message.
    let pointer = Pointer {
        press: Some(Data::from("set")),
        range: Some(slider),
        ..Pointer::default()
    };
    let msg = pointer.press_message(
        PointerPhase::Down,
        &travel(0.75, 0.0),
        "left",
        1,
        Held::default(),
    );
    assert_eq!(fields(msg.unwrap())["value"], Data::Integer(25));
}

#[test]
fn wheel_and_drop_messages_say_what_happened() {
    let pointer = Pointer {
        wheel: Some(Data::from("wheel")),
        drop: Some(Data::from("dropped")),
        ..Pointer::default()
    };
    let wheel = fields(
        pointer
            .wheel_message(&travel(0.5, -3.0), 0.1, Held {
                secondary: true,
                ..Held::default()
            })
            .unwrap(),
    );
    assert_eq!(wheel["zoom"], Data::Number(0.1));
    assert_eq!(wheel["dx"], Data::Integer(-3));
    assert_eq!(wheel["secondary"], Data::Bool(true));
    let dropped = fields(
        pointer
            .drop_message(&["/scans/a.tif".into()], &travel(0.5, 0.0))
            .unwrap(),
    );
    assert_eq!(
        dropped["paths"],
        Data::List(vec![Data::from("/scans/a.tif")])
    );
}

#[test]
fn an_image_zooms_and_carries_an_overlay() {
    let source = r#"return mod { view = function()
        return ui.image { src = "scan.jpg", zoom = 1.5, center = { 0.25, 0.75 },
            measure = "measured", to = "editor",
            overlay = { ui.column { area = { left = 0.1, top = 0.1, width = 0.5, height = 0.5 } } } }
    end }"#;
    let Node::Image(image) = view_of(source).unwrap() else {
        panic!("expected an image");
    };
    assert_eq!(image.zoom, Some(1.5));
    assert_eq!(image.center, Some([0.25, 0.75]));
    assert_eq!(image.measure, Some(Data::from("measured")));
    assert_eq!(image.to.as_deref(), Some("editor"));
    assert_eq!(image.overlay.len(), 1);
    let bad = r#"return mod { view = function()
        return ui.image { src = "a.png", center = { 0.5 } }
    end }"#;
    assert!(view_of(bad).unwrap_err().contains("two fractions"));
}

#[test]
fn a_mod_takes_keys_anywhere() {
    let source = r#"return mod {
        keys = { ["secondary-1"] = "histogram", ["alt-z"] = { zoom = true } },
        view = function() return ui.text "x" end,
    }"#;
    let dir = app(&[
        ("tools", source),
        (
            "plain",
            "return mod { view = function() return ui.text \"y\" end }",
        ),
    ]);
    let workspace = Workspace::open(dir.path(), false);
    let keyed: Vec<&str> = workspace.mod_keys().map(|(name, _)| name).collect();
    assert_eq!(keyed, ["tools"]);
    let (_, keys) = workspace.mod_keys().next().unwrap();
    let pressed = Pressed {
        key: "1",
        text: Some("1"),
        ctrl: false,
        alt: false,
        shift: false,
        cmd: true,
    };
    assert_eq!(
        keys.message(None, &pressed, true),
        Some(&Data::from("histogram"))
    );
    // Keys that type text are refused, as on overlays.
    let typing =
        r#"return mod { keys = { j = "down" }, view = function() return ui.text "x" end }"#;
    let dir = app(&[("typing", typing)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("typing")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("types text"), "{error}");
}
