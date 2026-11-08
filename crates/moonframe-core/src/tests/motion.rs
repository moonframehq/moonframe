use super::*;

#[test]
fn transitions_take_css_words_or_a_table() {
    use {
        crate::{Curve, Transition, TransitionKind},
        std::time::Duration,
    };
    let sheet = r#"return style { rules = {
      pane = { transition = "slide 220ms ease-out" },
      [".fading"] = { transition = "0.3s fade cubic-bezier(0.2, 0, 0, 1)" },
      [".table"] = { transition = { kind = "slide-fade", duration = 150, curve = "linear" } },
      [".bare"] = { transition = "fade" },
      [".rising"] = { transition = "rise 220ms" },
      [".late"] = { transition = "fade 150ms ease-out 40ms" },
      [".late-table"] = { transition = { kind = "fade", duration = 150, delay = "0.05s" } },
      [".staggered"] = { transition = "fade 150ms", delay = 60, clip = true },
    } }"#;
    let dir = app(&[]);
    write(dir.path(), "style.luau", sheet);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let transition = |class: Option<&str>| {
        let look = crate::Look {
            class: class.map(str::to_owned).into_iter().collect(),
            ..Default::default()
        };
        workspace
            .stylesheet()
            .resolve(Part::Pane, &look, false, &Rules::default())
            .base
            .transition
            .unwrap()
    };
    assert_eq!(transition(None), Transition {
        kind: TransitionKind::Slide,
        duration: Duration::from_millis(220),
        curve: Curve::EASE_OUT,
        delay: Duration::ZERO,
    });
    assert_eq!(transition(Some("fading")), Transition {
        kind: TransitionKind::Fade,
        duration: Duration::from_millis(300),
        curve: Curve::CubicBezier(0.2, 0.0, 0.0, 1.0),
        delay: Duration::ZERO,
    });
    assert_eq!(transition(Some("table")), Transition {
        kind: TransitionKind::SlideFade,
        duration: Duration::from_millis(150),
        curve: Curve::Linear,
        delay: Duration::ZERO,
    });
    assert_eq!(transition(Some("rising")).kind, TransitionKind::Rise);
    // A second duration is the delay, as in CSS.
    assert_eq!(transition(Some("late")).delay, Duration::from_millis(40));
    assert_eq!(
        transition(Some("late")).duration,
        Duration::from_millis(150)
    );
    assert_eq!(
        transition(Some("late-table")).delay,
        Duration::from_millis(50)
    );
    // `delay` on its own sits beside the transition, for staggered items.
    let staggered = crate::Look {
        class: vec!["staggered".to_owned()],
        ..Default::default()
    };
    assert_eq!(
        workspace
            .stylesheet()
            .resolve(Part::Pane, &staggered, false, &Rules::default())
            .base
            .delay,
        Some(Duration::from_millis(60))
    );
    assert_eq!(
        workspace
            .stylesheet()
            .resolve(Part::Pane, &staggered, false, &Rules::default())
            .base
            .clip,
        Some(true)
    );
    // Missing parts take the defaults: the `normal` speed, `ease`.
    assert_eq!(
        transition(Some("bare")).duration,
        Duration::from_millis(220)
    );
    assert_eq!(transition(Some("bare")).curve, Curve::EASE);

    for (value, expected) in [
        ("'slide 220ms bouncy'", "unknown `bouncy`"),
        ("'slide 20s'", "expected 0 to 10000"),
        ("'cubic-bezier(1, 2, 3)'", "four numbers"),
        ("'fade 1ms 2ms 3ms'", "at most two durations"),
        ("'cubic-bezier(2, 0, 0, 1)'", "x values must be from 0 to 1"),
        ("'cubic-bezier(0, 0, 1, 1'", "missing its `)`"),
        ("{ speed = 3 }", "unknown key `speed`"),
        ("true", "expected a transition"),
    ] {
        let dir = app(&[]);
        let sheet = format!("return style {{ rules = {{ pane = {{ transition = {value} }} }} }}");
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
fn curves_run_from_zero_to_one_and_ease_as_named() {
    use crate::Curve;
    for curve in [
        Curve::Linear,
        Curve::EASE,
        Curve::EASE_IN,
        Curve::EASE_OUT,
        Curve::EASE_IN_OUT,
    ] {
        assert!(curve.at(0.0).abs() < 1e-3, "{curve:?} starts at 0");
        assert!((curve.at(1.0) - 1.0).abs() < 1e-3, "{curve:?} ends at 1");
        let steps: Vec<f32> = (0..=20).map(|step| curve.at(step as f32 / 20.0)).collect();
        assert!(
            steps.windows(2).all(|pair| pair[1] >= pair[0] - 1e-4),
            "{curve:?} never goes back"
        );
    }
    // Ease-out is ahead of linear early on; ease-in is behind.
    assert!(Curve::EASE_OUT.at(0.3) > 0.3);
    assert!(Curve::EASE_IN.at(0.3) < 0.3);
    assert!((Curve::EASE_IN_OUT.at(0.5) - 0.5).abs() < 1e-3);
    // Out of range time is clamped.
    assert_eq!(Curve::Linear.at(2.0), 1.0);
}

#[test]
fn speeds_name_durations_and_animation_speed_scales_them() {
    use std::time::Duration;
    let sheet = r#"return style {
      speeds = { snappy = "90ms", slow = 600, lazy = "1s" },
      animation_speed = 2,
      rules = {
        pane = { transition = "slide snappy ease-out" },
        [".fast"] = { transition = "fade fast" },
        [".slow"] = { transition = { kind = "fade", duration = "slow" } },
        [".ms"] = { transition = { duration = 300 } },
        [".bare"] = { transition = "slide" },
      },
    }"#;
    let dir = app(&[]);
    write(dir.path(), "style.luau", sheet);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let duration = |class: Option<&str>| {
        let look = crate::Look {
            class: class.map(str::to_owned).into_iter().collect(),
            ..Default::default()
        };
        workspace
            .stylesheet()
            .resolve(Part::Pane, &look, false, &Rules::default())
            .base
            .transition
            .unwrap()
            .duration
    };
    // Every duration runs at twice the speed.
    assert_eq!(duration(None), Duration::from_millis(45));
    assert_eq!(duration(Some("fast")), Duration::from_millis(60)); // the default 120ms
    assert_eq!(duration(Some("slow")), Duration::from_millis(300)); // redefined as 600ms
    assert_eq!(duration(Some("ms")), Duration::from_millis(150));
    assert_eq!(duration(Some("bare")), Duration::from_millis(110)); // `normal`, 220ms

    // A speed of 0 turns animations off.
    let dir = app(&[]);
    write(
        dir.path(),
        "style.luau",
        "return style { animation_speed = 0, rules = { pane = { transition = 'slide 1s' } } }",
    );
    let workspace = Workspace::open(dir.path(), false);
    let off = workspace
        .stylesheet()
        .resolve(Part::Pane, &Default::default(), false, &Rules::default())
        .base
        .transition
        .unwrap();
    assert_eq!(off.duration, Duration::ZERO);

    for (sheet, expected) in [
        ("speeds = { ease = 100 }", "not a transition kind or curve"),
        ("speeds = { slide = 100 }", "not a transition kind or curve"),
        ("speeds = { quick = 'soon' }", "expected milliseconds"),
        ("speeds = { quick = 20000 }", "expected 0 to 10000"),
        ("animation_speed = 20", "expected 0 to 10"),
        (
            "rules = { pane = { transition = 'slide brisk' } }",
            "unknown `brisk`",
        ),
    ] {
        let dir = app(&[]);
        write(
            dir.path(),
            "style.luau",
            &format!("return style {{ {sheet} }}"),
        );
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace.style_error().map(ToString::to_string);
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{sheet}: {error:?}"
        );
    }
}

#[test]
fn a_component_is_animated_by_a_transition_or_what_the_stylesheet_moves() {
    let component = |styles: &str, build: &str| {
        format!(
            "return component {{ title = \"x\", when = \"w\", props = {{}}, example = {{}},\n  styles = {{ {styles} }},\n  build = function() return {build} end }}\n"
        )
    };
    let dir = app(&[]);
    write(
        dir.path(),
        "components/a-moving.luau",
        &component(
            r#"[".m"] = { transition = "fade fast" }"#,
            "ui.row { class = \"m\" }",
        ),
    );
    write(
        dir.path(),
        "components/b-sheet.luau",
        &component(
            "",
            "ui.column { ui.overlay { open = true, content = ui.text \"s\" } }",
        ),
    );
    write(
        dir.path(),
        "components/c-still.luau",
        &component(r#"[".s"] = { radius = 4 }"#, "ui.text \"s\""),
    );
    write(
        dir.path(),
        "components/d-off.luau",
        &component(r#"[".o"] = { transition = "none" }"#, "ui.text \"o\""),
    );
    let workspace = Workspace::open(dir.path(), false);
    let animated: Vec<_> = workspace
        .components()
        .iter()
        .map(|component| (component.name.as_str(), component.animated()))
        .collect();
    assert_eq!(animated, [
        ("a-moving", true),
        ("b-sheet", true),
        ("c-still", false),
        ("d-off", false),
    ]);
}

#[test]
fn icons_can_spin() {
    let source = r#"return mod { view = function()
      return ui.row { ui.icon { name = "loader-circle", spin = true }, ui.icon "cpu" } end }"#;
    let Ok(Node::Stack(row)) = view_of(source) else {
        panic!("no row");
    };
    let spins: Vec<bool> = row
        .children
        .iter()
        .map(|child| matches!(child, Node::Icon(icon) if icon.spin))
        .collect();
    assert_eq!(spins, [true, false]);
    let bad = r#"return mod { view = function() return ui.icon { name = "cpu", spin = 1 } end }"#;
    let error = view_of(bad).err().unwrap_or_default();
    assert!(
        error.contains("icon.spin: expected true or false"),
        "{error}"
    );
}

#[test]
fn a_tiny_animation_speed_is_refused_rather_than_crashing() {
    use std::time::Duration;
    let dir = app(&[]);
    write(
        dir.path(),
        "style.luau",
        r#"return style { animation_speed = 1e-30, rules = { panel = { transition = "fade" } } }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.style_error().unwrap().to_string();
    assert!(error.contains("animation_speed"), "{error}");
    // Off is still allowed.
    write(
        dir.path(),
        "style.luau",
        "return style { animation_speed = 0 }",
    );
    let mut workspace = workspace;
    workspace.reload();
    assert!(workspace.style_error().is_none());
    // And scaling never panics, whatever the speed.
    let names = crate::style::Names {
        animation_speed: 1e-30,
        ..crate::style::Names::NONE
    };
    assert_eq!(names.scaled(Duration::from_secs(1)), Duration::MAX);
}
