//! The shell `crates/moonframe-rune/src/tests.rs` runs too, here on GPUI.

use super::*;

const SHELL: &str = include_str!("../../../moonframe-core/fixtures/both-backends.luau");

#[gpui_kit::test]
fn one_luau_source_clicks_and_types(cx: &mut TestAppContext) {
    let mut window = open(cx, SHELL);
    assert!(!window.view().contains("The body shows"));
    window.click("collapsible-header");
    assert!(window.view().contains("The body shows"));
    window.click("both-name");
    window.typed("Ada");
    assert!(window.view().contains("Hello Ada"), "{}", window.view());
}

#[gpui_kit::test]
fn a_spread_mods_children_are_members_of_the_row_region(cx: &mut TestAppContext) {
    let dir = folder(
        r#"return mod {
            view = function()
                return ui.row {
                    height = 30,
                    ui.region { name = "status", direction = "row" },
                }
            end,
        }"#,
    );
    let meters = dir.path().join("mods/meters");
    std::fs::create_dir_all(&meters).unwrap();
    let source = r#"return mod {
        region = "status",
        view = function()
            return ui.row {
                spread = true,
                ui.row { class = "meter-one", width = 5000, shrink = false },
                ui.row { class = "meter-two", width = 5000, shrink = false },
            }
        end,
    }"#;
    std::fs::write(meters.join("main.luau"), source).unwrap();
    let mut window = open_in(cx, dir);
    let (first, second) = (window.bounds("meter-one"), window.bounds("meter-two"));
    // Too wide for the row together: the second wraps away by itself.
    assert!(
        second.origin.y >= first.origin.y + px(30.),
        "{first:?} {second:?}"
    );
}
