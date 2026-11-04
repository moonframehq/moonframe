use super::*;

#[test]
fn waveform_chunks_fit_the_host_and_keep_every_column_connected() {
    for columns in [100, 101, 199, 200, 256] {
        let source = format!(
            r#"
local waveform = use "waveform"
return mod {{ view = function()
    return waveform {{ lines = {{}}, channels = {{ {{
        low = table.create({columns}, 0.25), high = table.create({columns}, 0.75),
    }} }} }}
end }}
"#
        );
        let dir = app(&[("scope", &source)]);
        write(
            dir.path(),
            "components/waveform.luau",
            include_str!("../../../../app/components/waveform.luau"),
        );
        let workspace = Workspace::open(dir.path(), false);
        let slot = workspace.slot("scope").unwrap();
        assert!(slot.error().is_none(), "{columns}: {:?}", slot.error());
        let Some(Node::Paths(paths)) = slot.view() else {
            panic!("expected waveform paths");
        };
        let mut points = 0;
        let mut previous = None;
        for path in &paths.paths {
            assert!(path.points.len() <= 200);
            if let Some(last) = previous {
                assert_eq!(path.points.first(), Some(&last));
                points -= 1;
            }
            points += path.points.len();
            previous = path.points.last().copied();
        }
        assert_eq!(points, columns * 2);
    }
}

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
fn component_file_names_are_checked() {
    let dir = app(&[]);
    write(dir.path(), "components/My Card.luau", BADGE_COMPONENT);
    let workspace = Workspace::open(dir.path(), false);
    let component = workspace
        .components()
        .iter()
        .find(|component| component.name == "My Card")
        .unwrap();
    let error = component.preview.as_ref().unwrap_err().to_string();
    assert!(error.contains("a-z"), "{error}");
}
