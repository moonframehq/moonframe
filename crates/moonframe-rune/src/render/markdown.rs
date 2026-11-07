//! `ui.markdown`: Rune's document on the app's type scale, with GitHub's
//! alerts, a `lines` limit, and search.

use {
    super::{Painter, Place},
    crate::{Ui, style::Ink},
    moonframe_core::{Data, Markdown, Part, States, Token},
    rune::{El, ScrollSpec, col},
    rune_kit::{Case, Highlight, Markdown as RuneMarkdown, MarkdownStyle, markdown::find},
};

/// Heading sizes, levels 1 to 3, in sizes of the body.
const HEADINGS: [f32; 3] = [1.75, 1.25, 1.1];
/// A task's checkbox.
const TASK_BOX: f32 = 14.;
/// How line height relates to size, as `Ink` sets text.
const LINE: f32 = 1.618;
/// GitHub's alerts, as the marker in a quote and the title it shows.
const ALERTS: [(&str, &str); 5] = [
    ("[!NOTE]", "Note"),
    ("[!TIP]", "Tip"),
    ("[!IMPORTANT]", "Important"),
    ("[!WARNING]", "Warning"),
    ("[!CAUTION]", "Caution"),
];

/// `text` with each alert's marker as its bold title, on a line of its own.
fn titled_alerts(text: &str) -> String {
    let mut out = text.to_owned();
    for (marker, title) in ALERTS {
        out = out.replace(&format!("> {marker}\n"), &format!("> **{title}**\n>\n"));
    }
    out
}

impl<'w> Painter<'w> {
    fn markdown_style(&self, ink: Ink) -> MarkdownStyle {
        let token = |token| self.palette.token(token);
        let style = MarkdownStyle::neutral(self.palette.dark)
            .body_font(ink.text_style())
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
        let source = titled_alerts(&markdown.text);
        let query = markdown.find.as_deref().filter(|query| !query.is_empty());
        let found = query.map_or_else(Vec::new, |query| find(&source, query, Case::Insensitive));
        self.note_found(markdown, found.len(), place);
        let marks = found.into_iter().enumerate().map(|(index, range)| {
            let current = markdown.find_index == Some(index + 1);
            (range, Highlight::new().current_match(current))
        });
        let mut document = RuneMarkdown::new(source).selectable(true).highlights(marks);
        if let Some(msg) = markdown.link.clone() {
            let owner = place.owner.to_owned();
            let to = markdown.to.clone().unwrap_or_else(|| owner.clone());
            document = document.on_link(move |ui: &mut Ui, url: &str| {
                ui.send(&owner, &to, Markdown::message(&msg, url));
            });
        }
        let style = self
            .markdown_style(ink)
            .highlight_bg(self.palette.token(Token::Warning).alpha(0.35));
        let element = col().key(place.path.clone()).child(document.render(&style));
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
mod tests {
    use super::titled_alerts;

    #[test]
    fn an_alert_shows_its_title_on_a_line_of_its_own() {
        assert_eq!(
            titled_alerts("> [!TIP]\n> Mind it.\n"),
            "> **Tip**\n>\n> Mind it.\n"
        );
    }

    #[test]
    fn a_quote_that_is_no_alert_stays_as_it_is() {
        let quote = "> [!FOO]\n> Mind it.\n";
        assert_eq!(titled_alerts(quote), quote);
    }
}
