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
