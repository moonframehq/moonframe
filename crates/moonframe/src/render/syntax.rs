//! `syntax` on a `ui.text`: one line of code colored by its language. Lines
//! are highlighted alone, so `code-view` can build only the rows that show;
//! what spans lines, as a block comment does, colors only where it shows it.

use {
    gpui_kit::{
        base::input::Rope,
        component::{Theme, highlighter::SyntaxHighlighter},
        *,
    },
    std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc},
};

/// Most lines kept, so a long session over many files stays small.
const MOST: usize = 20_000;

type Spans = Rc<Vec<(Range<usize>, HighlightStyle)>>;

thread_local! {
    static PARSERS: RefCell<HashMap<String, SyntaxHighlighter>> = RefCell::new(HashMap::new());
    /// By language, darkness of the theme and line: a row built again as a
    /// list scrolls costs a look-up.
    static LINES: RefCell<HashMap<(String, bool, String), Spans>> = RefCell::new(HashMap::new());
}

/// `text` colored as `language`, or `None` to draw it plain.
pub(super) fn styled(language: Option<&str>, text: &str, theme: &Theme) -> Option<StyledText> {
    let language = language?;
    let spans = spans(language, text, theme);
    if spans.is_empty() {
        return None;
    }
    Some(StyledText::new(text.to_owned()).with_highlights(spans.iter().cloned()))
}

fn spans(language: &str, text: &str, theme: &Theme) -> Spans {
    let key = (language.to_owned(), theme.is_dark(), text.to_owned());
    if let Some(spans) = LINES.with_borrow(|lines| lines.get(&key).cloned()) {
        return spans;
    }
    let spans = PARSERS.with_borrow_mut(|parsers| {
        let parser = parsers
            .entry(language.to_owned())
            .or_insert_with(|| SyntaxHighlighter::new(language));
        parser.update(None, &Rope::from_str(text), None);
        parser.styles(&(0..text.len()), theme.highlight_theme.as_ref())
    });
    let spans = Rc::new(spans);
    LINES.with_borrow_mut(|lines| {
        if lines.len() >= MOST {
            lines.clear();
        }
        lines.insert(key, spans.clone());
    });
    spans
}
