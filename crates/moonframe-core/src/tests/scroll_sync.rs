//! `scrolled` and `scroll_at`: a source field and its preview tell and take
//! how far down their text they are scrolled.

use {
    super::*,
    crate::{Input, Markdown, ScrollAt, ScrollFollow},
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

#[test]
fn a_seq_past_what_a_float_holds_whole_still_tells_requests_apart() {
    let at = |seq: i64| {
        let Ok(Node::Input(input)) = view(&format!(
            "ui.input {{ multiline = true, scroll_at = {{ fraction = 0.5, seq = {seq} }} }}"
        )) else {
            panic!("no input");
        };
        input.scroll_at.unwrap().seq
    };
    let big = 1 << 24;
    assert_ne!(at(big), at(big + 1));
}

/// A field 1000 points from its last line, for [`ScrollFollow`].
const MAX: f32 = 1000.0;

fn asked(fraction: f32, seq: i64) -> Option<ScrollAt> {
    Some(ScrollAt { fraction, seq })
}

#[test]
fn a_request_is_not_told_back_however_long_it_takes_to_land() {
    let mut follow = ScrollFollow::default();
    assert_eq!(follow.moved(0.0, MAX, true), None, "first seen");
    follow.take(asked(0.5, 1));
    assert_eq!(follow.scroll(0.0, MAX), Some(500.0));
    // Many looks before the engine lays it out: still nothing to tell.
    for _ in 0..50 {
        assert_eq!(follow.moved(0.0, MAX, true), None);
    }
    assert_eq!(follow.moved(500.0, MAX, true), None, "landed");
    // The reader moves on from there, and that is told.
    assert_eq!(follow.moved(600.0, MAX, true), Some(0.6));
    // The same request again is not new; a new `seq` is.
    follow.take(asked(0.5, 1));
    assert_eq!(follow.scroll(600.0, MAX), None);
    follow.take(asked(0.5, 2));
    assert_eq!(follow.scroll(600.0, MAX), Some(500.0));
}

#[test]
fn a_readers_move_while_a_request_waits_is_told() {
    let mut follow = ScrollFollow::default();
    follow.moved(0.0, MAX, true);
    follow.take(asked(0.5, 1));
    assert_eq!(follow.scroll(0.0, MAX), Some(500.0));
    // The reader scrolls before it lands.
    assert_eq!(follow.moved(100.0, MAX, true), Some(0.1));
    assert_eq!(follow.moved(100.0, MAX, true), None, "and only once");
    // Or after it landed somewhere else entirely.
    follow.take(asked(0.8, 2));
    assert_eq!(follow.scroll(100.0, MAX), Some(800.0));
    assert_eq!(follow.moved(300.0, MAX, true), Some(0.3));
}

#[test]
fn a_move_that_is_not_the_readers_is_only_noted() {
    let mut follow = ScrollFollow::default();
    follow.moved(0.0, MAX, true);
    assert_eq!(follow.moved(40.0, MAX, false), None, "the text grew");
    assert_eq!(
        follow.moved(40.0, MAX, true),
        None,
        "and that is where it is"
    );
    // Forgotten, it starts again from where it is next seen.
    follow.forget();
    assert_eq!(follow.moved(900.0, MAX, true), None);
    assert_eq!(follow.moved(1000.0, MAX, true), Some(1.0));
}

#[test]
fn a_request_for_where_the_text_is_already_needs_no_scroll() {
    let mut follow = ScrollFollow::default();
    follow.take(asked(0.25, 1));
    assert!(follow.pending());
    assert_eq!(follow.scroll(250.0, MAX), None);
    assert!(!follow.pending());
    assert_eq!(follow.moved(250.0, MAX, true), None);
    // Text with nowhere to scroll stays at the top.
    follow.take(asked(1.0, 2));
    assert_eq!(follow.scroll(0.0, 0.0), None);
}
