use super::*;

#[test]
fn a_style_sets_text_size_in_pixels_inline_and_in_rules() {
    let source = r#"return mod { view = function()
  return ui.panel {
    ui.text { "a", style = { font_size = 14, size = "small" } },
    ui.text { "b", class = "sized" },
    ui.text { "c", class = "sized", style = { font_size = 18 } },
  }
end }"#;
    let dir = app(&[("probe", source)]);
    write(
        dir.path(),
        "style.luau",
        r#"return style { rules = { [".sized"] = { font_size = 11 } } }"#,
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
    let sheet = workspace.stylesheet();
    let sizes: Vec<_> = panel
        .children
        .iter()
        .map(|child| {
            let Node::Text(text) = child else {
                panic!("expected texts");
            };
            let style = sheet
                .resolve(Part::Text, &text.look, false, &Rules::default())
                .base;
            (style.size, style.font_size)
        })
        .collect();
    assert_eq!(sizes, [
        (Some(crate::TextSize::Small), Some(14.0)),
        (None, Some(11.0)),
        (None, Some(18.0)),
    ]);
}

#[test]
fn a_font_size_outside_its_range_is_refused() {
    let tiny = r#"return mod { view = function() return ui.text { "x", style = { font_size = 2 } } end }"#;
    let error = view_of(tiny).unwrap_err();
    assert!(error.contains("font_size"), "{error}");
}
