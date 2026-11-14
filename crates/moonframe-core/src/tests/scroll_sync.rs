//! `scrolled` and `scroll_at`: a source field and its preview tell and take
//! how far down their text they are scrolled.

use {
    super::*,
    crate::{Input, Markdown, ScrollAt},
};

fn view(element: &str) -> Result<Node, String> {
    view_of(&format!(
        "return mod {{ view = function() return {element} end }}"
    ))
}

#[test]
fn a_source_and_its_preview_tell_and_take_their_scroll() {
    let Ok(Node::Input(source)) = view(
        "ui.input { multiline = true, scrolled = 'source', \
         scroll_at = { fraction = 0.25, seq = 2 }, to = 'editor' }",
    ) else {
        panic!("no input");
    };
    assert_eq!(source.scrolled, Some(Data::from("source")));
    assert_eq!(
        source.scroll_at,
        Some(ScrollAt {
            fraction: 0.25,
            seq: 2
        })
    );
    let Ok(Node::Markdown(preview)) = view(
        "ui.markdown { '# A', scroll = true, scrolled = { pane = 'preview' }, \
         scroll_at = { fraction = 1 } }",
    ) else {
        panic!("no markdown");
    };
    assert_eq!(
        preview.scroll_at,
        Some(ScrollAt {
            fraction: 1.0,
            seq: 0
        })
    );
    // A table message gets `fraction`; anything else is wrapped.
    assert_eq!(
        ScrollAt::message(preview.scrolled.as_ref().unwrap(), 0.5),
        Data::Map(
            [
                ("pane".to_owned(), Data::from("preview")),
                ("fraction".to_owned(), Data::Number(0.5)),
            ]
            .into()
        )
    );
    assert_eq!(
        ScrollAt::message(&Data::from("source"), 0.0),
        Data::Map(
            [
                ("msg".to_owned(), Data::from("source")),
                ("fraction".to_owned(), Data::Number(0.0)),
            ]
            .into()
        )
    );
    // As Luau writes the fraction, not as a float stores it.
    assert_eq!(
        ScrollAt::message(&Data::from("source"), 0.0701),
        Data::Map(
            [
                ("msg".to_owned(), Data::from("source")),
                ("fraction".to_owned(), Data::Number(0.0701)),
            ]
            .into()
        )
    );
    // Neither is asked for unless set.
    let Ok(Node::Input(Input {
        scrolled: None,
        scroll_at: None,
        ..
    })) = view("ui.input { multiline = true }")
    else {
        panic!("a field scrolls on its own");
    };
    let Ok(Node::Markdown(Markdown {
        scrolled: None,
        scroll_at: None,
        ..
    })) = view("ui.markdown { 'a', scroll = true }")
    else {
        panic!("a document scrolls on its own");
    };
}

#[test]
fn a_fraction_runs_from_the_top_to_the_end() {
    assert_eq!(ScrollAt::fraction(0.0, 300.0), 0.0);
    assert_eq!(ScrollAt::fraction(150.0, 300.0), 0.5);
    assert_eq!(ScrollAt::fraction(300.0, 300.0), 1.0);
    // Past either end, as a bounce or the room under a code editor's last
    // line takes it, it stays at that end.
    assert_eq!(ScrollAt::fraction(-12.0, 300.0), 0.0);
    assert_eq!(ScrollAt::fraction(420.0, 300.0), 1.0);
    // Text that fits has nowhere to scroll.
    assert_eq!(ScrollAt::fraction(0.0, 0.0), 0.0);
    assert_eq!(ScrollAt::fraction(f32::NAN, 300.0), 0.0);
    // Rounded, so a pixel's jitter in a long text is no move.
    assert_eq!(
        ScrollAt::fraction(1000.0, 3000.0),
        ScrollAt::fraction(1000.01, 3000.0)
    );
}

#[test]
fn scrolling_goes_with_an_element_that_scrolls() {
    for (element, expected) in [
        (
            "ui.input { scrolled = 'moved' }",
            "`scrolled` goes with `multiline = true`",
        ),
        (
            "ui.input { scroll_at = { fraction = 0.5 } }",
            "`scroll_at` goes with `multiline = true`",
        ),
        (
            "ui.markdown { 'a', scrolled = 'moved' }",
            "`scrolled` goes with `scroll = true`",
        ),
        (
            "ui.markdown { 'a', scroll_at = { fraction = 0.5 } }",
            "`scroll_at` goes with `scroll = true`",
        ),
        (
            "ui.markdown { 'a', scroll = true, scroll_at = {} }",
            "needs `fraction`",
        ),
        (
            "ui.markdown { 'a', scroll = true, scroll_at = { fraction = 1.5 } }",
            "scroll_at.fraction",
        ),
        (
            "ui.input { multiline = true, scroll_at = { fraction = 0.5, at = 3 } }",
            "unknown key `at`",
        ),
    ] {
        let Err(error) = view(element) else {
            panic!("{element} should fail");
        };
        assert!(error.contains(expected), "{element}: {error}");
    }
    // `to` goes with `scrolled` as with any message.
    assert!(view("ui.input { multiline = true, scrolled = 'moved', to = 'other' }").is_ok());
    assert!(view("ui.markdown { 'a', scroll = true, scrolled = 'moved', to = 'other' }").is_ok());
}
