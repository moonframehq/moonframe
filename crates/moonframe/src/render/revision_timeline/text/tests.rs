use {super::*, core::prelude::v1::test};

/// Fits while at most `chars` characters, as a card that width would.
fn within(chars: usize) -> impl FnMut(&str) -> bool {
    move |text: &str| text.chars().count() <= chars
}

#[test]
fn text_that_fits_is_left_whole() {
    assert_eq!(clamp_words("Merged main.", within(40)), "Merged main.");
}

#[test]
fn a_long_summary_is_cut_between_words_with_an_ellipsis() {
    let text = "I'll start by checking which GitHub APIs exist for the remaining steps";
    let cut = clamp_words(text, within(30));
    assert_eq!(cut, "I'll start by checking which\u{2026}");
    assert!(cut.chars().count() <= 30);
    // The longest start that fits, not merely one that does: a character
    // less and the last word goes.
    assert_eq!(clamp_words(text, within(29)), cut);
    assert_eq!(
        clamp_words(text, within(28)),
        "I'll start by checking\u{2026}"
    );
}

#[test]
fn spaces_before_the_cut_are_dropped() {
    assert_eq!(clamp_words("one   two three", within(6)), "one\u{2026}");
}

#[test]
fn a_first_word_too_long_leaves_the_ellipsis() {
    assert_eq!(
        clamp_words("Supercalifragilistic word", within(5)),
        "\u{2026}"
    );
    // Nothing to cut: an empty summary stays empty.
    assert_eq!(clamp_words("", within(0)), "");
}

#[test]
fn checks_stay_few_for_a_long_summary() {
    let text = "word ".repeat(500);
    let mut checks = 0;
    let _ = clamp_words(&text, |candidate| {
        checks += 1;
        candidate.len() <= 100
    });
    assert!(checks <= 12, "{checks} checks");
}
