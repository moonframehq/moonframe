//! `ui.markdown`: Rune's document on the app's type scale, with GitHub's
//! alerts, a `lines` limit, and search.

#[allow(dead_code, reason = "shared with GPUI, which reads all of it")]
#[path = "../../../moonframe/src/render/markdown/outline.rs"]
mod outline;
mod scrolling;

use {
    super::{Painter, Place},
    crate::{Ui, icons, style::Ink},
    moonframe_core::{Data, IconName, Markdown, Part, Reveal, States, Token},
    outline::Outline,
    rune::{El, ScrollSpec, col, el, row, text},
    rune_kit::{
        Case, CodeFrame, Highlight, Markdown as RuneMarkdown, MarkdownStyle, markdown::find,
    },
    std::rc::Rc,
};

/// Heading sizes, levels 1 to 3, in sizes of the body.
const HEADINGS: [f32; 3] = [1.75, 1.25, 1.1];
/// A task's checkbox.
const TASK_BOX: f32 = 14.;
/// How line height relates to size, as `Ink` sets text.
const LINE: f32 = 1.618;
/// GitHub's alerts: the marker in a quote, the title it shows, its color and
/// its icon, as GPUI draws them.
const ALERTS: [(&str, &str, Token, &str); 5] = [
    ("[!NOTE]", "Note", Token::Accent, "info"),
    ("[!TIP]", "Tip", Token::Success, "sparkles"),
    ("[!IMPORTANT]", "Important", Token::Accent, "message-square"),
    ("[!WARNING]", "Warning", Token::Warning, "triangle-alert"),
    ("[!CAUTION]", "Caution", Token::Danger, "octagon-alert"),
];
/// The language an alert's fence goes under, followed by its kind's index.
const ALERT_FENCE: &str = "moonframe-alert-";
/// An alert's bar, along its left side.
const ALERT_BAR: f32 = 3.;

/// What a document's mod asked to hear of its structure, as last drawn.
#[derive(Default)]
pub(crate) struct Document {
    /// The text the outline is of.
    text: String,
    outline: Rc<Outline>,
    /// The headings the mod was last told of.
    told: Option<Vec<(u8, String, usize)>>,
    section: Option<usize>,
    reveal: Option<Reveal>,
    /// A block to scroll to the top of the document once it is laid out.
    pub(crate) pending: Option<usize>,
    scrolling: scrolling::Scrolling,
}

/// The key of a document's top-level block `index`.
fn block_key(path: &str, index: usize) -> String {
    format!("{path}-b{index}")
}

/// `text` with each footnote mark, `[^id]`, as a link to its note, and each
/// note's `[^id]:` as its label; with where each note is in what it returns.
/// Fenced code is left as it is.
fn linked_footnotes(text: &str) -> (String, Vec<(String, usize)>) {
    let mut out = String::with_capacity(text.len());
    let mut notes = Vec::new();
    let mut fenced = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
        }
        if fenced || !line.contains("[^") {
            out.push_str(line);
            continue;
        }
        let mut rest = line;
        let mut first = true;
        while let Some(at) = rest.find("[^") {
            let Some(end) = rest[at..].find(']').map(|end| at + end) else {
                break;
            };
            let id = &rest[at + 2..end];
            out.push_str(&rest[..at]);
            let defines = first && at == 0 && rest[end + 1..].starts_with(':');
            if defines {
                notes.push((id.to_owned(), out.len()));
                out.push_str(&format!("**[{id}]**"));
                rest = &rest[end + 2..];
            } else {
                out.push_str(&format!("[[{id}]](#fn-{id})"));
                rest = &rest[end + 1..];
            }
            first = false;
        }
        out.push_str(rest);
    }
    (out, notes)
}

/// `text` with each alert, a quote that starts with a marker, as a fence of
/// its own, which the document's code frame draws as GPUI draws an alert.
fn fenced_alerts(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut lines = text.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        let kind = ALERTS
            .iter()
            .position(|(marker, ..)| line.trim_end() == format!("> {marker}"));
        let Some(kind) = kind else {
            out.push_str(line);
            continue;
        };
        out.push_str(&format!("~~~~{ALERT_FENCE}{kind}\n"));
        while let Some(quoted) = lines.next_if(|next| next.starts_with('>')) {
            let body = quoted.strip_prefix("> ").unwrap_or(&quoted[1..]);
            out.push_str(body);
            if !body.ends_with('\n') {
                out.push('\n');
            }
        }
        out.push_str("~~~~\n");
    }
    out
}

impl<'w> Painter<'w> {
    fn markdown_style(&self, ink: Ink) -> MarkdownStyle {
        let token = |token| self.palette.token(token);
        // Links in the text's own ink, underlined, as on GPUI.
        let style = MarkdownStyle::neutral(self.palette.dark)
            .body_font(ink.text_style())
            .link(ink.color)
            .link_underline(ink.color)
            .task_checkbox(
                TASK_BOX,
                3.,
                8.,
                None,
                token(Token::Foreground),
                token(Token::Background),
                token(Token::Background),
                token(Token::MutedForeground),
            );
        HEADINGS
            .iter()
            .zip(1..)
            .fold(style, |style, (scale, level)| {
                let heading = Ink {
                    size: ink.size * scale,
                    weight: 600.,
                    ..ink
                };
                style.heading_font(level, heading.text_style())
            })
    }

    /// A document's outline, parsed once for each text it shows.
    fn outline(&self, path: &str, source: &str) -> Rc<Outline> {
        let mut documents = self.ui.documents.borrow_mut();
        let document = documents.entry(path.to_owned()).or_default();
        if document.text != source {
            document.text = source.to_owned();
            document.outline = Rc::new(Outline::parse(source));
        }
        Rc::clone(&document.outline)
    }

    /// Queues `msg` for the document's mod: a view cannot send.
    fn tell(&self, markdown: &Markdown, msg: Data, place: &Place<'w>) {
        let owner = place.owner.to_owned();
        let to = markdown.to.clone().unwrap_or_else(|| owner.clone());
        self.ui.markdown_notes.borrow_mut().push((owner, to, msg));
    }

    /// The heading a reader of the document at `path` has scrolled to: the
    /// one over the first block still showing at its top.
    fn section_at(path: &str, outline: &Outline) -> Option<usize> {
        let top = rune::memo::window_layout(path)?[1];
        let showing = (0..outline.blocks().len()).find(|index| {
            rune::memo::window_layout(&block_key(path, *index))
                .is_some_and(|[_, y, _, height]| y + height > top + 1.)
        })?;
        outline.section(showing)
    }

    /// `outline`, `section` and `reveal`: its headings when they change, the
    /// one a reader has scrolled to, and a scroll to a place asked for.
    fn follow(&self, markdown: &Markdown, outline: &Outline, place: &Place<'w>) {
        let path = place.path.as_str();
        let headings: Vec<(u8, String, usize)> = outline
            .headings
            .iter()
            .map(|heading| (heading.level, heading.text.clone(), heading.at))
            .collect();
        let section = markdown
            .section
            .as_ref()
            .and_then(|_| Self::section_at(path, outline));
        let (told, moved, reveal) = {
            let mut documents = self.ui.documents.borrow_mut();
            let Some(document) = documents.get_mut(path) else {
                return;
            };
            let told = markdown.outline.is_some() && document.told.as_ref() != Some(&headings);
            if told {
                document.told = Some(headings);
            }
            let moved =
                markdown.section.is_some() && section.is_some() && section != document.section;
            if moved {
                document.section = section;
            }
            let reveal = (markdown.reveal != document.reveal)
                .then_some(markdown.reveal)
                .flatten();
            document.reveal = markdown.reveal;
            (told, moved, reveal)
        };
        if let (true, Some(msg)) = (told, &markdown.outline) {
            let list = outline
                .headings
                .iter()
                .map(|heading| heading.data())
                .collect();
            self.tell(
                markdown,
                Markdown::note(msg, [("headings", Data::List(list))]),
                place,
            );
        }
        if let (true, Some(msg), Some(index)) = (moved, &markdown.section, section) {
            let heading = Data::Integer(i64::try_from(index + 1).unwrap_or(i64::MAX));
            self.tell(markdown, Markdown::note(msg, [("heading", heading)]), place);
        }
        let mut documents = self.ui.documents.borrow_mut();
        let Some(document) = documents.get_mut(path) else {
            return;
        };
        if let Some(reveal) = reveal {
            document.pending = Some(outline.block_at(reveal.at));
        }
        // Its block at the top, as on GPUI: where it is in the text, from
        // the last layout; until there is one, it waits.
        if let Some(block) = document.pending {
            let top = rune::memo::window_layout(&format!("{path}-text"));
            let at = rune::memo::window_layout(&block_key(path, block));
            if let (Some([_, top, ..]), Some([_, at, ..])) = (top, at) {
                document.pending = None;
                let mut reveals = self.ui.markdown_reveals.borrow_mut();
                reveals.push((path.to_owned(), at - top, false));
            }
        }
        if !markdown.scroll {
            return;
        }
        let looked = document.scrolling.look(
            markdown,
            rune::memo::window_layout(path),
            rune::memo::window_layout(&format!("{path}-text")),
        );
        drop(documents);
        if let Some(y) = looked.scroll {
            let mut reveals = self.ui.markdown_reveals.borrow_mut();
            reveals.push((path.to_owned(), y, true));
        }
        if looked.again {
            self.ui.markdown_again.set(true);
        }
        if let Some(msg) = looked.tell {
            self.tell(markdown, msg, place);
        }
    }

    /// What draws an alert's fence: its color down the side, its icon and
    /// title, and its text as markdown. Other fences keep their card.
    fn alert_frame(
        &self,
        style: &MarkdownStyle,
        ink: Ink,
    ) -> impl Fn(El, &CodeFrame<'_>) -> El + use<> {
        let kinds: Vec<(&'static str, rune::Color, Option<&'static rune::Icon>)> = ALERTS
            .iter()
            .map(|(_, title, token, icon)| {
                let icon = IconName::new(icon).as_ref().and_then(icons::icon);
                (*title, self.palette.token(*token), icon)
            })
            .collect();
        let style = style.clone();
        let title_ink = Ink {
            weight: 600.,
            ..ink
        };
        let muted = self.palette.token(Token::Muted);
        move |card: El, frame: &CodeFrame<'_>| {
            let kind = frame
                .language
                .and_then(|language| language.strip_prefix(ALERT_FENCE))
                .and_then(|index| index.parse::<usize>().ok())
                .and_then(|index| kinds.get(index));
            let Some((title, color, icon)) = kind else {
                return card;
            };
            let heading = row()
                .gap(8.)
                .items_center()
                .children(icon.map(|icon| rune::icon(icon, title_ink.size).color(*color)))
                .child(text(*title).font(title_ink.text_style()).color(*color));
            col()
                .w_full()
                .gap(4.)
                .py(8.)
                .pl(12. + ALERT_BAR)
                .pr(12.)
                .radius(6.)
                .fill(muted)
                .child(
                    el().pin(Some(0.), Some(0.), None, Some(0.))
                        .w(ALERT_BAR)
                        .fill(*color),
                )
                .child(heading)
                .child(RuneMarkdown::new(frame.source.to_owned()).render(&style))
        }
    }

    /// Tells the mod how many matches there are, when that changed.
    fn note_found(&self, markdown: &Markdown, count: usize, place: &Place<'w>) {
        let Some(msg) = &markdown.found else {
            return;
        };
        let key = format!("{}:{}", place.owner, place.path);
        let said = self.ui.markdown_found.borrow_mut().insert(key, count);
        if said != Some(count) {
            let owner = place.owner.to_owned();
            let to = markdown.to.clone().unwrap_or_else(|| owner.clone());
            let count = Data::Integer(i64::try_from(count).unwrap_or(i64::MAX));
            let note = Markdown::note(msg, [("count", count)]);
            self.ui.markdown_notes.borrow_mut().push((owner, to, note));
        }
    }

    pub(super) fn markdown(&self, markdown: &'w Markdown, place: &Place<'w>) -> El {
        let look = self.look(Part::Markdown, &markdown.look, States::default(), place);
        let ink = place.ink.under(&look.base, self.palette);
        let (source, notes) = linked_footnotes(&fenced_alerts(&markdown.text));
        let query = markdown.find.as_deref().filter(|query| !query.is_empty());
        let found = query.map_or_else(Vec::new, |query| find(&source, query, Case::Insensitive));
        self.note_found(markdown, found.len(), place);
        let marks = found.into_iter().enumerate().map(|(index, range)| {
            let current = markdown.find_index == Some(index + 1);
            (range, Highlight::new().current_match(current))
        });
        let outline = self.outline(&place.path, &source);
        self.follow(markdown, &outline, place);
        let path = place.path.clone();
        let keys: Vec<(usize, String)> = outline
            .blocks()
            .iter()
            .enumerate()
            .map(|(index, at)| (*at, block_key(&path, index)))
            .collect();
        // A `#heading` link scrolls there; the rest goes to the mod.
        let link = markdown.link.clone();
        let owner = place.owner.to_owned();
        let to = markdown.to.clone().unwrap_or_else(|| owner.clone());
        let document = RuneMarkdown::new(source)
            .selectable(true)
            .highlights(marks)
            .block_keys(keys)
            .on_link(move |ui: &mut Ui, url: &str| {
                // A note's mark scrolls to the note; a `#heading`, to it.
                let fragment = url.strip_prefix('#');
                let note = fragment
                    .and_then(|fragment| fragment.strip_prefix("fn-"))
                    .and_then(|id| notes.iter().find(|(note, _)| note == id))
                    .map(|(_, at)| *at);
                let heading = fragment
                    .and_then(|fragment| outline.anchor(fragment))
                    .map(|heading| heading.at);
                match (note.or(heading), &link) {
                    (Some(at), _) => {
                        if let Some(document) = ui.documents.get_mut().get_mut(&path) {
                            document.pending = Some(outline.block_at(at));
                        }
                    },
                    (None, Some(msg)) => ui.send(&owner, &to, Markdown::message(msg, url)),
                    (None, None) => {},
                }
            });
        let style = self
            .markdown_style(ink)
            .highlight_bg(self.palette.token(Token::Warning).alpha(0.35));
        let document = document.code_frame(self.alert_frame(&style, ink));
        // Each move of the text inside a scroll runs the view again, so the
        // section a reader is in follows them.
        let rendered = document
            .render(&style)
            .key(format!("{}-text", place.path))
            .on_layout(|_: &mut Ui, _: [f32; 4]| {});
        let element = col().key(place.path.clone()).child(rendered);
        let element = match (markdown.width, markdown.height) {
            (Some(width), Some(height)) => element.size(width, height),
            (Some(width), None) => element.w(width),
            (None, Some(height)) => element.h(height),
            (None, None) => element,
        };
        let element = match markdown.max_width {
            Some(width) => element.max_w(width),
            None => element,
        };
        // Cut at a line, as a preview in a tile is.
        let element = match markdown.lines {
            Some(lines) => element
                .max_h(lines as f32 * ink.size * LINE)
                .clip_rounded(0.),
            None => element,
        };
        let element = if markdown.grow {
            element.grow(1.).min_h(0.).min_w(0.)
        } else {
            element
        };
        let element = if markdown.scroll {
            element.min_h(0.).scroll(ScrollSpec {
                stick_bottom: false,
            })
        } else {
            element
        };
        crate::style::looked(element, &look, None, self.palette)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{fenced_alerts, linked_footnotes};

    #[test]
    fn a_footnote_mark_links_to_its_note() {
        let (text, notes) = linked_footnotes("Mixed.[^why]\n\n[^why]: Because.\n");
        assert_eq!(text, "Mixed.[[why]](#fn-why)\n\n**[why]** Because.\n");
        assert_eq!(notes, vec![(
            "why".to_owned(),
            text.find("**[why]**").unwrap()
        )]);
    }

    #[test]
    fn a_footnote_in_fenced_code_stays_as_it_is() {
        let code = "```\nlet a = b[^c];\n```\n";
        assert_eq!(linked_footnotes(code).0, code);
    }

    #[test]
    fn an_alert_becomes_a_fence_of_its_own() {
        let text = "Before.\n\n> [!TIP]\n> Mind it.\n> Twice.\n\nAfter.\n";
        assert_eq!(
            fenced_alerts(text),
            "Before.\n\n~~~~moonframe-alert-1\nMind it.\nTwice.\n~~~~\n\nAfter.\n"
        );
    }

    #[test]
    fn a_quote_that_is_no_alert_stays_as_it_is() {
        let quote = "> [!FOO]\n> Mind it.\n";
        assert_eq!(fenced_alerts(quote), quote);
    }
}
