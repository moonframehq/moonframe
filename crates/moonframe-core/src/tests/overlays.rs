use super::*;

#[test]
fn popovers_hold_an_anchor_and_floating_content() {
    use crate::{Anchor, Side};
    let source = r#"return mod {
      view = function(_, app)
        return ui.popover {
          open = true, dismiss = "close", side = "bottom", align = "end", width = 300,
          content = ui.text(`cpu {app.system.cpu}`),
          ui.row { msg = "toggle", ui.text "anchor" },
        }
      end,
    }"#;
    let dir = app(&[("pop", source)]);
    let mut workspace = Workspace::open(dir.path(), false);
    // `app.system` is the host's to share; until it does, the view fails.
    assert!(workspace.slot("pop").unwrap().error().is_some());
    workspace.set_host(
        "system",
        Data::Map([("cpu".to_owned(), Data::Number(0.5))].into()),
    );
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let Some(Node::Popover(popover)) = workspace.slot("pop").unwrap().view() else {
        panic!("no popover");
    };
    assert!(popover.open);
    assert_eq!(popover.dismiss, Some(Data::from("close")));
    assert_eq!((popover.side, popover.align), (Side::Bottom, Anchor::End));
    assert_eq!(popover.width, Some(300.0));
    let Node::Text(text) = popover.content.as_ref() else {
        panic!("no content");
    };
    assert_eq!(text.text, "cpu 0.5");
    // A stack with `msg` is clickable as a whole.
    let Node::Stack(anchor) = &popover.anchor[0] else {
        panic!("no anchor");
    };
    assert_eq!(
        anchor.action,
        Some(Action::Send {
            to: None,
            msg: Data::from("toggle")
        })
    );

    let missing = "return mod { view = function() return ui.popover { open = true } end }";
    let dir = app(&[("missing", missing)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("missing")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("popover.content: missing"), "{error}");
}

#[test]
fn controls_take_a_tooltip_a_context_message_and_text_to_copy() {
    let source = r#"return mod { view = function()
  return ui.row {
    tooltip = "Open", context = { menu = "row" }, to = "other", copy = "abc",
    ui.button { "b", tooltip = "Run", context = "menu" },
  }
end }"#;
    let dir = app(&[("extras", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("extras").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(row)) = slot.view() else {
        panic!("expected a row");
    };
    assert_eq!(row.action, None);
    assert_eq!(row.extras.tooltip.as_deref(), Some("Open"));
    assert_eq!(row.extras.copy.as_deref(), Some("abc"));
    let Some(Action::Send { to, .. }) = &row.extras.context else {
        panic!("expected a context message");
    };
    assert_eq!(to.as_deref(), Some("other"));
    let Node::Button(button) = &row.children[0] else {
        panic!("expected a button");
    };
    assert_eq!(button.extras.tooltip.as_deref(), Some("Run"));
}

#[test]
fn overlays_float_content_with_a_placement_backdrop_and_keys() {
    use crate::{Chord, Placement};
    let source = r#"return mod {
      view = function()
        return ui.overlay {
          open = true, dismiss = "close", place = "right", backdrop = true,
          width = 360, height = 200, class = "sheet",
          keys = { down = "next", ["cmd-k"] = { palette = true }, ["shift-tab"] = "back" },
          content = ui.column { ui.text "Settings" },
        }
      end,
    }"#;
    let Ok(Node::Overlay(overlay)) = view_of(source) else {
        panic!("no overlay");
    };
    assert!(overlay.open && overlay.backdrop);
    assert_eq!(overlay.dismiss, Some(Data::from("close")));
    assert_eq!(overlay.place, Placement::Right);
    assert_eq!((overlay.width, overlay.height), (Some(360.0), Some(200.0)));
    assert_eq!(overlay.look.class, ["sheet"]);
    assert!(matches!(overlay.content.as_ref(), Node::Stack(_)));
    // Sorted by name, so the same view reads the same.
    let chords: Vec<&Chord> = overlay.keys.0.iter().map(|(chord, _)| chord).collect();
    assert_eq!(chords[0], &Chord::parse("cmd-k").unwrap());
    assert_eq!(chords[1], &Chord::parse("down").unwrap());
    assert_eq!(chords[2], &Chord::parse("shift-tab").unwrap());
    assert_eq!(
        overlay.keys.0[0].1,
        Data::Map([("palette".to_owned(), Data::Bool(true))].into())
    );

    // Defaults: closed, centered, no backdrop, no keys.
    let plain = r#"return mod { view = function()
      return ui.overlay { content = ui.text "hi" } end }"#;
    let Ok(Node::Overlay(overlay)) = view_of(plain) else {
        panic!("no overlay");
    };
    assert!(!overlay.open && !overlay.backdrop);
    assert_eq!(overlay.place, Placement::Center);
    assert!(overlay.keys.is_empty() && overlay.dismiss.is_none());

    // Popovers take keys too.
    let popover = r#"return mod { view = function()
      return ui.popover { open = true, keys = { up = "up" }, content = ui.text "x", "anchor" } end }"#;
    let Ok(Node::Popover(popover)) = view_of(popover) else {
        panic!("no popover");
    };
    assert_eq!(popover.keys.0.len(), 1);

    for (props, expected) in [
        (r#"open = true"#, "overlay.content: missing"),
        (
            r#"place = "middle", content = ui.text "x""#,
            "overlay.place: unknown `middle`; expected one of center, top",
        ),
        (
            r#"anchor = 1, content = ui.text "x""#,
            "overlay: unknown key `anchor`",
        ),
        (
            r#"width = -1, content = ui.text "x""#,
            "overlay.width: expected 0 to",
        ),
        (
            r#"keys = "escape", content = ui.text "x""#,
            "overlay.keys: expected a table",
        ),
        (
            r#"keys = { "escape" }, content = ui.text "x""#,
            "overlay.keys: keys are key names",
        ),
        (
            r#"keys = { ["hyper-k"] = 1 }, content = ui.text "x""#,
            r#"overlay.keys["hyper-k"]: unknown modifier `hyper`"#,
        ),
        (
            r#"keys = { ["ctrl-K"] = 1 }, content = ui.text "x""#,
            r#"overlay.keys["ctrl-K"]: write `shift-k` for `K`"#,
        ),
        (
            r#"keys = { esc = 1 }, content = ui.text "x""#,
            r#"overlay.keys["esc"]: unknown key `esc`"#,
        ),
        (
            r#"keys = { ["ctrl-"] = 1 }, content = ui.text "x""#,
            "missing the key after the modifiers",
        ),
        (
            r#"keys = { ["ctrl-ctrl-k"] = 1 }, content = ui.text "x""#,
            "`ctrl` twice",
        ),
        (
            r#"keys = { ["ctrl-shift-k"] = 1, ["shift-ctrl-k"] = 2 }, content = ui.text "x""#,
            "the same key as",
        ),
        (
            r#"keys = { up = function() end }, content = ui.text "x""#,
            r#"overlay.keys["up"]: a function cannot leave its module"#,
        ),
    ] {
        let source =
            format!("return mod {{ view = function() return ui.overlay {{ {props} }} end }}");
        let error = view_of(&source).err().unwrap_or_default();
        assert!(error.contains(expected), "{props}: {error}");
    }
}

#[test]
fn key_names_match_keystrokes_on_each_platform() {
    use crate::{Chord, Keys, Pressed};
    let key = |key, ctrl, alt, shift, cmd| Pressed {
        key,
        text: None,
        ctrl,
        alt,
        shift,
        cmd,
    };
    let matches =
        |name: &str, pressed: Pressed, mac| Chord::parse(name).unwrap().matches(&pressed, mac);
    assert!(matches(
        "escape",
        key("escape", false, false, false, false),
        true
    ));
    assert!(!matches(
        "escape",
        key("escape", false, false, true, false),
        true
    ));
    assert!(matches(
        "shift-tab",
        key("tab", false, false, true, false),
        false
    ));
    assert!(matches(
        "ctrl-alt-shift-p",
        key("p", true, true, true, false),
        false
    ));
    assert!(matches("cmd-k", key("k", false, false, false, true), false));
    // `secondary` is ⌘ on macOS and Ctrl elsewhere.
    assert!(matches(
        "secondary-k",
        key("k", false, false, false, true),
        true
    ));
    assert!(!matches(
        "secondary-k",
        key("k", true, false, false, false),
        true
    ));
    assert!(matches(
        "secondary-k",
        key("k", true, false, false, false),
        false
    ));
    assert!(!matches(
        "secondary-k",
        key("k", false, false, false, true),
        false
    ));
    // Minus, function keys, and a symbol typed with shift.
    assert!(matches("cmd--", key("-", false, false, false, true), true));
    assert!(matches("f12", key("f12", false, false, false, false), true));
    assert!(Chord::parse("f25").is_err());
    let question = Pressed {
        text: Some("?"),
        ..key("/", false, true, true, false)
    };
    assert!(matches("alt-?", question, true));
    assert!(!matches("alt-/", question, true));
    // What a key types is never taken without ctrl, alt or cmd, so an
    // overlay cannot read what goes to a terminal behind it.
    for typed in ["a", "shift-a", "-", "?", "7", "space", "shift-space"] {
        let error = Chord::parse(typed).unwrap_err();
        assert!(error.contains("types text"), "{typed}: {error}");
    }

    // Escape sends `dismiss` unless a key takes it.
    let dismiss = Data::from("close");
    let escape = key("escape", false, false, false, false);
    let none = Keys::default();
    assert_eq!(none.message(Some(&dismiss), &escape, true), Some(&dismiss));
    assert_eq!(none.message(None, &escape, true), None);
    let own = Keys(vec![(Chord::parse("escape").unwrap(), Data::from("back"))]);
    assert_eq!(
        own.message(Some(&dismiss), &escape, true),
        Some(&Data::from("back"))
    );
    let enter = key("enter", false, false, false, false);
    assert_eq!(own.message(Some(&dismiss), &enter, true), None);
}

#[test]
fn overlay_parts_are_selectors() {
    let dir = app(&[]);
    write(
        dir.path(),
        "style.luau",
        r##"return style { rules = {
          overlay = { radius = 12, transition = "slide-fade fast" },
          ["overlay-backdrop"] = { background = "#000000/50" },
        } }"##,
    );
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.style_error().is_none(),
        "{:?}",
        workspace.style_error()
    );
    let look = crate::Look::default();
    let overlay = workspace
        .stylesheet()
        .resolve(Part::Overlay, &look, false, &Rules::default());
    assert_eq!(overlay.base.radius, Some(Corners::all(12.0)));
    assert!(overlay.base.transition.is_some());
    let backdrop =
        workspace
            .stylesheet()
            .resolve(Part::OverlayBackdrop, &look, false, &Rules::default());
    assert!(backdrop.base.background.is_some());
}

#[test]
fn key_chords_read_as_gpui_names_them() {
    use crate::KeyChord;
    let minus = KeyChord::parse("ctrl--").unwrap();
    assert!(minus.ctrl && minus.key == "-");
    let bare = KeyChord::parse("-").unwrap();
    assert!(!bare.ctrl && bare.key == "-");
    let every = KeyChord::parse("ctrl-alt-shift-cmd-f5").unwrap();
    assert!(every.ctrl && every.alt && every.shift && every.cmd && !every.secondary);
    assert_eq!(every.key, "f5");
    for good in [
        "escape",
        "enter",
        "pagedown",
        "space",
        "/",
        "1",
        "secondary-enter",
    ] {
        assert!(KeyChord::parse(good).is_ok(), "{good}");
    }
    for bad in ["", "--", "ctrl-", "esc", "Enter", "ctrl-ab", "hyper-x"] {
        assert!(KeyChord::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn float_pins_to_an_edge_or_corner() {
    use crate::Offsets;
    let float = |value: &str| -> Result<Option<Offsets>, String> {
        let source = format!(
            "return mod {{ view = function() return ui.column {{ ui.row {{ float = {value} }} }} end }}"
        );
        let dir = app(&[("f", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let slot = workspace.slot("f").unwrap();
        if let Some(problem) = slot.broken().first() {
            return Err(problem.to_string());
        }
        let Some(Node::Stack(column)) = slot.view() else {
            panic!("no column");
        };
        let Node::Stack(row) = &column.children[0] else {
            panic!("no row");
        };
        Ok(row.frame.float)
    };
    let at = |top, right, bottom, left| Offsets {
        top,
        right,
        bottom,
        left,
    };
    assert_eq!(
        float("'top-right'"),
        Ok(Some(at(Some(0.0), Some(0.0), None, None)))
    );
    assert_eq!(
        float("{ at = 'bottom', inset = 8 }"),
        Ok(Some(at(None, Some(8.0), Some(8.0), Some(8.0))))
    );
    assert_eq!(
        float("{ top = -4 }"),
        Ok(Some(at(Some(-4.0), None, None, None)))
    );
    assert!(
        float("'middle'")
            .unwrap_err()
            .contains("`middle` is not a place")
    );
    assert!(
        float("{ at = 'top', top = 1 }")
            .unwrap_err()
            .contains("not both")
    );
}

#[test]
fn tooltips_and_context_menus_go_on_any_element() {
    let source = r#"return mod { view = function()
  return ui.column {
    ui.text { "t", tooltip = "a text" },
    ui.icon { "bell", tooltip = "an icon", context = "menu" },
    ui.badge { "3", copy = "3" },
  }
end }"#;
    let dir = app(&[("tips", source)]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let Some(Node::Stack(column)) = workspace.slot("tips").unwrap().view() else {
        panic!("no column");
    };
    let (Node::Text(text), Node::Icon(icon), Node::Badge(badge)) = (
        &column.children[0],
        &column.children[1],
        &column.children[2],
    ) else {
        panic!("{:?}", column.children);
    };
    assert_eq!(text.extras.tooltip.as_deref(), Some("a text"));
    assert_eq!(
        icon.extras.context,
        Some(Action::Send {
            to: None,
            msg: Data::from("menu")
        })
    );
    assert_eq!(badge.extras.copy.as_deref(), Some("3"));
}
