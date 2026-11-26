//! A style value out of its range is a problem of the workspace as it
//! loads, wherever the rule is: what `--check` prints, so a value the live
//! window refuses never passes the check.

use super::*;

const CHIP: &str = r#"return component { title = "Chip", when = "a pill", styles = { [".chip"] = { STYLE } },
    build = function() return ui.row { class = "chip", ui.text "acme" } end }"#;

/// The problems of an app whose rule, wherever `place` puts it, is `style`.
fn problems(place: &str, style: &str) -> Vec<String> {
    let (main, component) = match place {
        "mod" => (
            format!(
                r#"return mod {{ styles = {{ [".chip"] = {{ {style} }} }}, view = function() return ui.row {{ class = "chip", ui.text "acme" }} end }}"#
            ),
            None,
        ),
        "component" => (
            r#"local chip = use "chip"
return mod { view = function() return chip {} end }"#
                .to_owned(),
            Some(CHIP.replace("STYLE", style)),
        ),
        _ => (
            format!(
                r#"return mod {{ view = function() return ui.row {{ style = {{ {style} }}, ui.text "acme" }} end }}"#
            ),
            None,
        ),
    };
    let dir = app(&[("acme", &main)]);
    if let Some(component) = component {
        write(dir.path(), "components/chip.luau", &component);
    }
    let workspace = Workspace::open(dir.path(), false);
    workspace
        .problems()
        .iter()
        .map(|(name, error)| format!("{name}: {error}"))
        .collect()
}

#[test]
fn a_style_value_out_of_range_is_a_problem_wherever_it_is() {
    for place in ["mod", "component", "element"] {
        for (style, said) in [
            ("radius = 5000", "a radius is from 0 to 1000"),
            ("font_size = 200", "expected 6 to 64"),
            ("opacity = 3", "expected 0 to 1"),
            ("padding = -4", "expected 0 to 1000"),
        ] {
            let found = problems(place, style);
            assert!(
                found.iter().any(|problem| problem.contains(said)),
                "{place} {style}: {found:?}"
            );
        }
        // The largest radius, a pill's, is in range.
        assert_eq!(
            problems(place, "radius = 999"),
            Vec::<String>::new(),
            "{place}"
        );
    }
}
