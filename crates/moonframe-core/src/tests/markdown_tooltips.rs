use super::*;

#[test]
fn a_markdown_takes_tooltips_by_address() {
    let source = r#"return mod { view = function() return ui.markdown {
        text = "[a](https://a.example)", tooltips = { ["https://a.example"] = "Leads to b" } } end }"#;
    let Ok(Node::Markdown(markdown)) = view_of(source) else {
        panic!("expected a markdown");
    };
    assert_eq!(
        markdown
            .tooltips
            .get("https://a.example")
            .map(String::as_str),
        Some("Leads to b")
    );
    let bad = r#"return mod { view = function() return ui.markdown { text = "", tooltips = { "a" } } end }"#;
    assert!(view_of(bad).is_err());
}
