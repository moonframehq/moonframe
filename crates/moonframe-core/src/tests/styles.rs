use super::*;

fn styled_panel(workspace: &Workspace) -> crate::Panel {
    let Some(Node::Panel(panel)) = workspace.slot("styled").unwrap().view() else {
        panic!("no panel");
    };
    panel.clone()
}

#[test]
fn stylesheets_layer_type_class_and_inline_rules() {
    let dir = app(&[("styled", STYLED)]);
    write(dir.path(), "style.luau", STYLE);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let sheet = workspace.stylesheet();
    assert_eq!((sheet.radius, sheet.font_size), (Some(6.0), Some(13.0)));
    assert_eq!(sheet.mono, crate::MonoFont {
        family: Some("Fira Code".into()),
        size: Some(12.0),
        weight: Some(500.0),
        line_height: Some(1.2),
        fallbacks: Some(vec!["Symbols Nerd Font Mono".into()]),
        ligatures: Some(false),
    });
    assert_eq!(sheet.overrides, [(Token::Accent, Shade {
        light: 0x4f46e5ff,
        dark: 0x818cf8ff
    })]);

    let panel = styled_panel(&workspace);
    let style = sheet
        .resolve(Part::Panel, &panel.look, false, &Rules::default())
        .base;
    // The class overrides the type's padding; the element's own style wins.
    assert_eq!(style.radius, Some(Corners::all(2.0)));
    assert_eq!(style.padding, Sides::all(3.0));
    assert_eq!(style.shadow, Some(Shadow::Preset(ShadowPreset::Small)));
    assert_eq!(
        style.background,
        Some(Color {
            paint: Paint::Fixed(Shade {
                light: 0xffffffff,
                dark: 0x1f1f23ff
            }),
            alpha: 1.0
        })
    );
    assert_eq!(
        style.color.map(|color| color.paint),
        Some(Paint::Token(Token::Danger))
    );

    let Node::Item(item) = &panel.children[0] else {
        panic!("no item");
    };
    let selected = sheet.resolve(Part::Item, &item.look, true, &Rules::default());
    assert_eq!(
        selected.base.background,
        Some(Color {
            paint: Paint::Token(Token::Accent),
            alpha: 0.2
        })
    );
    assert_eq!(
        selected.hover.background.map(|color| color.paint),
        Some(Paint::Token(Token::ListHover))
    );
    let resting = sheet.resolve(Part::Item, &item.look, false, &Rules::default());
    assert_eq!(resting.base.background, None);
}

#[test]
fn a_broken_stylesheet_keeps_the_last_good_one() {
    let dir = app(&[("styled", STYLED)]);
    write(dir.path(), "style.luau", STYLE);
    let mut workspace = Workspace::open(dir.path(), false);
    write(
        dir.path(),
        "style.luau",
        "return style { rules = { panel = { radius = 'big' } } }",
    );
    workspace.reload();
    let problems = workspace.problems();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].0, "style");
    assert_eq!(workspace.stylesheet().radius, Some(6.0));
}

#[test]
fn bad_styles_are_refused() {
    for (sheet, expected) in [
        ("rules = { pannel = {} }", "`pannel` is not an element"),
        (
            "rules = { ['item:Pressed!'] = {} }",
            "`Pressed!` is not a state",
        ),
        (
            "rules = { panel = { rounded = 4 } }",
            "unknown key `rounded`",
        ),
        (
            "rules = { panel = { background = 'chartreuse' } }",
            "unknown color `chartreuse`",
        ),
        (
            "rules = { panel = { background = '#12345' } }",
            "not a hex color",
        ),
        (
            "rules = { panel = { background = 'accent/150' } }",
            "percent from 0 to 100",
        ),
        (
            "colors = { brand = { light = '#fff' } }",
            "colors.brand.dark",
        ),
        ("radius = -1", "expected 0 to 64"),
        ("theme = {}", "unknown key `theme`"),
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
fn inline_styles_are_checked_against_the_palette() {
    let view = |style: &str| {
        format!("return mod {{ view = function() return ui.row {{ style = {style} }} end }}")
    };
    let dir = app(&[("ok", &view("{ background = 'paper', border = 'bottom' }"))]);
    write(dir.path(), "style.luau", STYLE);
    let workspace = Workspace::open(dir.path(), false);
    let Some(Node::Stack(row)) = workspace.slot("ok").unwrap().view() else {
        panic!("no row");
    };
    assert_eq!(row.look.style.border, Some(Border::Bottom));

    let dir = app(&[("bad", &view("{ background = 'paper' }"))]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.slot("bad").unwrap().error().unwrap().to_string();
    assert!(
        error.contains("row.style.background: unknown color `paper`"),
        "{error}"
    );
}

#[test]
fn padding_and_margin_take_each_edge() {
    let sheet = r#"return style { rules = {
      panel = { padding = { 4, 12 }, margin = 8 },
      [".a"] = { padding = { top = 20 }, margin = { x = "large", left = 2 } },
      [".b"] = { padding = { 1, 2, 3, 4 } },
    } }"#;
    let view = r#"return mod { view = function()
      return ui.row { ui.panel { class = "a" }, ui.panel { class = "b" } }
    end }"#;
    let dir = app(&[("edges", view)]);
    write(dir.path(), "style.luau", sheet);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let Some(Node::Stack(row)) = workspace.slot("edges").unwrap().view() else {
        panic!("no row");
    };
    let sheet = workspace.stylesheet();
    let style = |index: usize| {
        let Node::Panel(panel) = &row.children[index] else {
            panic!("no panel");
        };
        sheet
            .resolve(Part::Panel, &panel.look, false, &Rules::default())
            .base
    };
    let edges = |top, right, bottom, left| Sides {
        top: Some(top),
        right: Some(right),
        bottom: Some(bottom),
        left: Some(left),
    };
    // `.a` changes only the top padding and the horizontal margins.
    assert_eq!(style(0).padding, edges(20.0, 12.0, 4.0, 12.0));
    assert_eq!(style(0).margin, edges(8.0, 16.0, 8.0, 2.0));
    assert_eq!(style(1).padding, edges(1.0, 2.0, 3.0, 4.0));

    for (value, expected) in [
        ("{ 1, 2, 3 }", "1, 2, or 4 values"),
        ("{ 1, top = 2 }", "not both"),
        ("{ middle = 2 }", "unknown key `middle`"),
        ("{ top = 'huge' }", "unknown `huge`"),
    ] {
        let dir = app(&[]);
        let sheet = format!("return style {{ rules = {{ panel = {{ padding = {value} }} }} }}");
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
fn mods_bring_styles_scoped_to_what_they_draw() {
    let fancy = r#"return mod {
      styles = {
        panel = { radius = 4, padding = 2 },
        [".title"] = { color = "paper", weight = "bold" },
      },
      view = function()
        return ui.panel { class = "title", ui.text "x" }
      end,
    }"#;
    let plain = "return mod { view = function() return ui.panel { class = 'title' } end }";
    let dir = app(&[("fancy", fancy), ("plain", plain)]);
    write(
        dir.path(),
        "style.luau",
        r##"return style {
          colors = { paper = "#123456" },
          rules = { panel = { radius = 9 }, [".title"] = { weight = "medium" } },
        }"##,
    );
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let sheet = workspace.stylesheet();
    let style = |name: &str| {
        let slot = workspace.slot(name).unwrap();
        let Some(Node::Panel(panel)) = slot.view() else {
            panic!("no panel");
        };
        sheet
            .resolve(Part::Panel, &panel.look, false, slot.rules())
            .base
    };
    let fancy = style("fancy");
    // The stylesheet outranks the mod at the same step; the mod keeps the
    // rest, and its class rule outranks the stylesheet's type rule.
    assert_eq!(fancy.radius, Some(Corners::all(9.0)));
    assert_eq!(fancy.padding, Sides::all(2.0));
    assert_eq!(fancy.weight, Some(crate::Weight::Medium));
    assert_eq!(
        fancy.color.map(|color| color.paint),
        Some(Paint::Fixed(Shade {
            light: 0x123456ff,
            dark: 0x123456ff
        }))
    );
    // Another mod's panel gets none of it.
    let plain = style("plain");
    assert_eq!(plain.padding, Sides::default());
    assert_eq!(plain.color, None);

    let broken = "return mod { styles = { pannel = {} }, view = function() return 'x' end }";
    let dir = app(&[("broken", broken)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("broken")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("styles[\"pannel\"]"), "{error}");
}

#[test]
fn text_takes_the_whole_weight_scale_inline_and_in_rules() {
    use crate::Weight;
    let source = r#"return mod { view = function()
  return ui.panel {
    ui.text { "a", weight = "semibold" },
    ui.text { "b", weight = "light" },
    ui.text { "c", style = { weight = "semibold" } },
    ui.item { "d", selected = true, class = "nav" },
  }
end }"#;
    let dir = app(&[("probe", source)]);
    write(
        dir.path(),
        "style.luau",
        r#"return style { rules = {
          [".nav"] = { weight = "medium" },
          [".nav:selected"] = { weight = "semibold" },
          [".nav:hover"] = { weight = "light" },
        } }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let Some(Node::Panel(panel)) = workspace.slot("probe").unwrap().view() else {
        panic!("no panel");
    };
    let (Node::Text(a), Node::Text(b), Node::Text(c), Node::Item(d)) = (
        &panel.children[0],
        &panel.children[1],
        &panel.children[2],
        &panel.children[3],
    ) else {
        panic!("expected three texts and an item");
    };
    assert_eq!(
        (a.weight, b.weight),
        (Some(Weight::Semibold), Some(Weight::Light))
    );
    let sheet = workspace.stylesheet();
    let inline = sheet.resolve(Part::Text, &c.look, false, &Rules::default());
    assert_eq!(inline.base.weight, Some(Weight::Semibold));
    let selected = sheet.resolve(Part::Item, &d.look, true, &Rules::default());
    assert_eq!(selected.base.weight, Some(Weight::Semibold));
    assert_eq!(selected.hover.weight, Some(Weight::Light));
    let resting = sheet.resolve(Part::Item, &d.look, false, &Rules::default());
    assert_eq!(resting.base.weight, Some(Weight::Medium));

    let heavy = r#"return mod { view = function() return ui.text { "x", weight = "heavy" } end }"#;
    let error = view_of(heavy).unwrap_err();
    assert!(error.contains("semibold"), "{error}");
}

#[test]
fn a_broken_user_stylesheet_names_its_file() {
    let builtin = app(&[]);
    let user = tempfile::tempdir().unwrap();
    write(
        user.path(),
        "style.luau",
        "return style { rules = { panel = { radius = 'big' } } }",
    );
    let roots = crate::Roots::new([builtin.path().to_owned(), user.path().to_owned()]);
    let workspace = Workspace::open(roots, false);
    let error = workspace.style_error().unwrap().to_string();
    assert!(error.contains("radius"), "{error}");
    write(user.path(), "style.luau", "return 42");
    let mut workspace = workspace;
    workspace.reload();
    let error = workspace.style_error().unwrap().to_string();
    assert!(error.contains("style.luau"), "{error}");
}

#[test]
fn focus_and_disabled_states_layer_over_the_resting_rule() {
    use crate::States;
    let style = r#"return style {
      rules = {
        input = { background = "muted", border_color = "border" },
        ["input:hover"] = { border_color = "accent/50" },
        ["input:focus"] = { border_color = "accent" },
        ["input:disabled"] = { opacity = 0.5 },
        [".loud:focus"] = { background = "accent/10" },
        ["input-placeholder"] = { color = "muted-foreground" },
        ["terminal:focus"] = { border_color = "accent" },
      },
    }"#;
    let dir = app(&[(
        "field",
        "return mod { view = function() return ui.input { class = 'loud' } end }",
    )]);
    write(dir.path(), "style.luau", style);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let Some(Node::Input(input)) = workspace.slot("field").unwrap().view() else {
        panic!("no input");
    };
    let sheet = workspace.stylesheet();
    let at = |focus, disabled| {
        sheet.resolve_in(
            Part::Input,
            &input.look,
            States {
                selected: false,
                focus,
                disabled,
            },
            &Rules::default(),
        )
    };
    let token = |color: Option<Color>| color.map(|color| (color.paint, color.alpha));
    let resting = at(false, false);
    assert_eq!(
        token(resting.base.border_color),
        Some((Paint::Token(Token::Border), 1.0))
    );
    assert_eq!(
        token(resting.hover.border_color),
        Some((Paint::Token(Token::Accent), 0.5))
    );
    let focused = at(true, false);
    assert_eq!(
        token(focused.base.border_color),
        Some((Paint::Token(Token::Accent), 1.0))
    );
    // The class's focus rule comes after the type's.
    assert_eq!(
        token(focused.base.background),
        Some((Paint::Token(Token::Accent), 0.1))
    );
    let disabled = at(false, true);
    assert_eq!(disabled.base.opacity, Some(0.5));
    assert_eq!(disabled.hover, Default::default());
    let placeholder = sheet
        .resolve(
            Part::InputPlaceholder,
            &Default::default(),
            false,
            &Rules::default(),
        )
        .base;
    assert_eq!(
        token(placeholder.color),
        Some((Paint::Token(Token::MutedForeground), 1.0))
    );

    let dir = app(&[]);
    write(
        dir.path(),
        "style.luau",
        "return style { rules = { ['input:Pressed'] = {} } }",
    );
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.style_error().unwrap().to_string();
    assert!(error.contains("`Pressed` is not a state"), "{error}");
    assert!(error.contains("focus, disabled"), "{error}");
}

#[test]
fn named_states_match_rules_of_the_same_name() {
    let dir = app(&[(
        "drawer",
        r#"return mod {
  init = function() return { open = false } end,
  update = function(state) state.open = not state.open end,
  view = function(state)
    return ui.column { class = "drawer", state = state.open and "open" or "closed", ui.text "x" }
  end,
}"#,
    )]);
    write(
        dir.path(),
        "style.luau",
        "return style { rules = { ['.drawer'] = { opacity = 0.5 }, ['.drawer:open'] = { width = 300, opacity = 1 }, ['.drawer:closed'] = { width = 40 } } }",
    );
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    let resolved = |workspace: &Workspace| {
        let Some(Node::Stack(column)) = workspace.slot("drawer").unwrap().view() else {
            panic!("no column");
        };
        let style = workspace
            .stylesheet()
            .resolve(Part::Column, &column.look, false, &Rules::default())
            .base;
        (column.look.state.clone(), style.width, style.opacity)
    };
    assert_eq!(
        resolved(&workspace),
        (Some("closed".to_owned()), Some(40.0), Some(0.5))
    );
    workspace.send("drawer", "drawer", Data::from("toggle"));
    assert_eq!(
        resolved(&workspace),
        (Some("open".to_owned()), Some(300.0), Some(1.0))
    );

    let source = "return mod { view = function() return ui.column { state = 'Not ok' } end }";
    let dir = app(&[("bad", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.slot("bad").unwrap().error().unwrap().to_string();
    assert!(error.contains("state: use a-z"), "{error}");
}

#[test]
fn a_stylesheet_cannot_rewrite_style_for_the_layers_above_it() {
    let builtin = app(&[]);
    write(
        builtin.path(),
        "style.luau",
        "style = function() return { font_size = 7 } end\nreturn {}",
    );
    let user = tempfile::tempdir().unwrap();
    write(user.path(), "style.luau", "return style { font_size = 20 }");
    let roots = crate::Roots::new([builtin.path().to_owned(), user.path().to_owned()]);
    let workspace = Workspace::open(roots, false);
    assert_eq!(workspace.stylesheet().font_size, Some(20.0));
}
