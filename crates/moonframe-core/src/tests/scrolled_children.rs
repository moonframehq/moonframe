use super::*;

/// Whether each child of the view's stack shrinks.
fn shrinks(source: &str) -> Vec<bool> {
    let Node::Stack(stack) = view_of(source).unwrap() else {
        panic!("expected a stack");
    };
    stack
        .children
        .iter()
        .map(|child| match child {
            Node::Stack(stack) => stack.frame.shrink,
            Node::Panel(panel) => panel.frame.shrink,
            other => panic!("expected a stack or panel, got {other:?}"),
        })
        .collect()
}

#[test]
fn the_children_of_a_scrolling_column_keep_their_height() {
    let source = r#"return mod { view = function() return ui.column { scroll = true,
        ui.row { "a" },
        ui.column { "b" },
        ui.panel { title = "c", "c" },
        ui.row { shrink = true, "d" },
        ui.column { scroll = true, "e" },
        ui.row { scroll = true, "f" },
    } end }"#;
    assert_eq!(shrinks(source), [false, false, false, true, true, false]);
}

#[test]
fn only_scrolling_along_the_axis_keeps_the_size() {
    let sideways =
        r#"return mod { view = function() return ui.column { scroll = "x", ui.row { "a" } } end }"#;
    assert_eq!(shrinks(sideways), [true]);
    let along =
        r#"return mod { view = function() return ui.row { scroll = "x", ui.column { "a" } } end }"#;
    assert_eq!(shrinks(along), [false]);
    let both = r#"return mod { view = function() return ui.row { scroll = "both", ui.column { "a" } } end }"#;
    assert_eq!(shrinks(both), [false]);
    let still = r#"return mod { view = function() return ui.column { ui.row { "a" } } end }"#;
    assert_eq!(shrinks(still), [true]);
}

#[test]
fn a_scrolling_panel_keeps_its_children_height() {
    let source =
        r#"return mod { view = function() return ui.panel { scroll = true, ui.row { "a" } } end }"#;
    let Node::Panel(panel) = view_of(source).unwrap() else {
        panic!("expected a panel");
    };
    assert!(matches!(&panel.children[..], [Node::Stack(row)] if !row.frame.shrink));
}
