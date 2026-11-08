use {super::*, moonframe_core::Dim};

/// The text each mark covers in `text`, in order.
fn marked(text: &str) -> Vec<(&str, Mark)> {
    let mut marks = Vec::new();
    markdown(text, 0..text.len(), &fences(text), &mut marks);
    marks
        .into_iter()
        .map(|(range, mark)| (&text[range], mark))
        .collect()
}

#[test]
fn headings_are_bold_with_their_hashes_faded() {
    assert_eq!(
        marked("## Draft"),
        [("## Draft", Mark::Heading), ("##", Mark::Markup)]
    );
    // Seven hashes, or none followed by a space, are text.
    assert!(marked("####### no").is_empty());
    assert!(marked("#hashtag").is_empty());
}

#[test]
fn inline_spans_mark_their_text_and_fade_their_markup() {
    assert_eq!(
        marked("a **bold** b"),
        [
            ("**", Mark::Markup),
            ("bold", Mark::Strong),
            ("**", Mark::Markup)
        ]
    );
    assert_eq!(
        marked("an *aside* here"),
        [
            ("*", Mark::Markup),
            ("aside", Mark::Emphasis),
            ("*", Mark::Markup)
        ]
    );
    assert_eq!(
        marked("see [the site](https://ia.net)"),
        [
            ("[", Mark::Markup),
            ("the site", Mark::Link),
            ("](https://ia.net)", Mark::Markup)
        ]
    );
    assert_eq!(
        marked("run `ls`"),
        [("`", Mark::Markup), ("ls", Mark::Code), ("`", Mark::Markup)]
    );
    // Words with underscores and sums with stars are not emphasis.
    assert!(marked("snake_case_name and 2 * 3 * 4").is_empty());
}

#[test]
fn blocks_fade_their_leading_markup() {
    assert_eq!(
        marked("> said"),
        [(">", Mark::Markup), (" said", Mark::Quote)]
    );
    assert_eq!(marked("- [x] done"), [("- [x] ", Mark::Markup)]);
    assert_eq!(marked("12. twelfth"), [("12. ", Mark::Markup)]);
    assert_eq!(marked("* * *"), [("* * *", Mark::Markup)]);
}

#[test]
fn fenced_code_is_left_unstyled_inside() {
    let text = "```\n**not bold**\n```\nafter *this*";
    assert_eq!(fences(text).first(), Some(&(0..20)));
    assert_eq!(
        marked(text)[..3],
        [
            ("```", Mark::Markup),
            ("**not bold**", Mark::Code),
            ("```", Mark::Markup)
        ]
    );
    assert_eq!(marked(text)[4], ("this", Mark::Emphasis));
    // An unclosed fence runs to the end.
    assert_eq!(fences("a\n```\nb").first(), Some(&(2..7)));
}

#[test]
fn focus_is_the_caret_s_sentence_or_paragraph() {
    let text = "One. Two is \"here.\" Three?\n\nNext one.";
    let at = |needle: &str| text.find(needle).unwrap_or(text.len());
    let sentence = |cursor| &text[focus(text, cursor, Dim::Sentence)];
    assert_eq!(sentence(at("wo")), "Two is \"here.\"");
    assert_eq!(sentence(0), "One.");
    assert_eq!(sentence(at("hree")), "Three?");
    assert_eq!(sentence(at("ext")), "Next one.");
    let paragraph = &text[focus(text, at("wo"), Dim::Paragraph)];
    assert_eq!(paragraph, "One. Two is \"here.\" Three?");
    // A line of its own, such as a heading, is a sentence.
    let text = "# Title\nBody text";
    assert_eq!(&text[focus(text, 3, Dim::Sentence)], "# Title");
    // At the very end of what is being typed, the sentence stays lit.
    let text = "Done. Still typing";
    assert_eq!(
        &text[focus(text, text.len(), Dim::Sentence)],
        "Still typing"
    );
}

#[test]
fn paint_covers_the_range_and_dims_outside_the_focus() {
    let marks = [(2..4, Mark::Strong), (3..6, Mark::Strong)];
    let runs = paint(0..10, &marks, Some(5..8));
    let ranges: Vec<_> = runs.iter().map(|(range, _)| range.clone()).collect();
    assert_eq!(ranges, [0..2, 2..5, 5..6, 6..8, 8..10]);
    assert!(runs[0].1.dimmed && !runs[0].1.has(Mark::Strong));
    assert!(runs[1].1.dimmed && runs[1].1.has(Mark::Strong));
    assert!(!runs[2].1.dimmed && runs[2].1.has(Mark::Strong));
    assert!(!runs[3].1.dimmed && !runs[3].1.has(Mark::Strong));
    assert!(runs[4].1.dimmed);
    // Without marks or focus, one plain run.
    assert_eq!(paint(0..4, &[], None), [(0..4, Look::default())]);
}

#[test]
fn ranges_from_the_middle_of_the_text_mark_only_their_lines() {
    let text = "*one*\n**two**\nthree";
    let mut marks = Vec::new();
    let second = 6..14;
    markdown(text, second.clone(), &[], &mut marks);
    assert!(marks.iter().all(|(range, _)| range.start >= 6));
    assert!(marks.iter().any(|(_, mark)| *mark == Mark::Strong));
}
