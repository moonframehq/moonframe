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
