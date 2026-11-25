use super::*;

#[test]
fn a_split_takes_resize_and_sends_the_sizes_where_to_says() {
    let split = |source: &str| match view_of(source) {
        Ok(Node::Split(split)) => Ok(split),
        Ok(other) => panic!("expected a split, got {other:?}"),
        Err(error) => Err(error),
    };
    let source = r#"return mod { view = function() return ui.hsplit { resize = "sized", to = "layout",
        ui.pane { size = 200, "a" }, ui.pane { "b" } } end }"#;
    let Ok(crate::Split {
        resize: Some(Action::Send { to, msg }),
        ..
    }) = split(source)
    else {
        panic!("resize kept");
    };
    assert_eq!(to.as_deref(), Some("layout"));
    let sent = crate::Split::resized(&msg, [212.4, 787.6]);
    let Data::Map(sent) = sent else {
        panic!("a table");
    };
    assert_eq!(sent.get("msg"), Some(&Data::from("sized")));
    assert_eq!(
        sent.get("sizes"),
        Some(&Data::List(vec![Data::Integer(212), Data::Integer(788)]))
    );
    let alone = r#"return mod { view = function() return ui.vsplit { to = "layout", ui.pane { "a" } } end }"#;
    assert!(
        split(alone)
            .unwrap_err()
            .contains("`to` goes with `resize`")
    );
}

#[test]
fn a_pane_takes_a_reset_size_apart_from_its_size() {
    let source = r#"return mod { view = function() return ui.hsplit { resize = "sized",
        ui.pane { size = 320, reset = 256, "notes" }, ui.pane { "note" } } end }"#;
    let Ok(Node::Split(split)) = view_of(source) else {
        panic!("a split");
    };
    let sizes: Vec<_> = split
        .panes
        .iter()
        .map(|pane| (pane.size, pane.reset))
        .collect();
    assert_eq!(sizes, [(Some(320.), Some(256.)), (None, None)]);
    let negative = r#"return mod { view = function() return ui.hsplit {
        ui.pane { reset = -1, "a" }, ui.pane { "b" } } end }"#;
    assert!(view_of(negative).unwrap_err().contains("reset"));
}
