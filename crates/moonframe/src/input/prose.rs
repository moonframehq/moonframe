//! Writing fields: a multiline `ui.input` with `prose`, `dim` or
//! `typewriter`. The engine is gpui-base's code editor with its gutter,
//! folding and pairing turned off, and a highlighter of our own that styles
//! markdown as it is typed and fades what lies outside the sentence or
//! paragraph being written. The marks themselves come from [`marks`], plain
//! text in and ranges out, so they are tested without a window.

mod marks;

use {
    gpui_kit::{
        base::input::{
            EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter, Rope,
        },
        *,
    },
    marks::{Look, Mark},
    moonframe_core::{Dim, Input},
    std::{cell::Cell, ops::Range, rc::Rc},
};

/// How much markdown's markup (`#`, `**`, `[`…`](…)`) fades.
const MARKUP_FADE: f32 = 0.55;
/// How much text outside the focused sentence or paragraph fades.
const DIM_FADE: f32 = 0.7;
/// Lines kept around the caret in typewriter mode; the engine clamps it to
/// half the field, which keeps the caret's line in the middle.
const TYPEWRITER_LINES: usize = 10_000;
/// Rows of empty room under the last line outside typewriter mode, where
/// the engine would otherwise leave half the field.
const BOTTOM_ROWS: usize = 1;

/// What the field's highlighter reads as it paints: the mod's settings and
/// where the caret is, kept current as the caret moves.
#[derive(Default)]
pub(crate) struct Writing {
    markdown: Cell<bool>,
    dim: Cell<Option<Dim>>,
    typewriter: Cell<Option<bool>>,
    cursor: Cell<usize>,
}

impl Writing {
    /// Takes the mod's latest settings for the field.
    pub(super) fn apply(
        &self,
        input: &Input,
        state: &Entity<EditorState>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let changed = self.markdown.replace(input.prose) != input.prose
            || self.dim.replace(input.dim) != input.dim;
        if self.typewriter.replace(Some(input.typewriter)) != Some(input.typewriter) {
            let (around, below) = if input.typewriter {
                (Some(TYPEWRITER_LINES), None)
            } else {
                (None, Some(BOTTOM_ROWS))
            };
            state.update(cx, |state, cx| {
                state.set_cursor_surrounding_lines(around, window, cx);
                state.set_scroll_beyond_last_line(below, window, cx);
            });
        } else if changed {
            state.update(cx, |_, cx| cx.notify());
        }
    }
}

/// A writing field's engine, with the highlighter that reads `writing`.
pub(super) fn editor(
    writing: &Rc<Writing>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<EditorState> {
    let shared = writing.clone();
    cx.new(|cx| {
        let mut state = EditorState::new(window, cx)
            .line_number(false)
            .folding(false)
            .indent_guides(false)
            .auto_close(false)
            .smart_indent(false)
            .soft_wrap(true);
        state.set_highlighter_factory(
            Rc::new(move |_: &str| {
                let highlighter: Box<dyn InputHighlighter> = Box::new(Highlighter {
                    writing: shared.clone(),
                    text: String::new(),
                    fences: Vec::new(),
                });
                Some(highlighter)
            }),
            cx,
        );
        state
    })
}

/// Keeps `writing`'s caret where the editor's is, as it moves.
pub(super) fn follow(
    writing: &Rc<Writing>,
    state: &Entity<EditorState>,
    cx: &mut App,
) -> Subscription {
    let writing = writing.clone();
    cx.observe(state, move |state, cx| {
        writing.cursor.set(state.read(cx).cursor());
    })
}

/// Styles the field's text from the copy it keeps of it.
struct Highlighter {
    writing: Rc<Writing>,
    text: String,
    /// Fenced code blocks, which markdown inside them doesn't style.
    fences: Vec<Range<usize>>,
}

impl InputHighlighter for Highlighter {
    fn language(&self) -> SharedString {
        "prose".into()
    }

    fn update(
        &mut self,
        _: Option<InputEdit>,
        text: &Rope,
        _: bool,
        _: &mut Window,
        _: &mut Context<EditorState>,
    ) {
        self.text = text.to_string();
        self.fences = marks::fences(&self.text);
    }

    fn styles(
        &self,
        range: &Range<usize>,
        _: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let text = &self.text;
        let range = range.start.min(text.len())..range.end.min(text.len());
        let mut found = Vec::new();
        if self.writing.markdown.get() {
            marks::markdown(text, range.clone(), &self.fences, &mut found);
        }
        let focus = self
            .writing
            .dim
            .get()
            .map(|dim| marks::focus(text, self.writing.cursor.get().min(text.len()), dim));
        let mut runs: Vec<_> = marks::paint(range.clone(), &found, focus)
            .into_iter()
            .map(|(range, look)| (range, style(look)))
            .collect();
        // The engine asks for runs that cover the whole range, the
        // newline its lines end with included.
        if runs.is_empty() {
            runs.push((range, HighlightStyle::default()));
        }
        runs
    }

    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}

/// The engine's style for a run's look.
fn style(look: Look) -> HighlightStyle {
    let mut fade = 0.;
    if look.has(Mark::Markup) {
        fade = MARKUP_FADE;
    }
    if look.dimmed {
        fade = f32::max(fade, DIM_FADE);
    }
    HighlightStyle {
        font_weight: (look.has(Mark::Heading) || look.has(Mark::Strong))
            .then_some(FontWeight::BOLD),
        font_style: (look.has(Mark::Emphasis) || look.has(Mark::Quote))
            .then_some(FontStyle::Italic),
        underline: look.has(Mark::Link).then(|| UnderlineStyle {
            thickness: px(1.),
            color: None,
            wavy: false,
        }),
        fade_out: (fade > 0.).then_some(fade),
        ..HighlightStyle::default()
    }
}
