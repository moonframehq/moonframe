//! The HTML Notes.app gives a note's body, as markdown.
//!
//! Notes writes simple, regular HTML: a `<div>` per line (`<div><br></div>`
//! for an empty one), `<h1>`–`<h3>`, `<b>` `<i>` `<u>` `<strike>`, `<tt>`
//! for monospace, lists (`<ul>`, `<ol>`, checklists), `<a>`, `<img>` with
//! data urls, and `<table>`. A small tolerant reader handles that and
//! anything near it; what it does not know it reads as text.

/// A picture found in the body, for the importer to save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Picture {
    /// What the markdown says where the picture goes, replaced once saved.
    pub placeholder: String,
    pub src: String,
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Open {
        name: String,
        attrs: Vec<(String, String)>,
    },
    Close(String),
    Text(String),
}

fn tokenize(html: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |end| &after[end + 3..]);
            continue;
        }
        if rest.starts_with('<')
            && let Some(end) = rest.find('>')
        {
            let inner = &rest[1..end];
            rest = &rest[end + 1..];
            if let Some(name) = inner.strip_prefix('/') {
                out.push(Token::Close(name.trim().to_lowercase()));
            } else if !inner.starts_with('!') && !inner.starts_with('?') {
                let inner = inner.trim_end_matches('/');
                let name_end = inner
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(inner.len());
                let name = inner[..name_end].to_lowercase();
                let attrs = attributes(&inner[name_end..]);
                // Text inside these is not the note's.
                if matches!(name.as_str(), "style" | "script" | "head" | "title") {
                    let close = format!("</{name}");
                    rest = rest.to_ascii_lowercase().find(&close).map_or("", |at| {
                        let after = &rest[at..];
                        after.find('>').map_or("", |gt| &after[gt + 1..])
                    });
                    continue;
                }
                out.push(Token::Open { name, attrs });
            }
            continue;
        }
        // Text up to the next tag; the first character may be several bytes.
        let first = rest.chars().next().map_or(1, char::len_utf8);
        let end = rest[first..].find('<').map_or(rest.len(), |at| at + first);
        out.push(Token::Text(decode(&rest[..end])));
        rest = &rest[end..];
    }
    out
}

fn attributes(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let name_end = rest
            .find(|c: char| c == '=' || c.is_whitespace())
            .unwrap_or(rest.len());
        let name = rest[..name_end].to_lowercase();
        rest = rest[name_end..].trim_start();
        let mut value = String::new();
        if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            if let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let body = &after[1..];
                let end = body.find(quote).unwrap_or(body.len());
                value = decode(&body[..end]);
                rest = body.get(end + 1..).unwrap_or("");
            } else {
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                value = decode(&after[..end]);
                rest = &after[end..];
            }
        }
        if !name.is_empty() {
            out.push((name, value));
        }
        rest = rest.trim_start();
    }
    out
}

/// HTML entities as characters.
pub(crate) fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        // An entity's `;` comes within a few characters of its `&`.
        let Some(end) = rest
            .char_indices()
            .take_while(|(at, _)| *at < 12)
            .find(|(_, c)| *c == ';')
            .map(|(at, _)| at)
        else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            },
            None => {
                out.push('&');
                rest = &rest[1..];
            },
        }
    }
    out.push_str(rest);
    out
}

/// Characters that would read as markdown at the start of a line or inline.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '*' | '_' | '`' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

struct List {
    ordered: bool,
    checklist: bool,
    number: usize,
}

#[derive(Default)]
struct Writer {
    out: String,
    /// The line being built.
    line: String,
    lists: Vec<List>,
    /// The markup the current list item's line starts with.
    item_prefix: Option<String>,
    heading: Option<usize>,
    quote: bool,
    pre: bool,
    link: Option<String>,
    link_text: String,
    table: Vec<Vec<String>>,
    cell: Option<String>,
    pictures: Vec<Picture>,
}

impl Writer {
    fn text(&mut self, text: &str) {
        if let Some(cell) = &mut self.cell {
            cell.push_str(&text.replace('|', "\\|").replace('\n', " "));
            return;
        }
        if self.link.is_some() {
            self.link_text.push_str(text);
            return;
        }
        if self.pre {
            self.line.push_str(text);
            return;
        }
        let text = text.replace(['\n', '\r'], " ");
        self.line.push_str(&escape(&text));
    }

    fn mark(&mut self, mark: &str) {
        if let Some(cell) = &mut self.cell {
            cell.push_str(mark);
        } else if self.link.is_some() {
            self.link_text.push_str(mark);
        } else {
            self.line.push_str(mark);
        }
    }

    /// Ends the line being built, as a line of its own.
    fn end_line(&mut self) {
        let content = std::mem::take(&mut self.line);
        let trimmed = content.trim_end();
        let indent = "    ".repeat(self.lists.len().saturating_sub(1));
        let mut line = String::new();
        if self.quote {
            line.push_str("> ");
        }
        if let Some(prefix) = self.item_prefix.take() {
            line.push_str(&indent);
            line.push_str(&prefix);
            line.push_str(trimmed.trim_start());
        } else if let Some(level) = self.heading {
            if trimmed.trim().is_empty() {
                return;
            }
            line.push_str(&"#".repeat(level));
            line.push(' ');
            line.push_str(trimmed.trim());
        } else if !self.lists.is_empty() && !trimmed.trim().is_empty() {
            // More text in an item, under its marker.
            line.push_str(&indent);
            line.push_str("    ");
            line.push_str(trimmed.trim_start());
        } else {
            line.push_str(trimmed);
        }
        self.out.push_str(line.trim_end());
        self.out.push('\n');
    }

    fn flush(&mut self) {
        if !self.line.trim().is_empty() || self.item_prefix.is_some() {
            self.end_line();
        } else {
            self.line.clear();
        }
    }
}

/// `html` as markdown, with the pictures it holds (each placeholder is in
/// the markdown, for the caller to replace with where it saved them).
pub(crate) fn to_markdown(html: &str) -> (String, Vec<Picture>) {
    let mut writer = Writer::default();
    for token in tokenize(html) {
        match token {
            Token::Text(text) => writer.text(&text),
            Token::Open { name, attrs } => {
                let attr = |key: &str| {
                    attrs
                        .iter()
                        .find(|(name, _)| name == key)
                        .map(|(_, value)| value.clone())
                };
                match name.as_str() {
                    "div" | "p" => writer.flush(),
                    "br" => {
                        if writer.cell.is_some() {
                            writer.mark(" ");
                        } else if writer.pre {
                            writer.line.push('\n');
                        } else if writer.line.trim().is_empty() && writer.item_prefix.is_none() {
                            // `<div><br></div>`: an empty line.
                            writer.end_line();
                        } else {
                            writer.flush();
                        }
                    },
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        writer.flush();
                        writer.heading = name[1..].parse().ok();
                    },
                    "b" | "strong" => writer.mark("**"),
                    "i" | "em" => writer.mark("*"),
                    "strike" | "s" | "del" => writer.mark("~~"),
                    "u" => {},
                    "tt" | "code" if !writer.pre => writer.mark("`"),
                    "pre" => {
                        writer.flush();
                        writer.out.push_str("```\n");
                        writer.pre = true;
                    },
                    "blockquote" => {
                        writer.flush();
                        writer.quote = true;
                    },
                    "ul" | "ol" => {
                        writer.flush();
                        let class = attr("class").unwrap_or_default();
                        writer.lists.push(List {
                            ordered: name == "ol",
                            checklist: class.contains("checklist"),
                            number: 0,
                        });
                    },
                    "li" => {
                        writer.flush();
                        let checked = attr("class").is_some_and(|class| class.contains("checked"));
                        let prefix = match writer.lists.last_mut() {
                            Some(list) if list.checklist => {
                                if checked {
                                    "- [x] ".to_string()
                                } else {
                                    "- [ ] ".to_string()
                                }
                            },
                            Some(list) if list.ordered => {
                                list.number += 1;
                                format!("{}. ", list.number)
                            },
                            _ => "- ".to_string(),
                        };
                        writer.item_prefix = Some(prefix);
                    },
                    "a" => {
                        writer.link = attr("href");
                        writer.link_text.clear();
                    },
                    "img" => {
                        if let Some(src) = attr("src").filter(|src| !src.is_empty()) {
                            let placeholder = format!("\u{1}picture{}\u{1}", writer.pictures.len());
                            writer.pictures.push(Picture {
                                placeholder: placeholder.clone(),
                                src,
                            });
                            writer.mark(&placeholder);
                        }
                    },
                    "table" => {
                        writer.flush();
                        writer.table.clear();
                    },
                    "tr" => writer.table.push(Vec::new()),
                    "td" | "th" => writer.cell = Some(String::new()),
                    "hr" => {
                        writer.flush();
                        writer.out.push_str("---\n");
                    },
                    _ => {},
                }
            },
            Token::Close(name) => match name.as_str() {
                "div" | "p" => writer.flush(),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    writer.flush();
                    writer.heading = None;
                },
                "b" | "strong" => writer.mark("**"),
                "i" | "em" => writer.mark("*"),
                "strike" | "s" | "del" => writer.mark("~~"),
                "tt" | "code" if !writer.pre => writer.mark("`"),
                "pre" => {
                    writer.flush();
                    writer.pre = false;
                    writer.out.push_str("```\n");
                },
                "blockquote" => {
                    writer.flush();
                    writer.quote = false;
                },
                "li" => writer.flush(),
                "ul" | "ol" => {
                    writer.flush();
                    writer.lists.pop();
                },
                "a" => {
                    if let Some(href) = writer.link.take() {
                        let text = std::mem::take(&mut writer.link_text);
                        let text = text.trim();
                        let shown = if text.is_empty() {
                            href.clone()
                        } else {
                            escape(text)
                        };
                        let link = if text == href || text.is_empty() {
                            format!("<{href}>")
                        } else {
                            format!(
                                "[{shown}]({})",
                                href.replace(' ', "%20").replace(')', "%29")
                            )
                        };
                        writer.mark(&link);
                    }
                },
                "td" | "th" => {
                    if let (Some(cell), Some(row)) = (writer.cell.take(), writer.table.last_mut()) {
                        row.push(cell.trim().to_string());
                    }
                },
                "table" => {
                    let rows = std::mem::take(&mut writer.table);
                    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
                    if width > 0 {
                        for (index, row) in rows.iter().enumerate() {
                            let mut cells = row.clone();
                            cells.resize(width, String::new());
                            writer.out.push_str(&format!("| {} |\n", cells.join(" | ")));
                            if index == 0 {
                                writer
                                    .out
                                    .push_str(&format!("|{}\n", " --- |".repeat(width)));
                            }
                        }
                    }
                },
                _ => {},
            },
        }
    }
    writer.flush();
    (tidy(&writer.out), writer.pictures)
}

/// No more than one empty line in a row, none at the ends; ending in a line break.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut empty = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            empty += 1;
            continue;
        }
        if !out.is_empty() && empty > 0 {
            out.push('\n');
        }
        empty = 0;
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(html: &str) -> String {
        to_markdown(html).0
    }

    #[test]
    fn lines_and_empty_lines() {
        let html = "<div>First line</div><div>Second</div><div><br></div><div>After a gap</div>";
        assert_eq!(md(html), "First line\nSecond\n\nAfter a gap\n");
    }

    #[test]
    fn headings_and_emphasis() {
        let html = "<div><h1>Title</h1></div><div><b>bold</b> and <i>slanted</i> and <strike>gone</strike></div>";
        assert_eq!(md(html), "# Title\n**bold** and *slanted* and ~~gone~~\n");
    }

    #[test]
    fn lists_nest_and_checklists_check() {
        let html = "<ul><li>one</li><li>two<ul><li>inner</li></ul></li></ul><ol><li>a</li><li>b</li></ol>\
                    <ul class=\"checklist\"><li class=\"checked\">done</li><li>todo</li></ul>";
        assert_eq!(
            md(html),
            "- one\n- two\n    - inner\n1. a\n2. b\n- [x] done\n- [ ] todo\n"
        );
    }

    #[test]
    fn links_entities_and_escapes() {
        let html = "<div>See <a href=\"https://example.com/a b\">the site</a> &amp; <a href=\"https://x.io\">https://x.io</a> &lt;3 [not a link] 5*3</div>";
        assert_eq!(
            md(html),
            "See [the site](https://example.com/a%20b) & <https://x.io> <3 \\[not a link\\] 5\\*3\n"
        );
    }

    #[test]
    fn tables_and_monospace() {
        let html = "<table><tr><td>a</td><td>b|c</td></tr><tr><td>1</td><td>2</td></tr></table><div><tt>code</tt></div>";
        assert_eq!(
            md(html),
            "| a | b\\|c |\n| --- | --- |\n| 1 | 2 |\n`code`\n"
        );
    }

    #[test]
    fn pictures_leave_placeholders() {
        let (text, pictures) =
            to_markdown("<div>Look <img src=\"data:image/png;base64,AAAA\"></div>");
        assert_eq!(pictures.len(), 1);
        assert_eq!(text, format!("Look {}\n", pictures[0].placeholder));
    }

    #[test]
    fn text_in_any_script_and_quotes() {
        assert_eq!(
            md("<div>“Quoted” déjà vu &amp; ok</div><div>日本語 &unknown; 🎉&#x1F600;</div>"),
            "“Quoted” déjà vu & ok\n日本語 &unknown; 🎉😀\n"
        );
        assert_eq!(
            md("“starts with a curly quote"),
            "“starts with a curly quote\n"
        );
        assert_eq!(decode("&ééééééééééé;"), "&ééééééééééé;");
    }

    #[test]
    fn never_panics_on_odd_input() {
        // A fixed pseudo-random mix of tags, entities and multi-byte text.
        let pieces = [
            "<div>",
            "</div>",
            "<b>",
            "</i>",
            "<ul>",
            "<li class=\"checked\">",
            "</ul>",
            "<a href=\"x\">",
            "</a>",
            "<img src=\"",
            "\">",
            "<table>",
            "<tr>",
            "<td>",
            "</table>",
            "<br>",
            "&amp;",
            "&#x1F;",
            "&",
            ";",
            "<",
            ">",
            "“",
            "é",
            "日本",
            "🎉",
            "*",
            "[",
            "]",
            "<!--",
            "-->",
            "<style>",
            "</style",
            "\n",
            " ",
            "text",
            "<h1>",
            "<pre>",
            "</pre>",
            "<tt>",
            "%%",
        ];
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..3000 {
            let mut html = String::new();
            for _ in 0..(seed % 40) {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                html.push_str(pieces[(seed % pieces.len() as u64) as usize]);
            }
            let _ = to_markdown(&html);
        }
    }

    #[test]
    fn style_and_comments_are_left_out() {
        assert_eq!(
            md(
                "<html><head><style>div{color:red}</style></head><body><!-- x --><div>Hi</div></body></html>"
            ),
            "Hi\n"
        );
    }
}
