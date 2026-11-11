use {
    super::*,
    crate::{Element, Space},
};

#[test]
fn every_element_the_reader_knows_is_in_the_schema() {
    let schema = schema();
    for name in Element::NAMES {
        let element = schema
            .element(name)
            .unwrap_or_else(|| panic!("no `{name}` in the schema"));
        assert!(
            element.keys.iter().all(|key| key.name != "type"),
            "`type` is the tag ui sets"
        );
    }
    assert_eq!(schema.elements.len(), Element::NAMES.len());
    assert_eq!(schema.version, SCHEMA_VERSION);
}

#[test]
fn row_padding_lists_the_space_names() {
    let schema = schema();
    let row = schema.element("row").unwrap();
    assert!(row.children);
    let padding = row.keys.iter().find(|key| key.name == "padding").unwrap();
    assert_eq!(
        padding.kind,
        KeyType::Tokens(TokenSet {
            name: "Space",
            names: Space::NAMES,
        })
    );
    let text = schema.element("text").unwrap();
    assert!(!text.children);
    assert!(text.keys.iter().any(|key| key.name == "tone"));
}

#[test]
fn icons_styles_and_tokens_are_listed() {
    let schema = schema();
    assert!(schema.icons.contains(&"check"));
    assert!(schema.style.properties.contains(&"background"));
    assert!(schema.style.sheet.contains(&"rules"));
    let tone = schema.tokens.iter().find(|set| set.name == "Tone").unwrap();
    assert!(tone.names.contains(&"muted"));
}

#[test]
fn the_schema_writes_itself_as_data_and_json() {
    let schema = schema();
    let data = schema.to_data();
    let padding = data
        .get("elements")
        .and_then(|elements| elements.get("row"))
        .and_then(|row| row.get("keys"))
        .and_then(|keys| keys.get("padding"))
        .unwrap();
    assert_eq!(padding.get("type").and_then(Data::as_str), Some("tokens"));
    assert_eq!(padding.get("set").and_then(Data::as_str), Some("Space"));
    let json = schema.to_json();
    assert!(json.starts_with('{') && json.ends_with('}'));
    assert!(json.contains("\"version\":1"));
    assert!(json.contains("\"check\""));
}

#[test]
fn nearest_suggests_close_names_best_first() {
    let keys = ["padding", "gap", "grow", "width", "max_width", "height"];
    assert_eq!(nearest("paddin", keys), vec!["padding"]);
    assert_eq!(nearest("widht", keys)[0], "width");
    assert!(nearest("width", keys).contains(&"max_width"));
    assert!(nearest("zzzzzz", keys).is_empty());
    let many = ["ab", "abc", "abd", "abe", "abf", "abg", "abh"];
    assert_eq!(nearest("ab", many).len(), 5);
    assert_eq!(nearest("ab", many)[0], "ab");
}
