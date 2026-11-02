use super::*;

#[test]
fn radius_takes_one_value_or_each_corner() {
    let corners = |value: &str| {
        let dir = app(&[]);
        let sheet = format!("return style {{ rules = {{ panel = {{ radius = {value} }} }} }}");
        write(dir.path(), "style.luau", &sheet);
        let workspace = Workspace::open(dir.path(), false);
        match workspace.style_error() {
            Some(error) => Err(error.to_string()),
            None => Ok(workspace
                .stylesheet()
                .resolve(
                    Part::Panel,
                    &crate::Look::default(),
                    false,
                    &Rules::default(),
                )
                .base
                .radius),
        }
    };
    let each = |top_left, top_right, bottom_right, bottom_left| {
        Ok(Some(Corners {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }))
    };
    assert_eq!(corners("6"), Ok(Some(Corners::all(6.0))));
    assert_eq!(corners("{ 8, 8, 0, 0 }"), each(8.0, 8.0, 0.0, 0.0));
    assert_eq!(corners("{ 4, 0 }"), each(4.0, 0.0, 4.0, 0.0));
    assert_eq!(Corners::all(3.0).uniform(), Some(3.0));
    assert_eq!(
        Corners {
            top_left: 1.0,
            ..Corners::all(0.0)
        }
        .uniform(),
        None
    );
    for (value, expected) in [
        ("{ 1, 2, 3 }", "1, 2, or 4 values"),
        ("{ 1, top = 2 }", "only numbers"),
        ("-1", "from 0 to 1000"),
        ("'round'", "a radius in pixels"),
    ] {
        let error = corners(value).err().unwrap_or_default();
        assert!(error.contains(expected), "{value}: {error}");
    }
}

#[test]
fn shadows_take_css_strings_tables_and_presets() {
    let sheet = r##"return style {
      colors = { glow = "#ff0000" },
      rules = {
        panel = { shadow = "0 4px 12px -2px #00000033, inset 0 1 0 glow/50" },
        pane = { shadow = { y = 2, blur = 6, color = "accent/30" } },
        row = { shadow = { "1 1", { x = -1, inset = true } } },
        column = { shadow = "none" },
      },
    }"##;
    let dir = app(&[]);
    write(dir.path(), "style.luau", sheet);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let sheet = workspace.stylesheet();
    let shadow = |part| {
        sheet
            .resolve(part, &Default::default(), false, &Rules::default())
            .base
            .shadow
            .unwrap()
    };
    let black = |alpha| Color {
        paint: Paint::Fixed(Shade {
            light: 0x0000_0040,
            dark: 0x0000_0040,
        }),
        alpha,
    };
    let Shadow::Layers(panel) = shadow(Part::Panel) else {
        panic!("no layers");
    };
    assert_eq!(panel.len(), 2);
    assert_eq!(
        (
            panel[0].x,
            panel[0].y,
            panel[0].blur,
            panel[0].spread,
            panel[0].inset
        ),
        (0.0, 4.0, 12.0, -2.0, false)
    );
    assert_eq!(
        panel[0].color.paint,
        Paint::Fixed(Shade {
            light: 0x0000_0033,
            dark: 0x0000_0033
        })
    );
    assert!(panel[1].inset);
    assert_eq!(panel[1].color.alpha, 0.5);
    assert_eq!(
        shadow(Part::Pane),
        Shadow::Layers(vec![ShadowLayer {
            x: 0.0,
            y: 2.0,
            blur: 6.0,
            spread: 0.0,
            color: Color {
                paint: Paint::Token(Token::Accent),
                alpha: 0.3
            },
            inset: false,
        }])
    );
    let Shadow::Layers(row) = shadow(Part::Row) else {
        panic!("no layers");
    };
    assert_eq!((row[0].x, row[0].color), (1.0, black(1.0)));
    assert!(row[1].inset && row[1].x == -1.0);
    assert_eq!(shadow(Part::Column), Shadow::Preset(ShadowPreset::None));

    for (value, expected) in [
        ("'4'", "2 to 4 lengths"),
        ("'1 2 3 4 5'", "2 to 4 lengths"),
        ("'1 2 #000 3'", "must come before the color"),
        ("'1 2 chartreuse'", "unknown color `chartreuse`"),
        ("'1 2 -3'", "blur cannot be negative"),
        ("'0 1e9 4 #000'", "expected -1000 to 1000"),
        ("{ depth = 3 }", "unknown key `depth`"),
        ("true", "a CSS shadow"),
    ] {
        let dir = app(&[]);
        let sheet = format!("return style {{ rules = {{ panel = {{ shadow = {value} }} }} }}");
        write(dir.path(), "style.luau", &sheet);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace.style_error().map(ToString::to_string);
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{value}: {error:?}"
        );
    }
}

#[test]
fn lines_and_hatches_are_read() {
    let dir = app(&[(
        "chart",
        r#"return mod { view = function() return ui.row {
          ui.line { points = { 1, 3, 2 }, baseline = 2, area = true, dots = "last", height = 40 },
          ui.hatch { gap = 5, grow = true },
        } end }"#,
    )]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let Some(Node::Stack(row)) = workspace.slot("chart").unwrap().view() else {
        panic!("no row");
    };
    let Node::Line(line) = &row.children[0] else {
        panic!("{:?}", row.children[0]);
    };
    assert_eq!(line.points, [1.0, 3.0, 2.0]);
    assert_eq!(
        (line.baseline, line.area, line.dots),
        (Some(2.0), true, Some(crate::Dots::Last))
    );
    assert!(matches!(&row.children[1], Node::Hatch(hatch) if hatch.gap == 5.0 && hatch.grow));

    let dir = app(&[(
        "bad",
        r#"return mod { view = function() return ui.line { points = { 1, "x" } } end }"#,
    )]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        !workspace.slot("bad").unwrap().broken().is_empty()
            || workspace.slot("bad").unwrap().error().is_some()
    );
}

#[test]
fn looks_come_from_the_stylesheet_and_layer_like_colors() {
    let shows_look = r#"return mod { view = function(state, app)
        return ui.text(app.looks.tabs or "none")
    end }"#;
    let builtin = app(&[("look", shows_look)]);
    write(
        builtin.path(),
        "style.luau",
        r#"return style { looks = { tabs = "classic", menus = "flat" } }"#,
    );
    let text = |workspace: &Workspace| match workspace.slot("look").and_then(crate::Slot::view) {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("{other:?}"),
    };
    let workspace = Workspace::open(builtin.path(), false);
    assert_eq!(text(&workspace), "classic");

    // The user's stylesheet changes one look and keeps the others.
    let user = tempfile::tempdir().unwrap();
    write(
        user.path(),
        "style.luau",
        r#"return style { looks = { tabs = "square" } }"#,
    );
    let roots = crate::Roots::new([builtin.path().to_owned(), user.path().to_owned()]);
    let workspace = Workspace::open(roots, false);
    assert_eq!(text(&workspace), "square");
    assert_eq!(
        workspace
            .stylesheet()
            .looks
            .get("menus")
            .map(String::as_str),
        Some("flat")
    );

    // A look becomes a class, so it is a plain name.
    write(
        user.path(),
        "style.luau",
        r#"return style { looks = { tabs = "big tabs" } }"#,
    );
    let mut workspace = workspace;
    workspace.reload();
    let error = workspace.style_error().unwrap().to_string();
    assert!(error.contains("looks"), "{error}");
}

#[test]
fn gradients_take_strings_and_tables() {
    let sheet = r##"return style {
      rules = {
        panel = { gradient = "#ffa53a, accent/50" },
        row = { gradient = "90deg, #000, #fff" },
        column = { gradient = { from = "accent", to = "#ff0000", angle = -90 } },
      },
    }"##;
    let dir = app(&[]);
    write(dir.path(), "style.luau", sheet);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let sheet = workspace.stylesheet();
    let gradient = |part| {
        sheet
            .resolve(part, &Default::default(), false, &Rules::default())
            .base
            .gradient
            .unwrap()
    };
    let panel = gradient(Part::Panel);
    assert_eq!(panel.angle, 180.0);
    assert_eq!(panel.to.alpha, 0.5);
    assert_eq!(gradient(Part::Row).angle, 90.0);
    let column = gradient(Part::Column);
    assert_eq!(column.angle, 270.0);
    assert_eq!(column.from.paint, Paint::Token(Token::Accent));

    let bad = app(&[]);
    write(
        bad.path(),
        "style.luau",
        r##"return style { rules = { row = { gradient = "#000" } } }"##,
    );
    let workspace = Workspace::open(bad.path(), false);
    assert!(!workspace.problems().is_empty());
}
