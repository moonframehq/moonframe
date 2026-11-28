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
    /// When it changes folder, its own relative links follow it. The move
    /// and every rewrite are journaled as one batch first, so a run that
    /// stops part way finishes them when the vault next opens.
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
        // Changing only the case of a name is a rename on any disk, but on a
        // case-sensitive one `a.md` beside `A.md` is another note.
        let case_only = from.eq_ignore_ascii_case(&to) && from != to && !names_a_file(&target_file);
        if target_file.exists() && !case_only {
            return Err(Error::Exists(to));
        }
        let Some(id) = self.index.row_of(from)? else {
            return Err(Error::NotFound(from.to_string()));
        };

        // The notes linking to it, read before anything moves, and the note
        // itself when its own relative links must follow it.
        let mut sources: Vec<String> = {
            let mut statement = self.index.db().prepare(
                "SELECT DISTINCT s.path FROM links l JOIN notes s ON s.id = l.note WHERE l.dst = ?1 AND l.target_key != '' ORDER BY s.path",
            )?;
            let rows = statement.query_map(params![id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        if folder_of(from) != folder_of(&to) && !sources.iter().any(|source| source == from) {
            sources.push(from.to_string());
        }

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
        let plan = Plan {
            from,
            to: &to,
            id,
            wiki_name: &wiki_name,
        };

        // Each rewrite as (where it is read, where it is written, the text
        // it was planned against, the new text, links changed).
        let mut rewrites: Vec<(String, String, String, String, usize)> = Vec::new();
        for source in &sources {
            let current = fs::read_to_string(self.root.join(source))?;
            // A note linking to itself moves with it.
            let after = if source == from {
                to.clone()
            } else {
                source.clone()
            };
            let (rewritten, count) = self.rewrite(&current, source, &after, &plan)?;
            if count > 0 {
                rewrites.push((source.clone(), after, current, rewritten, count));
            }
        }

        let batch = write::Batch {
            moves: vec![(from.to_string(), to.clone())],
            writes: rewrites
                .iter()
                .map(|(_, after, current, rewritten, _)| write::BatchWrite {
                    path: after.clone(),
                    base: text::digest(current),
                    text: rewritten.clone(),
                })
                .collect(),
        };
        let entry = self.journal.record_batch(&batch)?;
        if let Some(folder) = target_file.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::rename(&source_file, &target_file)?;
        let mut renamed = Renamed {
            from: from.to_string(),
            to: to.clone(),
            ..Renamed::default()
        };
        for (source, after, current, rewritten, count) in rewrites {
            let file = self.root.join(&after);
            let disk = fs::read_to_string(&file)?;
            // Edited since it was read (Obsidian, a sync, the agent): rewrite
            // the links in what is there now instead of writing over it.
            let (contents, count) = if disk == current {
                (rewritten, count)
            } else {
                self.rewrite(&disk, &source, &after, &plan)?
            };
            if count == 0 {
                continue;
            }
            write::atomic_write(&file, contents.as_bytes())?;
            renamed.links += count;
            renamed.rewritten.push(after);
        }
        self.journal.clear_batch(&entry);
        scan::refresh(&self.root, &mut self.index, from)?;
        scan::refresh(&self.root, &mut self.index, &to)?;
        for path in &renamed.rewritten {
            scan::refresh(&self.root, &mut self.index, path)?;
        }
        Ok(renamed)
    }

    /// `source` (the text of the note at `path`, to be at `after`) with each
    /// link to the renamed note pointed at its new name (wikilinks) or place
    /// (markdown links, relative to `after`'s folder). When the note itself
    /// changes folder, its other relative markdown links are made relative
    /// to the new folder too.
    fn rewrite(
        &self,
        source: &str,
        path: &str,
        after: &str,
        plan: &Plan,
    ) -> Result<(String, usize), Error> {
        let spans = parse::spans(source);
        // Only links naming the note by its old title or path change; one
        // written with an alias still reads right after the move.
        let old_path = text::key(plan.from);
        let old_title = text::key(text::stem(plan.from));
        let by_name = |target: &str| {
            let key = text::key(target);
            key == old_path || key == old_title || old_path.ends_with(&format!("/{key}"))
        };
        let moves_folder = folder_of(path) != folder_of(after);
        let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
        for link in spans.wiki {
            if link.target.is_empty() {
                continue;
            }
            if by_name(&link.target) && self.points_at(&link.target, path, plan.id)? {
                edits.push((link.target_span, plan.wiki_name.to_string()));
            }
        }
        for (url, target) in spans.markdown {
            let new_target = if self.points_at(&target, path, plan.id)? {
                Some(plan.to.to_string())
            } else if moves_folder {
                self.moved_target(&target, path, after)?
            } else {
                None
            };
            let Some(new_target) = new_target else {
                continue;
            };
            let anchor = source.get(url.clone()).and_then(|written| {
                written
                    .split_once('#')
                    .map(|(_, anchor)| anchor.to_string())
            });
            let mut new_url = encode(&relative_to(after, &new_target));
            if let Some(anchor) = anchor {
                new_url.push('#');
                new_url.push_str(&anchor);
            }
            edits.push((url, new_url));
        }
        edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let count = edits.len();
        let mut out = source.to_string();
        for (range, replacement) in edits {
            out.replace_range(range, &replacement);
        }
        Ok((out, count))
    }

    /// Where a markdown link `target` in a note moving from `path` to
    /// `after` must point, when it would stop reaching what it reaches now:
    /// the note it resolves to, else a file (an image, a PDF) beside the
    /// note's old folder. None when it still reaches the same place.
    fn moved_target(&self, target: &str, path: &str, after: &str) -> Result<Option<String>, Error> {
        if let Some(note) = self.index.resolve_path(target, path)? {
            // A path from the note's folder must still be one, for viewers
            // that do not look notes up by name.
            let reaches_from = |place: &str| {
                text::from_folder(target, place)
                    .is_some_and(|found| text::key(&found) == text::key(&note))
            };
            let still = self.index.resolve_path(target, after)?.as_deref() == Some(note.as_str());
            let moved = (reaches_from(path) && !reaches_from(after)) || !still;
            return Ok(moved.then_some(note));
        }
        let Some(file) = text::from_folder(target, path) else {
            return Ok(None);
        };
        let reaches =
            |place: Option<String>| place.is_some_and(|place| self.root.join(place).is_file());
        let still = reaches(text::from_folder(target, after)) || reaches(Some(target.to_string()));
        Ok((!still && self.root.join(&file).is_file()).then_some(file))
    }

    fn points_at(&self, target: &str, from: &str, id: i64) -> Result<bool, Error> {
        let Some(path) = self.index.resolve_path(target, from)? else {
            return Ok(false);
        };
        Ok(self.index.row_of(&path)? == Some(id))
    }
}

/// What a rename is doing, for rewriting the links of each note.
struct Plan<'a> {
    from: &'a str,
    to: &'a str,
    id: i64,
    wiki_name: &'a str,
}

/// The folder part of a vault path (`""` at the root).
fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// Whether `file`'s folder holds an entry with exactly its name: on a disk
/// that ignores case, `exists` is also true for a name differing only in
/// case.
fn names_a_file(file: &std::path::Path) -> bool {
    let (Some(folder), Some(name)) = (file.parent(), file.file_name()) else {
        return false;
    };
    fs::read_dir(folder)
        .is_ok_and(|entries| entries.flatten().any(|entry| entry.file_name() == name))
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
