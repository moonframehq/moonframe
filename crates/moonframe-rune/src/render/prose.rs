//! Writing fields: a multiline `ui.input` with `prose` or `dim`. Markdown is
//! styled as it is typed and what lies outside the sentence or paragraph being
//! written fades, from the marks GPUI's writing field paints, shared.

#[allow(dead_code, reason = "shared with GPUI, which reads all of it")]
#[path = "../../../moonframe/src/input/prose/marks.rs"]
mod marks;

use {
    marks::{Look, Mark},
    moonframe_core::{Dim, Input},
    rune::{Color, Span},
};

/// How much markdown's markup (`#`, `**`, `[`…`](…)`) fades, as on GPUI.
const MARKUP_FADE: f32 = 0.55;
/// How much text outside the focused sentence or paragraph fades.
const DIM_FADE: f32 = 0.7;

fn span(range: std::ops::Range<usize>, look: Look, ink: Color) -> Span {
    let fade = match (look.has(Mark::Markup), look.dimmed) {
        (_, true) => DIM_FADE,
        (true, false) => MARKUP_FADE,
        (false, false) => 0.,
    };
    let mut span = Span::new(range.start, range.end);
    if fade > 0. {
        span = span.color(ink.alpha(1. - fade));
    }
    if look.has(Mark::Heading) || look.has(Mark::Strong) {
        span = span.bold();
    }
    if look.has(Mark::Emphasis) || look.has(Mark::Quote) {
        span = span.italic();
    }
    if look.has(Mark::Link) {
        span = span.underline(ink);
    }
    span
}

/// What a writing field paints over `text`, its caret at `caret`, in `ink`.
pub(super) fn spans(input: &Input, text: &str, caret: usize, ink: Color) -> Vec<Span> {
    painted(input.prose, input.dim, text, caret, ink)
}

/// `spans` for markdown (`prose`) and focus (`dim`) as asked.
fn painted(prose: bool, dim: Option<Dim>, text: &str, caret: usize, ink: Color) -> Vec<Span> {
    if !prose && dim.is_none() || text.is_empty() {
        return Vec::new();
    }
    let whole = 0..text.len();
    let mut found = Vec::new();
    if prose {
        let fences = marks::fences(text);
        marks::markdown(text, whole.clone(), &fences, &mut found);
    }
    let focus = dim.map(|dim| marks::focus(text, caret, dim));
    marks::paint(whole, &found, focus)
        .into_iter()
        .filter(|(_, look)| *look != Look::default())
        .map(|(range, look)| span(range, look, ink))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_field_paints_nothing() {
        assert!(painted(false, None, "# Title", 0, Color::BLACK).is_empty());
    }

    #[test]
    fn prose_makes_a_heading_bold_and_fades_its_hash() {
        let found = painted(true, None, "# Title", 0, Color::BLACK);
        assert!(found.iter().any(|span| span.weight.is_some()), "bold");
        let hash = found.iter().find(|span| span.start == 0).unwrap();
        assert!(hash.color.is_some(), "the markup fades");
    }

    #[test]
    fn focus_fades_the_sentence_not_being_written() {
        let text = "One sentence. Another one.";
        let found = painted(false, Some(Dim::Sentence), text, 2, Color::BLACK);
        let faded: Vec<u32> = found
            .iter()
            .filter(|span| span.color.is_some())
            .map(|span| span.start)
            .collect();
        assert!(faded.iter().all(|start| *start >= 13), "{faded:?}");
        assert!(!faded.is_empty(), "the other sentence fades");
    }
}
