use {
    super::*,
    crate::{Document, DocumentScroll, DocumentScrollTo, DocumentSource},
};

fn document(keys: &str) -> Result<Document, String> {
    let source =
        format!(r#"return mod {{ view = function() return ui.document {{ {keys} }} end }}"#);
    match view_of(&source)? {
        Node::Document(document) => Ok(document),
        other => panic!("expected a document, got {other:?}"),
    }
}

#[test]
fn a_document_takes_its_html_or_a_file_and_the_messages_it_sends() {
    let shown = document(r#"html = "<h1>acme</h1>", link = "link", scroll = { at = 1 }"#).unwrap();
    assert_eq!(shown.id, "document");
    assert_eq!(shown.source, DocumentSource::Html("<h1>acme</h1>".into()));
    assert_eq!(shown.link, Some(Data::from("link")));
    assert!(shown.scroll.is_some() && shown.select.is_none() && !shown.editable);
    let file = document(r#"id = "brief", src = "notes/acme.html", editable = true, change = "edited", done = "done""#)
        .unwrap();
    assert_eq!(file.id, "brief");
    assert_eq!(file.source, DocumentSource::File("notes/acme.html".into()));
    assert!(file.editable && file.change.is_some() && file.done.is_some());
    // The HTML is its main value.
    let positional =
        view_of(r#"return mod { view = function() return ui.document "<p>acme</p>" end }"#);
    assert!(matches!(
        positional,
        Ok(Node::Document(Document {
            source: DocumentSource::Html(_),
            ..
        }))
    ));
}

#[test]
fn a_document_needs_one_source() {
    let neither = document(r#"link = "link""#).unwrap_err();
    assert!(neither.contains("`html`"), "{neither}");
    let both = document(r#"html = "<p></p>", src = "a.html""#).unwrap_err();
    assert!(both.contains("not both"), "{both}");
}

#[test]
fn a_document_scrolls_to_a_y_or_an_id() {
    let to = |keys: &str| {
        document(&format!(r#"html = "<p></p>", scroll_to = {keys}"#)).map(|d| d.scroll_to)
    };
    let asked = |to, seq| Ok(Some(DocumentScrollTo { to, seq }));
    assert_eq!(to("480"), asked(DocumentScroll::Y(480.0), Data::Nil));
    assert_eq!(
        to(r#""part-2""#),
        asked(DocumentScroll::Id("part-2".into()), Data::Nil)
    );
    assert_eq!(
        to(r#"{ id = "part-2", seq = 3 }"#),
        asked(DocumentScroll::Id("part-2".into()), Data::Integer(3))
    );
    assert_eq!(to("{ y = 12 }"), asked(DocumentScroll::Y(12.0), Data::Nil));
    for (bad, said) in [
        ("-1", "from 0"),
        (r#""""#, "an element's id"),
        (r#"{ y = 1, id = "a" }"#, "not both"),
        ("{ seq = 1 }", "takes `y`"),
        ("true", "expected points"),
    ] {
        let error = to(bad).unwrap_err();
        assert!(error.contains(said), "{bad}: {error}");
    }
}
