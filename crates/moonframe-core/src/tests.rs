use {
    crate::{
        Action, Border, Color, Command, Corners, Data, Direction, Node, Paint, Part, Rules, SHELL,
        Shade, Shadow, ShadowLayer, ShadowPreset, Sides, Token, Workspace,
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
fn browsers_take_web_addresses_and_commands_name_them() {
    use crate::{Browser, BrowserCommand};
    let source = r#"return mod { view = function()
        return ui.column {
            ui.browser "https://example.com",
            ui.browser { id = "docs", url = "file:///tmp/a.html", zoom = 2, navigate = { go = true } },
            ui.button { "Back", command = "browser-back", browser = "docs" },
            ui.button { "Open", command = "browser-open", browser = "docs", to = "other" },
        }
    end }"#;
    let dir = app(&[("web", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("web").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(column)) = slot.view() else {
        panic!("expected a column");
    };
    let [
        Node::Browser(first),
        Node::Browser(docs),
        Node::Button(back),
        Node::Button(open),
    ] = &column.children[..]
    else {
        panic!("unexpected children {:?}", column.children);
    };
    assert_eq!(
        (first.id.as_str(), first.url.as_deref()),
        ("browser", Some("https://example.com"))
    );
    let Browser { zoom, navigate, .. } = docs;
    assert_eq!((*zoom, navigate.is_some()), (2.0, true));
    assert_eq!(
        back.action,
        Some(Action::Browser {
            to: None,
            id: "docs".into(),
            command: BrowserCommand::Back
        })
    );
    assert_eq!(
        open.action,
        Some(Action::Browser {
            to: Some("other".into()),
            id: "docs".into(),
            command: BrowserCommand::Open
        })
    );

    for (view, expected) in [
        (
            r#"ui.browser "javascript:alert(1)""#,
            "not an http, https or file address",
        ),
        (
            r#"ui.browser "vscode://x""#,
            "not an http, https or file address",
        ),
        (
            r#"ui.browser { url = "https://a b" }"#,
            "not an http, https or file address",
        ),
        (
            r#"ui.button { "x", command = "browser-back" }"#,
            "needs `browser",
        ),
        (
            r#"ui.button { "x", msg = "m", browser = "a" }"#,
            "`browser` goes with a browser command",
        ),
        (
            r#"ui.button { "x", command = "quit", browser = "a" }"#,
            "`browser` goes with a browser command",
        ),
        (
            r#"ui.button { "x", command = "browser-go", browser = "a" }"#,
            "browser-back",
        ),
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
  mono = { family = "Fira Code", size = 12, weight = 500, line_height = 1.2,
           fallbacks = { "Symbols Nerd Font Mono" }, ligatures = false },
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
        Align, Anchor, BrowserCommand, Command, Direction, Element, Justify, Placement, Side,
        Space, Surface, TextSize, Tone, TransitionKind, Variant, Weight,
    };
    let definitions = include_str!("../../../types/moonframe.d.luau");
    let missing: Vec<String> = Element::NAMES
        .iter()
        // `use` and `ui.mount { id = ... }` build instances; no mod calls it.
        .filter(|name| **name != "instance")
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
                BrowserCommand::NAMES,
                TransitionKind::NAMES,
                Border::NAMES,
                Side::NAMES,
                Anchor::NAMES,
                Placement::NAMES,
                crate::Fit::NAMES,
                crate::Titlebar::NAMES,
                crate::Platform::NAMES,
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
        r##"return style { radius = 9, mono = { size = 15 }, colors = { accent = "#ff0000" }, rules = { panel = { radius = 4 } } }"##,
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
    // The user's size, over the app's family.
    assert_eq!(
        (sheet.mono.size, sheet.mono.family.as_deref()),
        (Some(15.0), Some("Fira Code"))
    );
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
    assert_eq!(style.radius, Some(Corners::all(4.0)));
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

const TAB_COMPONENT: &str = r#"
return component {
  title = "Tab",
  when = "One of several views.",
  props = { label = "the text", selected = "whether it is the open one" },
  styles = {
    [".tab"] = { padding = 6, radius = 4 },
    [".tab:selected"] = { color = "accent" },
  },
  example = { label = "One", selected = true },
  build = function(props)
    return ui.row { class = "tab", selected = props.selected, ui.text { props.label } }
  end,
}
"#;

#[test]
fn component_styles_sit_under_the_stylesheet_and_reach_every_mod() {
    let source = "local tab = use \"tab\"\nreturn mod { view = function()\n  return tab { label = \"a\", selected = true }\nend }\n";
    let dir = app(&[("tabs", source)]);
    write(dir.path(), "components/tab.luau", TAB_COMPONENT);
    write(
        dir.path(),
        "style.luau",
        r#"return style { rules = { [".tab"] = { radius = 9 } } }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("tabs").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(tab)) = slot.view() else {
        panic!("expected the tab's row");
    };
    assert!(tab.selected);
    let resolved = workspace
        .stylesheet()
        .resolve(Part::Row, &tab.look, tab.selected, slot.rules());
    // The stylesheet's radius wins; the component's padding and selected
    // color stand where nothing overrides them.
    assert_eq!(resolved.base.radius, Some(Corners::all(9.0)));
    assert_eq!(resolved.base.padding, Sides::all(6.0));
    assert!(resolved.base.color.is_some());

    let component = &workspace.components()[0];
    assert_eq!(component.when, "One of several views.");
    assert_eq!(component.props.len(), 2);
    assert_eq!(component.rules.selectors(), [".tab", ".tab:selected"]);
    let catalogue = crate::catalogue(workspace.components());
    assert!(catalogue.contains("#### `tab`"), "{catalogue}");
    assert!(catalogue.contains("`.tab:selected`"), "{catalogue}");
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

/// The view of a mod made from `source`, or its problem.
fn view_of(source: &str) -> Result<Node, String> {
    let dir = app(&[("probe", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("probe").unwrap();
    match slot.error() {
        Some(error) => Err(error.to_string()),
        None => Ok(slot.view().unwrap().clone()),
    }
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
fn images_name_a_file_and_refuse_urls() {
    use {crate::Fit, std::path::PathBuf};
    let source = r#"return mod { view = function()
        return ui.row {
          ui.image "logo.png",
          ui.image { src = "~/Pictures/me.png", width = 32, height = 32, fit = "cover", class = "avatar" },
          ui.image { src = "/tmp/abs.png" },
        }
    end }"#;
    let dir = app(&[("pics", source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("pics").unwrap();
    assert_eq!(slot.error().map(ToString::to_string), None);
    let Some(Node::Stack(row)) = slot.view() else {
        panic!("no row");
    };
    let images: Vec<_> = row
        .children
        .iter()
        .map(|child| match child {
            Node::Image(image) => image.clone(),
            other => panic!("expected an image, got {other:?}"),
        })
        .collect();
    assert_eq!(images[0].fit, Fit::Contain);
    assert_eq!(
        (images[1].width, images[1].height),
        (Some(32.0), Some(32.0))
    );
    assert_eq!(images[1].fit, Fit::Cover);
    assert_eq!(images[1].look.class, ["avatar"]);
    // Each resolves against the mod's folder or home, inside what is allowed.
    let root = tempfile::tempdir().unwrap();
    let real = root.path().canonicalize().unwrap();
    write(&real, "app/mods/pics/logo.png", "png");
    write(&real, "home/Pictures/me.png", "png");
    let folder = real.join("app/mods/pics");
    let home = real.join("home");
    let resolved: Vec<Option<PathBuf>> = images
        .iter()
        .map(|image| image.resolve(&folder, Some(&home), std::slice::from_ref(&real)))
        .collect();
    assert_eq!(resolved, [
        Some(real.join("app/mods/pics/logo.png")),
        Some(real.join("home/Pictures/me.png")),
        None,
    ]);

    for (view, expected) in [
        (
            r#"ui.image "https://example.com/a.png""#,
            "images are files, not https:// URLs",
        ),
        (r#"ui.image { src = "" }"#, "an empty path"),
        (r#"ui.image {}"#, "image.src: missing"),
        (
            r#"ui.image { src = "a.png", fit = "stretch" }"#,
            "unknown `stretch`",
        ),
        (
            r#"ui.image { src = "a.png", alt = "x" }"#,
            "unknown key `alt`",
        ),
    ] {
        let source = format!("return mod {{ view = function() return {view} end }}");
        let dir = app(&[("bad", &source)]);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace.slot("bad").unwrap().error().unwrap().to_string();
        assert!(error.contains(expected), "{view}: {error}");
    }
}

#[test]
fn the_shell_asks_for_its_window_and_views_see_it() {
    use crate::{Chrome, Platform, Titlebar, WindowState};
    let dir = app(&[]);
    let workspace = Workspace::open(dir.path(), false);
    assert_eq!(workspace.chrome(), Chrome::default());
    assert_eq!(workspace.chrome().titlebar, Titlebar::System);

    let shell = r#"return mod {
      window = { titlebar = "hidden", traffic_lights = { x = 12, y = 16 } },
      view = function(_, app)
        return ui.row { drag = true,
          ui.text(`{app.window.platform} {app.window.titlebar} {app.window.maximized}`),
          ui.button { "x", command = "close-window" },
          ui.button { "-", command = "minimize" },
          ui.button { "+", command = "maximize" },
        }
      end,
    }"#;
    write(dir.path(), "main.luau", shell);
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(workspace.chrome(), Chrome {
        titlebar: Titlebar::Hidden,
        traffic_lights: Some((12.0, 16.0)),
    });
    let row = |workspace: &Workspace| {
        let Some(Node::Stack(row)) = workspace.shell().view() else {
            panic!("no row");
        };
        row.clone()
    };
    let text = |workspace: &Workspace| match &row(workspace).children[0] {
        Node::Text(text) => text.text.clone(),
        other => panic!("expected text, got {other:?}"),
    };
    assert!(row(&workspace).drag);
    let platform = Platform::current().name();
    assert_eq!(text(&workspace), format!("{platform} system false"));
    workspace.set_window(WindowState {
        titlebar: Titlebar::Hidden,
        maximized: true,
        ..WindowState::default()
    });
    assert_eq!(text(&workspace), format!("{platform} hidden true"));
    let commands: Vec<_> = row(&workspace).children[1..]
        .iter()
        .map(|child| match child {
            Node::Button(button) => button.action.clone(),
            other => panic!("expected a button, got {other:?}"),
        })
        .collect();
    assert_eq!(commands, [
        Some(Action::Command(Command::CloseWindow)),
        Some(Action::Command(Command::Minimize)),
        Some(Action::Command(Command::Maximize)),
    ]);

    for (window, expected) in [
        (r#"{ titlebar = "none" }"#, "unknown `none`"),
        (r#"{ traffic_lights = { x = 500 } }"#, "traffic_lights.x"),
        (r#"{ frameless = true }"#, "unknown key `frameless`"),
    ] {
        let source =
            format!("return mod {{ window = {window}, view = function() return ui.text 'x' end }}");
        write(dir.path(), "main.luau", &source);
        let workspace = Workspace::open(dir.path(), false);
        let error = workspace.shell().error().unwrap().to_string();
        assert!(error.contains(expected), "{window}: {error}");
    }

    // A mod is not the shell: the window isn't its to set.
    write(dir.path(), "main.luau", SHELL_SOURCE);
    let source = r#"return mod { window = { titlebar = "hidden" }, view = function() return ui.text "x" end }"#;
    write(dir.path(), "mods/sneaky/main.luau", source);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("sneaky")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("only the shell"), "{error}");
    assert_eq!(workspace.chrome(), Chrome::default());
}

#[test]
fn images_come_only_from_the_app_folders_and_the_cache() {
    let app = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(app.path(), "mods/pic/avatar.png", "png");
    write(outside.path(), "secret.png", "png");
    let image = |src: &str| crate::Image {
        look: crate::Look::default(),
        src: src.to_owned(),
        width: None,
        height: None,
        fit: crate::Fit::Contain,
    };
    let folder = app.path().join("mods/pic");
    let allowed = [app.path().to_owned()];
    let shown = |src: &str| image(src).resolve(&folder, None, &allowed).is_some();
    assert!(shown("avatar.png"));
    assert!(!shown("missing.png"));
    // Out of the app's folders, by a path or by `..`: an empty box.
    let secret = outside.path().join("secret.png");
    assert!(!shown(secret.to_str().unwrap()));
    let relative = format!("../../../{}", secret.strip_prefix("/").unwrap().display());
    assert!(!shown(&relative));
    // A folder is not a picture.
    assert!(!shown("."));
}

#[test]
fn a_nil_child_does_not_hide_the_ones_after_it() {
    let source = r#"return mod { view = function()
  local none = nil
  return ui.row { "a", none, none, none, "b", false, "c", float = { top = -4, right = -4 }, max_width = 200 }
end }"#;
    let Node::Stack(row) = view_of(source).unwrap() else {
        panic!("expected a row");
    };
    let texts: Vec<&str> = row
        .children
        .iter()
        .map(|child| match child {
            Node::Text(text) => text.text.as_str(),
            other => panic!("expected text, got {other:?}"),
        })
        .collect();
    assert_eq!(texts, ["a", "b", "c"]);
    assert_eq!(row.frame.max_width, Some(200.0));
    let both = r#"return mod { view = function() return ui.column { scroll = "both" } end }"#;
    let Node::Stack(column) = view_of(both).unwrap() else {
        panic!("expected a column");
    };
    assert_eq!(column.frame.scroll, crate::Scroll::Both);
    let float = row.frame.float.unwrap();
    assert_eq!(
        (float.top, float.right, float.left),
        (Some(-4.0), Some(-4.0), None)
    );
}

#[test]
fn buttons_take_an_icon_and_text_takes_lines_and_wrap() {
    let source = r#"return mod { view = function()
  return ui.row {
    ui.button { icon = "plus", msg = "add" },
    ui.text { "a long body", lines = 3 },
    ui.text { "fn main() {}", wrap = false },
  }
end }"#;
    let Node::Stack(row) = view_of(source).unwrap() else {
        panic!("expected a row");
    };
    let Node::Button(button) = &row.children[0] else {
        panic!("expected a button");
    };
    assert_eq!(button.label, "");
    assert_eq!(
        button.icon.as_ref().map(crate::IconName::name),
        Some("plus")
    );
    let (Node::Text(body), Node::Text(code)) = (&row.children[1], &row.children[2]) else {
        panic!("expected texts");
    };
    assert_eq!((body.lines, body.wrap), (Some(3), true));
    assert!(!code.wrap);
    let missing = r#"return mod { view = function() return ui.button { msg = "x" } end }"#;
    assert!(view_of(missing).unwrap_err().contains("label"));
}

#[test]
fn a_component_demo_runs_live_and_keeps_to_itself() {
    let dir = app(&[(
        "other",
        "return mod { update = function(s, m) error('reached') end, view = function() return ui.text 'x' end }",
    )]);
    write(dir.path(), "components/pill.luau", BADGE_COMPONENT);
    write(
        dir.path(),
        "components/demos/pill.luau",
        r#"local pill = use "pill"
return mod {
  init = function() return { n = 1 } end,
  update = function(state, msg)
    if msg == "add" then state.n += 1 end
    return state, { { to = "other", msg = "hi" } }
  end,
  view = function(state) return pill { label = "n", value = state.n } end,
}"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    let value = |workspace: &Workspace| {
        let (_, slot) = workspace.demo("pill").unwrap();
        let Some(Node::Stack(row)) = slot.view() else {
            panic!("expected the pill's row: {:?}", slot.error());
        };
        let Node::Badge(badge) = &row.children[1] else {
            panic!("expected a badge");
        };
        badge.text.clone()
    };
    assert_eq!(value(&workspace), "1");
    let send = Action::Send {
        to: None,
        msg: Data::from("add"),
    };
    assert_eq!(workspace.dispatch("demo:pill", &send), None);
    assert_eq!(value(&workspace), "2");
    // It reached no other mod, and runs no app command.
    assert_eq!(
        workspace
            .slot("other")
            .unwrap()
            .error()
            .map(ToString::to_string),
        None
    );
    assert_eq!(
        workspace.dispatch("demo:pill", &Action::Command(Command::Quit)),
        None
    );
    // The browser shows the file it runs.
    let component = &workspace.components()[0];
    assert!(
        component
            .demo
            .as_ref()
            .unwrap()
            .source
            .contains("state.n += 1")
    );
}

#[test]
fn a_component_without_a_demo_gets_one_made_from_its_example() {
    let dir = app(&[]);
    write(dir.path(), "components/pill.luau", BADGE_COMPONENT);
    let workspace = Workspace::open(dir.path(), false);
    let demo = workspace.components()[0].demo.clone().unwrap();
    assert!(demo.path.is_none());
    assert!(
        demo.source
            .contains("return pill { label = \"CPU\", value = 12 }"),
        "{}",
        demo.source
    );
    let (name, slot) = workspace.demo("pill").unwrap();
    assert_eq!(name, "demo:pill");
    assert!(
        matches!(slot.view(), Some(Node::Stack(_))),
        "{:?}",
        slot.error()
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

#[test]
fn only_web_addresses_count_as_remote() {
    use crate::{is_file_url, is_remote_url, is_web_url};
    assert!(is_remote_url("https://example.com/a"));
    assert!(is_remote_url("HTTP://example.com"));
    // A file is a page a browser may show, but never one for the system
    // browser, which would open whatever program it is.
    assert!(is_web_url("file:///Users/me/notes.html"));
    assert!(!is_remote_url("file:///Applications/Calculator.app"));
    assert!(is_file_url("FILE:///tmp/a.html"));
    assert!(!is_file_url("https://example.com"));
    assert!(!is_remote_url("javascript:alert(1)"));
}

/// The one element `view` returns, or the problem reading it.
fn input_view(view: &str) -> Result<Node, String> {
    let source = format!("return mod {{ view = function() return {view} end }}");
    let dir = app(&[("field", &source)]);
    let workspace = Workspace::open(dir.path(), false);
    let slot = workspace.slot("field").unwrap();
    match slot.error() {
        Some(error) => Err(error.to_string()),
        None => Ok(slot.view().unwrap().clone()),
    }
}

#[test]
fn inputs_read_their_text_messages_and_keys() {
    use crate::{Input, KeyChord};
    let Ok(Node::Input(input)) = input_view(
        r#"ui.input {
          id = "search", value = "abc", placeholder = "Search…",
          change = { query = true }, submit = "go", to = "results",
          keys = { up = "previous", ["shift-tab"] = { back = 1 }, ["secondary-k"] = "clear" },
          focus = true, mono = true, width = 240, class = "search",
        }"#,
    ) else {
        panic!("no input");
    };
    assert_eq!(input.id.as_deref(), Some("search"));
    assert_eq!(input.value.as_deref(), Some("abc"));
    assert_eq!(input.placeholder.as_deref(), Some("Search…"));
    assert_eq!(input.to.as_deref(), Some("results"));
    assert_eq!(input.submit, Some(Data::from("go")));
    assert!(input.focus && input.mono && !input.multiline && !input.disabled);
    assert_eq!((input.width, input.grow), (Some(240.0), false));
    assert_eq!(input.look.class, ["search"]);
    // Sorted by name, so the order is the same on every render.
    let keys: Vec<_> = input.keys.iter().map(|(chord, _)| chord.clone()).collect();
    assert_eq!(keys, [
        KeyChord::parse("secondary-k").unwrap(),
        KeyChord::parse("shift-tab").unwrap(),
        KeyChord::parse("up").unwrap(),
    ]);
    assert!(keys[0].secondary && keys[0].key == "k");
    assert!(keys[1].shift && keys[1].key == "tab" && !keys[1].ctrl);

    // A table message gets `value`; anything else is wrapped.
    let change = input.change.as_ref().unwrap();
    assert_eq!(
        Input::message(change, "ab"),
        Data::Map(
            [
                ("query".to_owned(), Data::Bool(true)),
                ("value".to_owned(), Data::from("ab")),
            ]
            .into()
        )
    );
    assert_eq!(
        Input::message(&Data::from("go"), "ab"),
        Data::Map(
            [
                ("msg".to_owned(), Data::from("go")),
                ("value".to_owned(), Data::from("ab")),
            ]
            .into()
        )
    );
    // A list can't take a named key, so it is wrapped too.
    let list = Data::List(vec![Data::Integer(1)]);
    assert_eq!(
        Input::message(&list, ""),
        Data::Map(
            [
                ("msg".to_owned(), list.clone()),
                ("value".to_owned(), Data::from("")),
            ]
            .into()
        )
    );

    let Ok(Node::Input(lines)) = input_view("ui.input { multiline = true, rows = 6, grow = true }")
    else {
        panic!("no multiline input");
    };
    assert!(lines.multiline && lines.grow);
    assert_eq!((lines.rows, lines.id.as_deref()), (6, None));
    let Ok(Node::Input(plain)) = input_view("ui.input {}") else {
        panic!("no plain input");
    };
    assert_eq!(plain.rows, Input::DEFAULT_ROWS);
    assert!(plain.keys.is_empty() && plain.change.is_none());
}

#[test]
fn bad_inputs_are_refused() {
    for (view, expected) in [
        ("ui.input { lable = 'x' }", "unknown key `lable`"),
        (
            "ui.input { rows = 3 }",
            "`rows` goes with `multiline = true`",
        ),
        (
            "ui.input { multiline = true, rows = 0 }",
            "a whole number from 1",
        ),
        (
            "ui.input { multiline = true, rows = 2.5 }",
            "a whole number from 1",
        ),
        ("ui.input { id = 'a b' }", "an id is 1 to 64"),
        ("ui.input { to = 'x' }", "`to` goes with `change`"),
        ("ui.input { keys = 'up' }", "expected a table"),
        ("ui.input { keys = { 'up' } }", "keys are key names"),
        (
            "ui.input { keys = { arrowup = 'x' } }",
            "unknown key `arrowup`",
        ),
        (
            "ui.input { keys = { ['ctl-n'] = 'x' } }",
            "unknown modifier `ctl`",
        ),
        (
            "ui.input { keys = { ['ctrl-ctrl-n'] = 'x' } }",
            "`ctrl` twice",
        ),
        ("ui.input { keys = { N = 'x' } }", "unknown key `N`"),
        (
            "ui.input { keys = { ['ctrl-shift-n'] = 'a', ['shift-ctrl-n'] = 'b' } }",
            "the same key as",
        ),
        (
            "ui.input { keys = { up = function() end } }",
            "cannot leave its module",
        ),
        (
            "ui.input { value = true }",
            "input.value: expected a string",
        ),
    ] {
        let error = input_view(view).err();
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.contains(expected)),
            "{view}: {error:?}"
        );
    }
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
        "return style { rules = { ['input:pressed'] = {} } }",
    );
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace.style_error().unwrap().to_string();
    assert!(error.contains("unknown state `pressed`"), "{error}");
    assert!(error.contains("focus, disabled"), "{error}");
}

/// Keeps saved state in memory, and counts the saves.
#[derive(Clone, Default)]
struct Memory(std::rc::Rc<std::cell::RefCell<(std::collections::BTreeMap<String, Data>, usize)>>);

impl crate::Persist for Memory {
    fn state(&self, key: &str) -> Option<Data> {
        self.0.borrow().0.get(key).cloned()
    }

    fn save_state(&self, key: &str, value: &Data) {
        let mut inner = self.0.borrow_mut();
        inner.0.insert(key.to_owned(), value.clone());
        inner.1 += 1;
    }

    fn topics(&self) -> std::collections::BTreeMap<String, Data> {
        self.0
            .borrow()
            .0
            .iter()
            .filter_map(|(key, value)| {
                Some((key.strip_prefix("topic:")?.to_owned(), value.clone()))
            })
            .collect()
    }

    fn save_topic(&self, name: &str, value: &Data) {
        self.0
            .borrow_mut()
            .0
            .insert(format!("topic:{name}"), value.clone());
    }
}

#[test]
fn mod_state_outlives_the_workspace() {
    let dir = app(&[("counter", COUNTER)]);
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    workspace.send("counter", "counter", Data::from("add"));
    workspace.send("counter", "counter", Data::from("add"));
    let saved = memory.0.borrow().0.get("mod:counter").cloned();
    assert_eq!(
        saved,
        Some(Data::Map([("count".to_owned(), Data::Integer(2))].into()))
    );
    // A change saves once; an unchanged state is not saved again.
    let saves = memory.0.borrow().1;
    workspace.reload();
    assert_eq!(memory.0.borrow().1, saves);

    // A new workspace, as at the next launch, starts where the last one left off.
    drop(workspace);
    let workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory)));
    assert_eq!(panel_text(&workspace, "counter"), "count 2");
}

/// A mod that counts, shows the label it was mounted with, and starts from
/// its props' `start`.
const TALLY: &str = r#"
return mod {
  init = function(previous, props)
    return previous or { count = props and props.start or 0 }
  end,
  update = function(state, msg)
    if msg == "add" then state.count += 1 end
  end,
  view = function(state, app, props)
    return ui.text(`{props and props.label or "?"} {state.count}`)
  end,
}
"#;

/// The text an instance's view draws.
fn instance_text(workspace: &Workspace, key: &str) -> String {
    match workspace.slot(key).and_then(crate::Slot::view) {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn a_mod_mounted_twice_keeps_two_states() {
    let dir = app(&[("tally", TALLY)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function()
            return ui.column {
              ui.mount { "tally", id = "a", props = { label = "A" } },
              ui.mount { "tally", id = "b", props = { label = "B", start = 10 } },
            }
        end }"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    assert_eq!(instance_text(&workspace, "tally#a"), "A 0");
    assert_eq!(instance_text(&workspace, "tally#b"), "B 10");
    // A click inside an instance goes to that instance only.
    workspace.send("tally#a", "tally#a", Data::from("add"));
    assert_eq!(instance_text(&workspace, "tally#a"), "A 1");
    assert_eq!(instance_text(&workspace, "tally#b"), "B 10");
}

const CLICKER: &str = r#"
return component {
  title = "Clicker",
  example = { name = "demo" },
  init = function(previous, props) return previous or { clicks = 0 } end,
  update = function(state, msg, props)
    if msg == "click" then
      state.clicks += 1
      return state, { { to = "parent", msg = { clicked = props.name, clicks = state.clicks } } }
    elseif msg == "reset" then
      state.clicks = 0
    end
  end,
  build = function(props, state)
    return ui.button { `{props.name} {state.clicks}`, msg = "click" }
  end,
}
"#;

const USES_CLICKER: &str = r#"
local clicker = use "clicker"
return mod {
  init = function(previous) return previous or { heard = "" } end,
  update = function(state, msg)
    if type(msg) == "table" and msg.clicked then
      state.heard = `{msg.clicked} {msg.clicks}`
      if msg.clicks == 2 then
        return state, { { to = "clicker#one", msg = "reset" } }
      end
    end
  end,
  view = function(state)
    return ui.column {
      clicker { id = "one", name = "one" },
      clicker { id = "two", name = "two" },
      ui.text(state.heard),
    }
  end,
}
"#;

fn button_label(workspace: &Workspace, key: &str) -> String {
    match workspace.slot(key).and_then(crate::Slot::view) {
        Some(Node::Button(button)) => button.label.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn a_component_with_state_keeps_its_own_and_talks_to_its_parent() {
    let dir = app(&[("host", USES_CLICKER)]);
    write(dir.path(), "components/clicker.luau", CLICKER);
    let mut workspace = Workspace::open(dir.path(), false);
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    // Each is the drawing mod's own, under its key.
    assert_eq!(button_label(&workspace, "host/clicker#one"), "one 0");
    assert_eq!(button_label(&workspace, "host/clicker#two"), "two 0");

    workspace.send("host/clicker#one", "host/clicker#one", Data::from("click"));
    assert_eq!(button_label(&workspace, "host/clicker#one"), "one 1");
    assert_eq!(button_label(&workspace, "host/clicker#two"), "two 0");
    // `to = "parent"` reached the mod that drew it.
    let Some(Node::Stack(column)) = workspace.slot("host").and_then(crate::Slot::view) else {
        panic!("no column");
    };
    assert!(matches!(&column.children[2], Node::Text(text) if text.text == "one 1"));

    // The parent reaches its instance by `clicker#one`, and it resets.
    workspace.send("host/clicker#one", "host/clicker#one", Data::from("click"));
    assert_eq!(button_label(&workspace, "host/clicker#one"), "one 0");
}

#[test]
fn an_instance_resumes_its_saved_state_when_drawn_again() {
    let dir = app(&[("tally", TALLY)]);
    // The shell draws the instance only while `shown`.
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          init = function(previous) return previous or { shown = true } end,
          update = function(state, msg) if msg == "flip" then state.shown = not state.shown end end,
          view = function(state)
            return ui.column { state.shown and ui.mount { "tally", id = "kept" } }
          end,
        }"#,
    );
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    workspace.send("tally#kept", "tally#kept", Data::from("add"));
    workspace.send("tally#kept", "tally#kept", Data::from("add"));
    assert!(memory.0.borrow().0.contains_key("mod:tally#kept"));

    // Hidden: the instance is dropped, its state kept.
    workspace.send(SHELL, SHELL, Data::from("flip"));
    assert!(workspace.slot("tally#kept").is_none());
    workspace.send(SHELL, SHELL, Data::from("flip"));
    assert_eq!(instance_text(&workspace, "tally#kept"), "? 2");

    // And across a restart.
    drop(workspace);
    let workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory)));
    assert_eq!(instance_text(&workspace, "tally#kept"), "? 2");
}

#[test]
fn a_stateless_component_cannot_be_mounted_with_an_id() {
    let dir = app(&[]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function()
            return ui.column { ui.mount { "nope", id = "x" } }
        end }"#,
    );
    let workspace = Workspace::open(dir.path(), false);
    let problems: Vec<String> = workspace
        .problems()
        .iter()
        .map(|(name, error)| format!("{name}: {error}"))
        .collect();
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("nope#x") && problem.contains("no mod named `nope`")),
        "{problems:?}"
    );
}

/// Publishes what it is sent on `topic:<msg.topic>`.
const PUBLISHER: &str = r#"
return mod {
  update = function(state, msg)
    return state, { { to = "topic:" .. msg.topic, msg = msg.value } }
  end,
  view = function() return ui.text "publisher" end,
}
"#;

/// Shows the last thing it heard on the topics it subscribes to.
fn listener(subscribe: &str) -> String {
    format!(
        r#"return mod {{
          subscribe = {{ {subscribe} }},
          init = function(previous) return previous or {{ heard = "nothing" }} end,
          update = function(state, msg)
            if type(msg) == "table" and msg.topic then
              state.heard = msg.topic .. "=" .. tostring(type(msg.value) == "table" and msg.value.branch or msg.value)
            end
          end,
          view = function(state, app, props) return ui.text(state.heard) end,
        }}"#
    )
}

fn publish(workspace: &mut Workspace, topic: &str, value: Data) {
    let msg = Data::Map(
        [
            ("topic".to_owned(), Data::from(topic)),
            ("value".to_owned(), value),
        ]
        .into(),
    );
    workspace.send("publisher", "publisher", msg);
}

#[test]
fn a_topic_pushes_to_its_subscribers_and_is_saved() {
    let exact = listener(r#""tabs/main""#);
    let prefix = listener(r#""tabs/*""#);
    let other = listener(r#""theme""#);
    let dir = app(&[
        ("publisher", PUBLISHER),
        ("exact", &exact),
        ("prefix", &prefix),
        ("other", &other),
    ]);
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    publish(&mut workspace, "tabs/main", Data::from("3 tabs"));
    let text = |workspace: &Workspace, name: &str| match workspace
        .slot(name)
        .and_then(crate::Slot::view)
    {
        Some(Node::Text(text)) => text.text.clone(),
        other => panic!("{name}: {other:?}"),
    };
    assert_eq!(text(&workspace, "exact"), "tabs/main=3 tabs");
    assert_eq!(text(&workspace, "prefix"), "tabs/main=3 tabs");
    assert_eq!(text(&workspace, "other"), "nothing");
    assert_eq!(
        memory.0.borrow().0.get("topic:tabs/main"),
        Some(&Data::from("3 tabs"))
    );

    // Provider data is pushed on `provider/<name>`.
    let git = listener(r#""provider/git""#);
    write(dir.path(), "mods/git-watch/main.luau", &git);
    workspace.reload();
    workspace.set_host(
        "git",
        Data::Map([("branch".to_owned(), Data::from("main"))].into()),
    );
    assert_eq!(text(&workspace, "git-watch"), "provider/git=main");

    // A bad name is the sender's problem, and a provider's topic is not a mod's to publish.
    publish(&mut workspace, "has space", Data::Bool(true));
    let error = workspace
        .slot("publisher")
        .unwrap()
        .error()
        .unwrap()
        .to_string();
    assert!(error.contains("a topic is 1 to"), "{error}");
    publish(&mut workspace, "provider/git", Data::Bool(true));
    assert!(workspace.slot("publisher").unwrap().error().is_some());

    // At the next launch the topic is there, and a subscriber hears it as it loads.
    drop(workspace);
    let fresh = Workspace::open_with(dir.path(), false, Some(Box::new(memory)));
    assert_eq!(text(&fresh, "exact"), "tabs/main=3 tabs");
}

#[test]
fn an_instance_mounted_late_catches_up_on_its_topics() {
    let watcher = listener(r#""tabs/main""#);
    let dir = app(&[("publisher", PUBLISHER), ("watcher", &watcher)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod {
          init = function(previous) return previous or { shown = false } end,
          update = function(state, msg) if msg == "show" then state.shown = true end end,
          view = function(state)
            return ui.column { state.shown and ui.mount { "watcher", id = "late" } }
          end,
        }"#,
    );
    let mut workspace = Workspace::open(dir.path(), false);
    publish(&mut workspace, "tabs/main", Data::from("early"));
    workspace.send(SHELL, SHELL, Data::from("show"));
    assert_eq!(instance_text(&workspace, "watcher#late"), "tabs/main=early");
}

/// Copies the folder `from` into `to`, as the app is shipped.
fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Every stack in `node`, depth first.
fn stacks<'n>(node: &'n Node, found: &mut Vec<&'n crate::Stack>) {
    match node {
        Node::Stack(stack) => {
            found.push(stack);
            for child in &stack.children {
                stacks(child, found);
            }
        },
        Node::Panel(panel) => panel.children.iter().for_each(|child| stacks(child, found)),
        Node::Popover(popover) => popover.anchor.iter().for_each(|child| stacks(child, found)),
        Node::Split(split) => split
            .panes
            .iter()
            .flat_map(|pane| &pane.children)
            .for_each(|child| stacks(child, found)),
        _ => {},
    }
}

#[test]
fn the_apps_tabs_are_keyed_and_the_curved_look_joins_the_main_card() {
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    for look in ["pills", "curved"] {
        let dir = tempfile::tempdir().unwrap();
        copy_folder(&shipped, dir.path());
        let sheet = dir.path().join("style.luau");
        let source = fs::read_to_string(&sheet).unwrap();
        assert!(source.contains(r#"looks = { tabs = "pills" }"#));
        let source = source.replace(
            r#"looks = { tabs = "pills" }"#,
            &format!(r#"looks = {{ tabs = "{look}" }}"#),
        );
        fs::write(&sheet, source).unwrap();
        let workspace = Workspace::open_with(dir.path(), false, Some(Box::new(Memory::default())));
        assert!(
            workspace.problems().is_empty() && workspace.style_error().is_none(),
            "{look}: {:?} {:?}",
            workspace.problems(),
            workspace.style_error()
        );

        let strip = workspace.slot("shell/tabbed#main").unwrap().view().unwrap();
        let mut found = Vec::new();
        stacks(strip, &mut found);
        let keys: Vec<_> = found
            .iter()
            .filter_map(|stack| stack.key.as_deref())
            .collect();
        assert_eq!(keys, ["main-1", "_add"], "{look}");
        let shoulders = found
            .iter()
            .filter(|stack| {
                stack
                    .look
                    .class
                    .iter()
                    .any(|class| class == "tabs-shoulder")
            })
            .count();
        assert_eq!(
            shoulders,
            if look == "curved" {
                2
            } else {
                0
            },
            "{look}"
        );

        let shell = workspace.slot(SHELL).unwrap().view().unwrap();
        let mut found = Vec::new();
        stacks(shell, &mut found);
        let joined = found
            .iter()
            .filter(|stack| {
                stack
                    .look
                    .class
                    .iter()
                    .any(|class| class == "toolbar-joined")
            })
            .count();
        // The center's bar, and one over each side panel.
        assert_eq!(
            joined,
            if look == "curved" {
                3
            } else {
                0
            },
            "{look}"
        );
    }
}

#[test]
fn the_layout_menu_picks_the_tab_look_and_moves_the_title_bar() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let mut workspace =
        Workspace::open_with(root.as_path(), false, Some(Box::new(Memory::default())));
    let classes = |workspace: &Workspace, name: &str, class: &str| {
        let mut found = Vec::new();
        stacks(workspace.slot(name).unwrap().view().unwrap(), &mut found);
        found
            .iter()
            .filter(|stack| stack.look.class.iter().any(|own| own == class))
            .count()
    };
    let joined_card = |workspace: &Workspace| {
        matches!(workspace.slot("editor").unwrap().view(),
            Some(Node::Panel(panel)) if panel.look.class.iter().any(|class| class == "tab-card-joined"))
    };
    assert_eq!(classes(&workspace, SHELL, "toolbar-joined"), 0);
    assert!(!joined_card(&workspace));

    // The title bar's button opens the picker in a window of its own.
    let mut found = Vec::new();
    stacks(workspace.slot(SHELL).unwrap().view().unwrap(), &mut found);
    assert!(
        found
            .iter()
            .any(|stack| stack.action == Some(Action::Window("looks".to_owned())))
    );

    let pick = |key: &str, value: &str| Data::Map([(key.to_owned(), Data::from(value))].into());
    workspace.send("looks", "looks", pick("look", "curved"));
    assert_eq!(classes(&workspace, SHELL, "toolbar-joined"), 3);
    assert_eq!(classes(&workspace, "shell/tabbed#main", "tabs-shoulder"), 2);
    assert!(joined_card(&workspace), "{:?}", workspace.problems());

    // Back to the stylesheet's look, but with the bar over the editor.
    workspace.send("looks", "looks", pick("look", "stylesheet"));
    workspace.send("looks", "looks", pick("layout", "joined"));
    assert_eq!(classes(&workspace, "shell/tabbed#main", "tabs-shoulder"), 0);
    assert_eq!(classes(&workspace, SHELL, "toolbar-joined"), 3);
    assert!(joined_card(&workspace));

    // A mount names a mod, never a path.
    let dir = app(&[(
        "escape",
        r#"return mod { view = function() return ui.column { ui.mount { "../../etc", id = "x" } } end }"#,
    )]);
    let workspace = Workspace::open(dir.path(), false);
    assert!(broken(&workspace, "escape").contains("is not a mod name"));
    assert!(workspace.roots().mod_candidates("../escape").is_empty());

    // `window` stands alone on a control.
    let dir = app(&[(
        "bad",
        r#"return mod { view = function() return ui.row { window = "looks", msg = "x" } end }"#,
    )]);
    let workspace = Workspace::open(dir.path(), false);
    let error = workspace
        .slot("bad")
        .unwrap()
        .error()
        .map(ToString::to_string);
    assert!(
        error
            .as_deref()
            .is_some_and(|error| error.contains("takes no `msg`")),
        "{error:?}"
    );
}

#[test]
fn the_apps_tab_bar_opens_kinds_keeps_its_tabs_and_publishes_them() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(root.as_path(), false, Some(Box::new(memory.clone())));
    // The title bar's strip is the shell's; the main area draws its content.
    let key = "shell/tabbed#main";
    let tabs = |workspace: &Workspace| -> Vec<(String, String)> {
        let Some(Data::Map(state)) = memory.0.borrow().0.get(&format!("mod:{key}")).cloned() else {
            panic!("no saved tabs; problems: {:?}", workspace.problems());
        };
        let Some(Data::List(tabs)) = state.get("tabs").cloned() else {
            panic!("no tabs: {state:?}");
        };
        tabs.into_iter()
            .map(|tab| match tab {
                Data::Map(tab) => match (tab.get("id"), tab.get("kind")) {
                    (Some(Data::String(id)), Some(Data::String(kind))) => {
                        (id.clone(), kind.clone())
                    },
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            })
            .collect()
    };
    // It starts with a terminal, and its kinds include the browser and the explorer.
    assert_eq!(tabs(&workspace), [(
        "main-1".to_owned(),
        "component:terminal".to_owned()
    )]);
    assert!(workspace.slot(key).is_some(), "{:?}", workspace.problems());

    // + then a pick opens a browser tab, selected, drawn as the browser component.
    workspace.send(key, key, Data::from("add"));
    let pick = Data::Map([("pick".to_owned(), Data::from("component:browser"))].into());
    workspace.send(key, key, pick);
    assert_eq!(
        tabs(&workspace)[1],
        ("main-2".to_owned(), "component:browser".to_owned())
    );
    // The change is published on its topic.
    let topic = memory.0.borrow().0.get("topic:tabs/main").cloned();
    assert!(
        matches!(&topic, Some(Data::Map(fields)) if fields.get("count") == Some(&Data::Integer(2))),
        "{topic:?}"
    );

    // The main area followed the strip over `tabs/main`: it draws the browser.
    let editor = memory.0.borrow().0.get("mod:editor").cloned();
    assert!(
        matches!(&editor, Some(Data::Map(state)) if matches!(state.get("tab"), Some(Data::Map(tab)) if tab.get("id") == Some(&Data::from("main-2")))),
        "{editor:?}"
    );
    // What the browser reports there comes back to the strip, on the topic.
    let report = Data::Map(
        [
            ("tab_report".to_owned(), Data::from("main-2")),
            ("url".to_owned(), Data::from("https://example.com/")),
            ("title".to_owned(), Data::from("Example")),
        ]
        .into(),
    );
    workspace.send("editor", "editor", report);
    let strip = memory.0.borrow().0.get(&format!("mod:{key}")).cloned();
    assert!(format!("{strip:?}").contains("Example"), "{strip:?}");

    // A mod kind: the explorer opens as an instance of its own.
    let pick = Data::Map([("pick".to_owned(), Data::from("mod:explorer"))].into());
    workspace.send(key, key, pick);
    assert!(
        workspace.slot("explorer#main-3").is_some(),
        "{:?}",
        workspace.problems()
    );

    // Closing the browser keeps the others; and they come back at the next launch.
    workspace.send(
        key,
        key,
        Data::Map([("close".to_owned(), Data::from("main-2"))].into()),
    );
    assert_eq!(tabs(&workspace).len(), 2);
    drop(workspace);
    let workspace = Workspace::open_with(root.as_path(), false, Some(Box::new(memory.clone())));
    assert_eq!(tabs(&workspace).len(), 2);
    assert!(
        workspace.slot("explorer#main-3").is_some(),
        "the selected explorer tab is drawn again"
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
fn a_previews_instances_stay_inside_the_preview() {
    // Sends everywhere it should not, when told to.
    let leaky = r#"return mod {
      update = function(state, msg)
        if msg == "go" then
          return state, {
            { to = "topic:leak", msg = true },
            { to = "tally#kept", msg = "add" },
            { to = "provider:files", msg = { open = "/" } },
          }
        end
      end,
      view = function() return ui.text "leaky" end,
    }"#;
    let dir = app(&[("tally", TALLY), ("leaky", leaky)]);
    write(
        dir.path(),
        "main.luau",
        r#"return mod { view = function() return ui.column { ui.mount { "tally", id = "kept" } } end }"#,
    );
    // A component whose preview mounts the same ids as the real app.
    write(
        dir.path(),
        "components/mounter.luau",
        r#"return component {
          example = { label = "x" },
          build = function(props)
            return ui.column { ui.mount { "tally", id = "kept" }, ui.mount { "leaky", id = "one" } }
          end,
        }"#,
    );
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(dir.path(), false, Some(Box::new(memory.clone())));
    workspace.send("tally#kept", "tally#kept", Data::from("add"));
    assert_eq!(instance_text(&workspace, "tally#kept"), "? 1");
    // The preview's copy is its own, not the real instance.
    assert_eq!(instance_text(&workspace, "demo:mounter/tally#kept"), "? 0");

    workspace.send(
        "demo:mounter/leaky#one",
        "demo:mounter/leaky#one",
        Data::from("go"),
    );
    assert_eq!(
        instance_text(&workspace, "tally#kept"),
        "? 1",
        "the real instance heard it"
    );
    assert!(
        !memory.0.borrow().0.contains_key("topic:leak"),
        "the topic was published"
    );
    assert!(workspace.take_outbox().is_empty(), "a provider was sent to");
    assert!(
        !memory.0.borrow().0.keys().any(|key| key.contains("demo:")),
        "a preview's state was saved"
    );
}
