use {
    crate::{
        Action, Border, Color, Command, Data, Direction, Node, Paint, Part, Rules, SHELL, Shade,
        Shadow, ShadowLayer, ShadowPreset, Sides, Token, Workspace,
    },
    std::{fs, path::Path},
};

const SHELL_SOURCE: &str = r#"
return mod {
  init = function(previous) return previous or { left = true } end,
  update = function(state, msg)
    if msg == "toggle" then state.left = not state.left end
  end,
  view = function(state, app)
    return ui.column { grow = true,
      ui.hsplit {
        ui.pane { size = 200, fixed = true, visible = state.left, ui.region "left" },
        ui.pane { ui.mount "counter" },
      },
    }
  end,
}
"#;

const COUNTER: &str = r#"
return mod {
  region = "left",
  init = function(previous) return previous or { count = 0 } end,
  update = function(state, msg)
    if msg == "add" then
      state.count += 1
      return state, { { to = "shell", msg = "toggle" } }
    end
  end,
  view = function(state)
    return ui.panel { title = "Counter",
      ui.text(`count {state.count}`),
      ui.button { "Add", msg = "add" },
    }
  end,
}
"#;

fn write(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

fn app(mods: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "main.luau", SHELL_SOURCE);
    for (name, source) in mods {
        write(dir.path(), &format!("mods/{name}/main.luau"), source);
    }
    dir
}

/// The first child of mod `name` marked broken, as text.
fn broken(workspace: &Workspace, name: &str) -> String {
    let slot = workspace.slot(name).unwrap();
    assert_eq!(slot.error(), None, "the view as a whole still draws");
    slot.broken()
        .first()
        .map(ToString::to_string)
        .unwrap_or_default()
}

fn panel_text(workspace: &Workspace, name: &str) -> String {
    let Some(Node::Panel(panel)) = workspace.slot(name).unwrap().view() else {
        panic!("{name} drew no panel");
    };
    let Node::Text(text) = &panel.children[0] else {
        panic!("no text");
    };
    text.text.clone()
}

fn button_action(workspace: &Workspace, name: &str) -> Action {
    let Some(Node::Panel(panel)) = workspace.slot(name).unwrap().view() else {
        panic!("{name} drew no panel");
    };
    let Node::Button(button) = &panel.children[1] else {
        panic!("no button");
    };
    button.action.clone().unwrap()
}

#[test]
fn loads_the_shell_and_places_mods_in_regions() {
    let dir = app(&[("counter", COUNTER)]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.region("left"), ["counter"]);
    let Some(Node::Stack(column)) = workspace.shell().view() else {
        panic!("no column");
    };
    let Node::Split(split) = &column.children[0] else {
        panic!("no split");
    };
    assert_eq!(split.direction, Direction::Row);
    assert_eq!(split.panes.len(), 2);
    assert_eq!(split.panes[0].size, Some(200.0));
    assert!(split.panes[0].fixed && split.panes[0].visible);
    assert_eq!(panel_text(&workspace, "counter"), "count 0");
}

#[test]
fn messages_update_state_and_send_on() {
    let dir = app(&[("counter", COUNTER)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let action = button_action(&workspace, "counter");
    assert_eq!(workspace.dispatch("counter", &action), None);
    assert_eq!(panel_text(&workspace, "counter"), "count 1");
    // The counter told the shell to hide the left pane.
    let Some(Node::Stack(column)) = workspace.shell().view() else {
        panic!("no column");
    };
    let Node::Split(split) = &column.children[0] else {
        panic!("no split");
    };
    assert!(!split.panes[0].visible);
}

#[test]
fn commands_go_back_to_the_app() {
    let dir = app(&[]);
    let mut workspace = Workspace::open(dir.path(), false);
    let quit = Action::Command(Command::Quit);
    assert_eq!(workspace.dispatch(SHELL, &quit), Some(Command::Quit));
}

#[test]
fn reload_keeps_state_and_picks_up_new_mods() {
    let dir = app(&[("counter", COUNTER)]);
    let mut workspace = Workspace::open(dir.path(), false);
    let action = button_action(&workspace, "counter");
    workspace.dispatch("counter", &action);
    write(
        dir.path(),
        "mods/counter/main.luau",
        &COUNTER.replace("`count {state.count}`", "`total {state.count}`"),
    );
    write(dir.path(), "mods/other/main.luau", COUNTER);
    workspace.reload();
    assert_eq!(panel_text(&workspace, "counter"), "total 1");
    assert_eq!(workspace.region("left"), ["counter", "other"]);
}

#[test]
fn a_broken_edit_keeps_the_last_good_view() {
    let dir = app(&[("counter", COUNTER)]);
    let mut workspace = Workspace::open(dir.path(), false);
    write(dir.path(), "mods/counter/main.luau", "return mod { view = ");
    workspace.reload();
    let problems = workspace.problems();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].0, "counter");
    assert_eq!(panel_text(&workspace, "counter"), "count 0");
}

#[test]
fn typos_are_reported_where_they_are() {
    let source = r#"return mod { view = function()
        return ui.column { ui.row { ui.button { lable = "x" } } }
    end }"#;
    let dir = app(&[("typo", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = broken(&workspace, "typo");
    assert!(
        error.contains("column[1].row[1].button: unknown key `lable`"),
        "{error}"
    );
}

#[test]
fn icons_are_checked_against_the_bundled_set() {
    assert!(
        crate::ICONS.is_sorted(),
        "ICONS must stay sorted for binary search"
    );
    let source = r#"return mod { view = function()
        return ui.row { ui.icon "cpu", ui.icon { name = "network", tone = "muted" } }
    end }"#;
    let dir = app(&[("icons", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("icons").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(row)) = slot.view() else {
        panic!("expected a row");
    };
    let paths: Vec<_> = row
        .children
        .iter()
        .map(|child| match child {
            Node::Icon(icon) => icon.name.path(),
            other => panic!("expected an icon, got {other:?}"),
        })
        .collect();
    assert_eq!(paths, ["icons/cpu.svg", "icons/network.svg"]);
}

#[test]
fn bad_values_are_refused() {
    for (view, expected) in [
        (
            "ui.hsplit { ui.text 'x' }",
            "children of a split are ui.pane",
        ),
        ("ui.pane {}", "a pane goes inside"),
        ("ui.text { 'x', tone = 'loud' }", "unknown `loud`"),
        ("{ kind = 'row' }", "without `type`"),
        ("ui.button { 'x', msg = 'a', command = 'quit' }", "not both"),
        (
            "ui.button { 'x', command = 'explode' }",
            "unknown `explode`",
        ),
        ("42", "expected an element"),
        ("ui.icon 'cpus'", "no icon named `cpus`"),
    ] {
        let source = format!("return mod {{ view = function() return {view} end }}");
        let dir = app(&[("bad", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace
            .slot("bad")
            .unwrap()
            .error()
            .map(ToString::to_string);
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{view}: {error:?}"
        );
    }
}

#[test]
fn endless_loops_are_stopped() {
    let source = "return mod { view = function() while true do end end }";
    let dir = app(&[("spin", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.slot("spin").unwrap().error().unwrap().to_string();
    assert!(error.contains("ran out of instructions"), "{error}");
}

#[test]
fn the_sandbox_has_no_escape_hatches() {
    for global in [
        "os",
        "io",
        "debug",
        "require",
        "loadstring",
        "print",
        "getfenv",
    ] {
        let source = format!(
            "return mod {{ view = function() return ui.text(tostring({global} == nil)) end }}"
        );
        let dir = app(&[("probe", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let Some(Node::Text(text)) = workspace.slot("probe").unwrap().view() else {
            panic!("{global}: no text");
        };
        assert_eq!(text.text, "true", "{global} is reachable");
    }
    // The shared library cannot be changed.
    let source = "ui.text = nil\nreturn mod { view = function() return 'x' end }";
    let dir = app(&[("patch", source)]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(workspace.slot("patch").unwrap().error().is_some());
}

#[test]
fn data_crosses_both_ways() {
    let lua = mlua::Lua::new();
    let value: mlua::Value = lua
        .load("return { name = 'a', items = { 1, 2.5, true }, nested = { x = {} } }")
        .eval()
        .unwrap();
    let data = Data::read(&value).unwrap();
    let back = Data::read(&data.to_lua(&lua).unwrap()).unwrap();
    assert_eq!(data, back);
    let mixed: mlua::Value = lua.load("return { 1, x = 2 }").eval().unwrap();
    assert!(Data::read(&mixed).is_err());
    let function: mlua::Value = lua.load("return { f = function() end }").eval().unwrap();
    assert!(Data::read(&function).is_err());
}

#[test]
fn bad_mod_names_are_reported() {
    let dir = app(&[("Bad Name", COUNTER)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("Bad Name")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("not a mod name"), "{error}");
}

const STYLE: &str = r##"
return style {
  radius = 6,
  font_size = 13,
  colors = {
    accent = { light = "#4f46e5", dark = "#818cf8" },
    paper = { light = "#fff", dark = "#1f1f23" },
  },
  rules = {
    panel = { radius = 10, background = "paper", shadow = "small", padding = "large" },
    [".loud"] = { color = "danger", weight = "bold", padding = 3 },
    ["item:selected"] = { background = "accent/20" },
    ["item:hover"] = { background = "list-hover" },
  },
}
"##;

const STYLED: &str = r#"
return mod {
  view = function()
    return ui.panel { class = "loud", style = { radius = 2 },
      ui.item { "a", selected = true, class = { "loud" } },
    }
  end,
}
"#;

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
    assert_eq!(sheet.overrides, [(Token::Accent, Shade {
        light: 0x4f46e5ff,
        dark: 0x818cf8ff
    })]);

    let panel = styled_panel(&workspace);
    let style = sheet
        .resolve(Part::Panel, &panel.look, false, &Rules::default())
        .base;
    // The class overrides the type's padding; the element's own style wins.
    assert_eq!(style.radius, Some(2.0));
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
            "rules = { ['item:pressed'] = {} }",
            "unknown state `pressed`",
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
    assert_eq!(fancy.radius, Some(9.0));
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
    });
    assert_eq!(transition(Some("fading")), Transition {
        kind: TransitionKind::Fade,
        duration: Duration::from_millis(300),
        curve: Curve::CubicBezier(0.2, 0.0, 0.0, 1.0),
    });
    assert_eq!(transition(Some("table")), Transition {
        kind: TransitionKind::SlideFade,
        duration: Duration::from_millis(150),
        curve: Curve::Linear,
    });
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
fn the_type_definitions_cover_what_the_host_accepts() {
    use crate::{
        Align, Anchor, Command, Direction, Element, Justify, Side, Space, Surface, TextSize, Tone,
        TransitionKind, Variant, Weight,
    };
    let definitions = include_str!("../../../types/moonframe.d.luau");
    let missing: Vec<String> = Element::NAMES
        .iter()
        .filter(|name| !definitions.contains(&format!("\t{name}: (")))
        .map(|name| format!("ui.{name}"))
        .chain(
            [
                Tone::NAMES,
                Space::NAMES,
                TextSize::NAMES,
                Weight::NAMES,
                Align::NAMES,
                Justify::NAMES,
                Surface::NAMES,
                Direction::NAMES,
                Variant::NAMES,
                Command::NAMES,
                TransitionKind::NAMES,
                Border::NAMES,
                Side::NAMES,
                Anchor::NAMES,
            ]
            .concat()
            .into_iter()
            .filter(|name| !definitions.contains(&format!("\"{name}\"")))
            .map(|name| format!("\"{name}\"")),
        )
        .collect();
    assert!(
        missing.is_empty(),
        "types/moonframe.d.luau is missing {missing:?}"
    );
}

#[test]
fn long_strings_are_refused() {
    let long = "x".repeat(20_000);
    let source = format!("return mod {{ view = function() return ui.row {{ '{long}' }} end }}");
    let dir = app(&[("long", &source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = broken(&workspace, "long");
    assert!(error.contains("longer than"), "{error}");

    let lua = mlua::Lua::new();
    let huge = mlua::Value::String(lua.create_string("y".repeat(100_000)).unwrap());
    assert!(Data::read(&huge).is_err());
    let key = lua.create_table().unwrap();
    key.raw_set("k".repeat(100_000), 1).unwrap();
    assert!(Data::read(&mlua::Value::Table(key)).is_err());
}

#[test]
fn log_lines_cannot_drive_the_terminal() {
    assert_eq!(
        crate::vm::printable("ok\x1b[2Jcleared\ttab"),
        "ok?[2Jcleared\ttab"
    );
    assert_eq!(crate::vm::printable(&"z".repeat(10_000)).len(), 4096);
}

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
fn the_users_folder_goes_over_the_apps() {
    let builtin = app(&[("styled", STYLED), ("counter", COUNTER)]);
    write(builtin.path(), "style.luau", STYLE);
    let user = tempfile::tempdir().unwrap();
    // Replaces the built-in counter, adds a mod, and restyles one property
    // of one rule and one color.
    write(
        user.path(),
        "mods/counter/main.luau",
        r#"return mod { region = "left", view = function() return ui.text "mine" end }"#,
    );
    write(
        user.path(),
        "mods/extra/main.luau",
        r#"return mod { view = function() return ui.text "extra" end }"#,
    );
    write(
        user.path(),
        "style.luau",
        r##"return style { radius = 9, colors = { accent = "#ff0000" }, rules = { panel = { radius = 4 } } }"##,
    );
    let roots = crate::Roots::new([builtin.path().to_owned(), user.path().to_owned()]);
    let workspace = Workspace::open(roots, false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.mod_names().collect::<Vec<_>>(), [
        "counter", "extra", "styled"
    ]);
    let Some(Node::Text(text)) = workspace.slot("counter").unwrap().view() else {
        panic!("the user's counter did not replace the app's");
    };
    assert_eq!(text.text, "mine");

    let sheet = workspace.stylesheet();
    assert_eq!((sheet.radius, sheet.font_size), (Some(9.0), Some(13.0)));
    assert_eq!(sheet.overrides, [(Token::Accent, Shade {
        light: 0xff0000ff,
        dark: 0xff0000ff
    })]);
    let style = sheet
        .resolve(
            Part::Panel,
            &crate::Look::default(),
            false,
            &Rules::default(),
        )
        .base;
    // The user's radius, over the app's background and shadow.
    assert_eq!(style.radius, Some(4.0));
    assert_eq!(style.shadow, Some(Shadow::Preset(ShadowPreset::Small)));
    assert!(style.background.is_some());
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

const BADGE_COMPONENT: &str = r#"
return component {
  title = "Pill",
  category = "Data",
  description = "A labelled value.",
  example = { label = "CPU", value = 12 },
  build = function(props)
    return ui.row {
      ui.text { props.label },
      ui.badge { tostring(props.value) },
    }
  end,
}
"#;

#[test]
fn elements_carry_the_line_of_the_mod_that_built_them() {
    // Lines 3 and 4 build the row and text; the component is used on line 5.
    let source = "local pill = use \"pill\"\nreturn mod { view = function()\n  return ui.column {\n    ui.text { \"a\" },\n    pill { label = \"x\", value = 1 },\n  }\nend }\n";
    let dir = app(&[("lines", source)]);
    write(dir.path(), "components/pill.luau", BADGE_COMPONENT);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("lines").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    assert!(slot.source().unwrap().ends_with("mods/lines/main.luau"));
    let Some(Node::Stack(column)) = slot.view() else {
        panic!("expected a column");
    };
    assert_eq!(column.look.line, Some(3));
    let Node::Text(text) = &column.children[0] else {
        panic!("expected text");
    };
    assert_eq!(text.look.line, Some(4));
    // Built inside the component, so it points at where the mod used it.
    let Node::Stack(pill) = &column.children[1] else {
        panic!("expected the component's row");
    };
    assert_eq!(pill.look.line, Some(5));
}

#[test]
fn components_are_listed_with_a_preview_and_usage() {
    let dir = app(&[]);
    write(dir.path(), "components/pill.luau", BADGE_COMPONENT);
    write(
        dir.path(),
        "components/broken.luau",
        "return component { build = function() return 42 end }",
    );
    write(
        dir.path(),
        "components/loop.luau",
        "local again = use \"loop\"\nreturn component { build = again }",
    );
    let workspace = Workspace::open(dir.path(), false);
    let components = workspace.components();
    let names: Vec<&str> = components
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    assert_eq!(names, ["broken", "loop", "pill"]);

    let pill = &components[2];
    assert_eq!(
        (pill.title.as_str(), pill.category.as_str()),
        ("Pill", "Data")
    );
    assert_eq!(
        pill.usage,
        "local pill = use \"pill\"\n\npill { label = \"CPU\", value = 12 }"
    );
    assert!(
        matches!(pill.preview, Ok(Node::Stack(_))),
        "{:?}",
        pill.preview
    );

    let problem = |name: &str| {
        components
            .iter()
            .find(|component| component.name == name)
            .and_then(|component| component.preview.as_ref().err())
            .map(ToString::to_string)
            .unwrap_or_default()
    };
    assert!(
        problem("broken").contains("expected an element"),
        "{}",
        problem("broken")
    );
    assert!(
        problem("loop").contains("uses itself"),
        "{}",
        problem("loop")
    );
    assert!(
        workspace
            .problems()
            .iter()
            .any(|(name, _)| *name == "broken.luau"),
        "{:?}",
        workspace.problems()
    );
}

#[test]
fn using_a_missing_component_names_the_ones_there_are() {
    let source = r#"local nope = use "nope"
return mod { view = function() return ui.text "x" end }"#;
    let dir = app(&[("user", source)]);
    write(dir.path(), "components/pill.luau", BADGE_COMPONENT);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.slot("user").unwrap().error().unwrap().to_string();
    assert!(
        error.contains("no component named `nope`; there are: pill"),
        "{error}"
    );
}

#[test]
fn a_broken_child_is_marked_in_place_and_the_rest_still_draws() {
    let source = r#"return mod { view = function()
        return ui.row { ui.text "before", ui.icon "newspaper", ui.text "after" }
    end }"#;
    let dir = app(&[("news", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let error = broken(&workspace, "news");
    // A short message with the closest names, not every icon.
    assert!(
        error.contains("row[2].icon.name: no icon named `newspaper`"),
        "{error}"
    );
    assert!(error.contains("IconName"), "{error}");
    assert!(error.len() < 300, "{error}");
    let Some(Node::Stack(row)) = workspace.slot("news").unwrap().view() else {
        panic!("the row still draws");
    };
    assert!(matches!(&row.children[..], [
        Node::Text(_),
        Node::Problem(_),
        Node::Text(_)
    ]));
    // It still counts as a problem for `just check`.
    assert!(workspace.problems().iter().any(|(name, _)| *name == "news"));
    // A near miss gets its suggestion.
    let typo = "return mod { view = function() return ui.row { ui.icon 'cpus' } end }";
    let dir = app(&[("typo", typo)]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(
        broken(&workspace, "typo").contains("close ones: cpu"),
        "{}",
        broken(&workspace, "typo")
    );
}

#[test]
fn a_users_broken_copy_falls_back_to_the_apps_until_it_loads() {
    let builtin = app(&[("counter", COUNTER)]);
    let user = tempfile::tempdir().unwrap();
    write(
        user.path(),
        "mods/counter/main.luau",
        "return mod { view = ",
    );
    let roots = crate::Roots::new([builtin.path().to_owned(), user.path().to_owned()]);
    let workspace = Workspace::open(roots, false);
    let slot = workspace.slot("counter").unwrap();
    // The app's counter draws, under the error of the user's copy.
    assert!(slot.view().is_some());
    assert!(slot.source().unwrap().starts_with(builtin.path()));
    let error = slot.error().unwrap().to_string();
    assert!(
        error.contains("showing") && error.contains(&*user.path().to_string_lossy()),
        "{error}"
    );
}

#[test]
fn packages_lie_between_the_app_and_the_user_and_never_replace_the_shell() {
    let builtin = app(&[]);
    let dir = tempfile::tempdir().unwrap();
    let (single, folder, user) = (
        dir.path().join("My.Clock"),
        dir.path().join("extras"),
        dir.path().join("user"),
    );
    write(
        &single,
        "main.luau",
        r#"return mod { view = function() return ui.text "clock" end }"#,
    );
    write(
        &single,
        "style.luau",
        "error('a single mod has no stylesheet')",
    );
    write(
        &folder,
        "mods/extra/main.luau",
        r#"return mod { view = function() return ui.text "extra" end }"#,
    );
    write(&folder, "style.luau", "return style { radius = 7 }");
    fs::create_dir_all(&user).unwrap();
    let roots = crate::Roots::with_packages(
        builtin.path().to_owned(),
        [single.clone(), folder.clone()],
        Some(user),
    );
    assert_eq!(roots.layers().count(), 4);
    assert_eq!(
        roots.file("main.luau"),
        Some(builtin.path().join("main.luau"))
    );
    assert_eq!(roots.files("style.luau"), [folder.join("style.luau")]);
    let workspace = Workspace::open(roots, false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.mod_names().collect::<Vec<_>>(), [
        "extra", "my-clock"
    ]);
    assert_eq!(workspace.stylesheet().radius, Some(7.0));
}
