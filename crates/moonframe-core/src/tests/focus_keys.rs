use super::*;

#[test]
fn a_row_or_column_keeps_its_keys() {
    let source = r#"return mod { view = function()
      return ui.column { keys = { left = "dismiss", right = { keep = true } }, "card" } end }"#;
    let Ok(Node::Stack(column)) = view_of(source) else {
        panic!("no column");
    };
    assert_eq!(column.keys.0.len(), 2);
    assert_eq!(column.keys.0[0].1, Data::from("dismiss"));
    // Named as overlays name them, and refusing what types text.
    let typing =
        r#"return mod { view = function() return ui.row { keys = { j = "down" }, "x" } end }"#;
    let error = view_of(typing).unwrap_err();
    assert!(error.contains("types text"), "{error}");
}
