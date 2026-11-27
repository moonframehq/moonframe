use {
    super::{Reported, State, Work, read, sealed},
    moonframe_core::{Data, Document, DocumentScroll, DocumentScrollTo, DocumentSource, Look},
};

fn element(editable: bool, scroll_to: Option<DocumentScroll>) -> Document {
    Document {
        look: Look::default(),
        id: "acme".into(),
        source: DocumentSource::Html(String::new()),
        link: Some(Data::from("link")),
        scroll: Some(Data::from("scroll")),
        scroll_to: scroll_to.map(|to| DocumentScrollTo { to, seq: Data::Nil }),
        select: Some(Data::from("select")),
        editable,
        change: Some(Data::from("change")),
        done: Some(Data::from("done")),
    }
}

fn field(msg: &Data, key: &str) -> Option<Data> {
    match msg {
        Data::Map(map) => map.get(key).cloned(),
        _ => None,
    }
}

#[test]
fn nothing_loads_until_the_view_is_sealed() {
    let mut state = State::default();
    state.update(&element(false, None), "<p>acme</p>", false);
    assert_eq!(state.work(None), Work::default());
    state.seal();
    let work = state.work(None);
    assert_eq!(work.load, Some(sealed("<p>acme</p>")));
    assert_eq!(work.dark, Some(false));
    // Loaded and told its look: nothing more until it changes.
    assert_eq!(state.work(Some(false)), Work::default());
    state.update(&element(false, None), "<p>acme</p>", true);
    assert_eq!(state.work(Some(false)).dark, Some(true));
}

#[test]
fn once_loaded_it_takes_the_editing_switch_and_scrolls_once() {
    let mut state = State::default();
    state.seal();
    state.update(
        &element(true, Some(DocumentScroll::Id("part".into()))),
        "<p>a</p>",
        false,
    );
    assert!(state.work(Some(false)).load.is_some());
    // Not loaded yet: the calls wait.
    assert!(state.work(Some(false)).calls.is_empty());
    state.loaded();
    let calls = state.work(Some(false)).calls;
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "setEditable");
    assert_eq!(calls[1], ("scrollToId", serde_json::json!("part")));
    assert!(state.work(Some(false)).calls.is_empty());
    // The same request again does nothing; a new one scrolls.
    state.update(
        &element(true, Some(DocumentScroll::Id("part".into()))),
        "<p>a</p>",
        false,
    );
    assert!(state.work(Some(false)).calls.is_empty());
    state.update(
        &element(true, Some(DocumentScroll::Y(40.0))),
        "<p>a</p>",
        false,
    );
    assert_eq!(state.work(Some(false)).calls, vec![(
        "scrollToY",
        serde_json::json!(40.0)
    )]);
}

#[test]
fn the_html_an_editor_reported_does_not_load_again() {
    let mut state = State::default();
    state.seal();
    state.update(&element(true, None), "<p>a</p>", false);
    state.work(Some(false));
    state.loaded();
    state.work(Some(false));
    let changed = state
        .report(Reported::Change {
            html: "<p>ab</p>".into(),
            large: false,
        })
        .unwrap();
    assert_eq!(field(&changed, "html"), Some(Data::from("<p>ab</p>")));
    assert_eq!(field(&changed, "msg"), Some(Data::from("change")));
    // The mod keeps it and gives it back: the caret stays where it is.
    state.update(&element(true, None), "<p>ab</p>", false);
    assert_eq!(state.work(Some(false)).load, None);
    // The HTML from before the edit loads again: a reset.
    state.update(&element(true, None), "<p>a</p>", false);
    assert_eq!(state.work(Some(false)).load, Some(sealed("<p>a</p>")));
    // Something else entirely loads.
    state.update(&element(true, None), "<p>new</p>", false);
    assert_eq!(state.work(Some(false)).load, Some(sealed("<p>new</p>")));
}

#[test]
fn an_edit_too_large_to_give_back_says_so() {
    let mut state = State::default();
    state.seal();
    state.update(&element(true, None), "<p>a</p>", false);
    state.work(Some(false));
    let large = state
        .report(Reported::Change {
            html: String::new(),
            large: true,
        })
        .unwrap();
    assert!(field(&large, "html").is_none());
    assert!(
        matches!(field(&large, "error"), Some(Data::String(error)) if error.contains("larger"))
    );
    let over = "a".repeat(moonframe_core::MAX_HTML + 1);
    let done = state
        .report(Reported::Done {
            html: over,
            large: false,
        })
        .unwrap();
    assert!(field(&done, "error").is_some());
    // The view's document stays the mod's: nothing reloads.
    assert_eq!(state.work(Some(false)).load, None);
}

#[test]
fn edits_are_told_only_while_editable() {
    let mut state = State::default();
    state.update(&element(false, None), "<p>a</p>", false);
    let done = || Reported::Done {
        html: "x".into(),
        large: false,
    };
    assert_eq!(state.report(done()), None);
    state.update(&element(true, None), "<p>a</p>", false);
    let done = state.report(done()).unwrap();
    assert_eq!(field(&done, "html"), Some(Data::from("x")));
}

#[test]
fn reports_become_the_messages_asked_for() {
    let mut state = State::default();
    state.update(&element(false, None), "", false);
    let link = state
        .report(Reported::Link {
            url: "https://acme.example/".into(),
            text: "Acme".into(),
        })
        .unwrap();
    assert_eq!(
        field(&link, "url"),
        Some(Data::from("https://acme.example/"))
    );
    let scroll = state
        .report(Reported::Scroll {
            y: 120.4,
            height: 300.0,
            content: 900.0,
        })
        .unwrap();
    assert_eq!(field(&scroll, "y"), Some(Data::Number(120.0)));
    assert_eq!(field(&scroll, "content_height"), Some(Data::Number(900.0)));
    let select = state
        .report(Reported::Select {
            text: "acme".into(),
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        })
        .unwrap();
    assert_eq!(field(&select, "text"), Some(Data::from("acme")));
    assert_eq!(field(&select, "width"), Some(Data::Number(3.0)));
}

#[test]
fn only_what_the_script_reports_is_read() {
    assert_eq!(
        read(r#"{"kind":"link","url":"https://acme.example/"}"#),
        Some(Reported::Link {
            url: "https://acme.example/".into(),
            text: String::new()
        })
    );
    assert_eq!(read(r#"{"kind":"open","url":"x"}"#), None);
    assert_eq!(read(r#"{"kind":"link","url":"x","extra":1}"#), None);
    assert_eq!(read("not json"), None);
    let huge = format!(
        r#"{{"kind":"change","html":"{}"}}"#,
        "a".repeat(super::MAX_REPORT)
    );
    assert_eq!(read(&huge), None);
}

#[test]
fn html_the_mod_leaves_as_it_was_keeps_the_edits() {
    let mut state = State::default();
    state.seal();
    let mut only_done = element(true, None);
    only_done.change = None;
    state.update(&only_done, "<p>a</p>", false);
    state.work(Some(false));
    state.loaded();
    state.work(Some(false));
    // Edited, with no `change` asked for: the mod's HTML stays as it was.
    assert_eq!(
        state.report(Reported::Change {
            html: "<p>ab</p>".into(),
            large: false,
        }),
        None
    );
    state.update(&only_done, "<p>a</p>", false);
    assert_eq!(state.work(Some(false)).load, None);
}
