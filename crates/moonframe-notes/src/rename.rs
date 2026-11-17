//! Renaming or moving a note and rewriting every link to it, as one
//! journaled batch (VAULT.md, "Writes").

use {
    crate::{Error, Vault, parse, scan, text, write},
    rusqlite::params,
    std::fs,
};

/// What a rename changed.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Renamed {
    pub from: String,
    pub to: String,
    /// Notes whose links were rewritten.
    pub rewritten: Vec<String>,
    pub links: usize,
}

impl Vault {
    /// Moves the note at `from` to `to` (both vault paths) and rewrites the
    /// links that pointed at it, keeping their `#anchor` and `|display`.
    pub fn rename(&mut self, from: &str, to: &str) -> Result<Renamed, Error> {
        let to = to.trim().trim_start_matches('/').to_string();
        let to = if to.to_lowercase().ends_with(".md") {
            to
        } else {
            format!("{to}.md")
        };
        if !scan::is_note(&to) {
            return Err(Error::Invalid(format!("{to} is not a place for a note")));
        }
        let source_file = self.root.join(from);
        let target_file = self.root.join(&to);
        if !source_file.is_file() {
            return Err(Error::NotFound(from.to_string()));
        }
        let case_only = from.eq_ignore_ascii_case(&to) && from != to;
        if target_file.exists() && !case_only {
            return Err(Error::Exists(to));
        }
        let Some(id) = self.index.row_of(from)? else {
            return Err(Error::NotFound(from.to_string()));
        };

        // The notes linking to it, read before anything moves.
        let sources: Vec<String> = {
            let mut statement = self.index.db().prepare(
                "SELECT DISTINCT s.path FROM links l JOIN notes s ON s.id = l.note WHERE l.dst = ?1 AND l.target_key != '' ORDER BY s.path",
            )?;
            let rows = statement.query_map(params![id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };

        // How links will name it: by title when that stays unique, else by path.
        let new_title = text::stem(&to).to_string();
        let others: i64 = self.index.db().query_row(
            "SELECT count(*) FROM notes WHERE title_key = ?1 AND id != ?2",
            params![text::key(&new_title), id],
            |row| row.get(0),
        )?;
        let wiki_name = if others == 0 {
            new_title.clone()
        } else {
            text::strip_md(&to).to_string()
        };

        let mut rewrites: Vec<(String, String, usize)> = Vec::new();
        for source in &sources {
            let current = fs::read_to_string(self.root.join(source))?;
            let (rewritten, count) = self.rewrite(&current, source, id, &wiki_name, &to)?;
            if count > 0 {
                // A note linking to itself moves with it.
                let path = if source == from {
                    to.clone()
                } else {
                    source.clone()
                };
                rewrites.push((path, rewritten, count));
            }
        }

        if let Some(folder) = target_file.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::rename(&source_file, &target_file)?;
        let mut renamed = Renamed {
            from: from.to_string(),
            to: to.clone(),
            ..Renamed::default()
        };
        for (path, contents, count) in rewrites {
            write::write_journaled(&self.root, &self.journal, &path, &contents)?;
            renamed.links += count;
            renamed.rewritten.push(path);
        }
        scan::refresh(&self.root, &mut self.index, from)?;
        scan::refresh(&self.root, &mut self.index, &to)?;
        for path in &renamed.rewritten {
            scan::refresh(&self.root, &mut self.index, path)?;
        }
        Ok(renamed)
    }

    /// `source` (the text of the note at `path`) with each link that
    /// resolves to row `id` pointed at `wiki_name` (wikilinks) or `to`
    /// (markdown links, relative to the note's folder).
    fn rewrite(
        &self,
        source: &str,
        path: &str,
        id: i64,
        wiki_name: &str,
        to: &str,
    ) -> Result<(String, usize), Error> {
        let spans = parse::spans(source);
        // Only links naming the note by its old title or path change; one
        // written with an alias still reads right after the move.
        let old = self.index.db().query_row(
            "SELECT path FROM notes WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )?;
        let old_path = text::key(&old);
        let old_title = text::key(text::stem(&old));
        let by_name = |target: &str| {
            let key = text::key(target);
            key == old_path || key == old_title || old_path.ends_with(&format!("/{key}"))
        };
        let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
        for link in spans.wiki {
            if link.target.is_empty() {
                continue;
            }
            if by_name(&link.target) && self.points_at(&link.target, path, id)? {
                edits.push((link.target_span, wiki_name.to_string()));
            }
        }
        for (url, target) in spans.markdown {
            if self.points_at(&target, path, id)? {
                let anchor = source.get(url.clone()).and_then(|written| {
                    written
                        .split_once('#')
                        .map(|(_, anchor)| anchor.to_string())
                });
                let mut new_url = encode(&relative_to(path, to));
                if let Some(anchor) = anchor {
                    new_url.push('#');
                    new_url.push_str(&anchor);
                }
                edits.push((url, new_url));
            }
        }
        edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let count = edits.len();
        let mut out = source.to_string();
        for (range, replacement) in edits {
            out.replace_range(range, &replacement);
        }
        Ok((out, count))
    }

    fn points_at(&self, target: &str, from: &str, id: i64) -> Result<bool, Error> {
        let Some(path) = self.index.resolve_path(target, from)? else {
            return Ok(false);
        };
        Ok(self.index.row_of(&path)? == Some(id))
    }
}

/// `to` as a path from the folder holding `from`.
fn relative_to(from: &str, to: &str) -> String {
    let from_parts: Vec<&str> = from.split('/').collect();
    let from_folders = &from_parts[..from_parts.len().saturating_sub(1)];
    let to_parts: Vec<&str> = to.split('/').collect();
    let shared = from_folders
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut out: Vec<&str> = vec![".."; from_folders.len() - shared];
    out.extend(&to_parts[shared..]);
    out.join("/")
}

/// Spaces and the few characters a markdown url cannot hold, escaped.
fn encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '#' => out.push_str("%23"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_climb_out_of_folders() {
        assert_eq!(relative_to("notes/A.md", "notes/B.md"), "B.md");
        assert_eq!(relative_to("notes/A.md", "wiki/B.md"), "../wiki/B.md");
        assert_eq!(relative_to("A.md", "wiki/x/B.md"), "wiki/x/B.md");
        assert_eq!(encode("My Note (1).md"), "My%20Note%20%281%29.md");
    }
}
