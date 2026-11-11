//! What the app and agents ask the index. Every answer is a page: a limit
//! and offset in, plain rows out, so a vault of any size never sends more
//! than what is on screen.

use {
    super::Index,
    crate::Error,
    rusqlite::{OptionalExtension, params, params_from_iter, types::Value as Sql},
    serde::Serialize,
    serde_json::Value,
};

/// Marks around matched words in a search snippet.
pub const MATCH_START: char = '\u{1}';
pub const MATCH_END: char = '\u{2}';

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Hit {
    pub path: String,
    pub title: String,
    /// Body text around the match, matched words between MATCH_START and MATCH_END.
    pub snippet: String,
    pub score: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Listed {
    pub path: String,
    pub title: String,
    pub excerpt: String,
    pub mtime: i64,
    pub words: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Newest change first.
    Modified,
    Title,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Backlink {
    pub path: String,
    pub title: String,
    pub line: i64,
    pub context: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Outgoing {
    pub target: String,
    pub anchor: Option<String>,
    pub kind: String,
    pub line: i64,
    /// Where it resolves; None for a dangling link.
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Mention {
    pub path: String,
    pub title: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Unresolved {
    pub target: String,
    pub count: i64,
    /// A few notes that link to it.
    pub from: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TaskRow {
    pub path: String,
    pub title: String,
    pub line: i64,
    pub done: bool,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct NoteInfo {
    pub path: String,
    pub title: String,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub owner: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub props: Value,
    pub headings: Vec<(i64, String, i64)>,
    pub words: i64,
    pub mtime: i64,
    pub backlinks: i64,
}

/// A search as typed: words and phrases for the full-text index, and
/// `tag:` `path:` `type:` `owner:` filters; `-word` leaves notes out.
#[derive(Debug, Default, PartialEq, Eq)]
struct Parsed {
    words: Vec<String>,
    excluded: Vec<String>,
    tags: Vec<String>,
    paths: Vec<String>,
    kinds: Vec<String>,
    owners: Vec<String>,
    /// Whether the last word is still being typed (no space after it).
    open_end: bool,
}

fn parse_query(query: &str) -> Parsed {
    let typing = !query.ends_with(char::is_whitespace);
    let mut parsed = Parsed::default();
    let mut chars = query.chars().peekable();
    let mut tokens: Vec<(String, bool)> = Vec::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let mut token = String::new();
        let mut quoted = false;
        if c == '"' {
            chars.next();
            quoted = true;
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                token.push(c);
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                token.push(c);
                chars.next();
            }
        }
        if !token.is_empty() {
            tokens.push((token, quoted));
        }
    }
    let last = tokens.len().saturating_sub(1);
    for (index, (token, quoted)) in tokens.into_iter().enumerate() {
        if !quoted {
            let lower = token.to_lowercase();
            if let Some(tag) = lower
                .strip_prefix("tag:")
                .or_else(|| lower.strip_prefix('#').filter(|_| token.len() > 1))
            {
                parsed.tags.push(tag.trim_start_matches('#').to_string());
                continue;
            }
            if let Some(path) = token.strip_prefix("path:") {
                parsed.paths.push(path.to_string());
                continue;
            }
            if let Some(kind) = lower.strip_prefix("type:") {
                parsed.kinds.push(kind.to_string());
                continue;
            }
            if let Some(owner) = lower.strip_prefix("owner:") {
                parsed.owners.push(owner.to_string());
                continue;
            }
            if let Some(word) = token.strip_prefix('-').filter(|word| !word.is_empty()) {
                parsed.excluded.push(word.to_string());
                continue;
            }
        }
        // Only a bare last word still being typed matches as a prefix.
        parsed.open_end = typing && index == last && !quoted;
        parsed.words.push(token);
    }
    parsed
}

/// A word as an FTS5 string, quoted so its punctuation is literal.
fn fts_term(word: &str) -> String {
    format!("\"{}\"", word.replace('"', "\"\""))
}

impl Index {
    /// Notes matching `query`, best first.
    pub fn search(&self, query: &str, offset: usize, limit: usize) -> Result<Vec<Hit>, Error> {
        let parsed = parse_query(query);
        let mut filters = String::new();
        let mut args: Vec<Sql> = Vec::new();
        for tag in &parsed.tags {
            filters.push_str(" AND EXISTS (SELECT 1 FROM tags t WHERE t.note = n.id AND (t.key = ? OR t.key LIKE ? ESCAPE '\\'))");
            args.push(Sql::Text(tag.clone()));
            args.push(Sql::Text(format!("{}/%", like_escape(tag))));
        }
        for path in &parsed.paths {
            filters.push_str(" AND n.path_key LIKE ? ESCAPE '\\'");
            args.push(Sql::Text(format!("{}%", like_escape(&path.to_lowercase()))));
        }
        for kind in &parsed.kinds {
            filters.push_str(" AND lower(n.kind) = ?");
            args.push(Sql::Text(kind.clone()));
        }
        for owner in &parsed.owners {
            filters.push_str(" AND n.owner = ?");
            args.push(Sql::Text(owner.clone()));
        }
        if parsed.words.is_empty() && parsed.excluded.is_empty() {
            let sql = format!(
                "SELECT n.path, n.title, n.excerpt, 0.0 FROM notes n WHERE 1 = 1{filters} ORDER BY n.sort_time DESC, n.path LIMIT ? OFFSET ?"
            );
            args.push(Sql::Integer(limit as i64));
            args.push(Sql::Integer(offset as i64));
            return self.hits(&sql, args);
        }
        let mut terms: Vec<String> = Vec::new();
        let last = parsed.words.len().saturating_sub(1);
        for (index, word) in parsed.words.iter().enumerate() {
            let mut term = fts_term(word);
            if index == last && parsed.open_end && word.chars().count() >= 2 {
                term.push('*');
            }
            terms.push(term);
        }
        let mut matching = terms.join(" ");
        for word in &parsed.excluded {
            if matching.is_empty() {
                // FTS5 cannot start with NOT; match everything not excluded instead.
                filters.push_str(" AND n.id NOT IN (SELECT rowid FROM fts WHERE fts MATCH ?)");
                args.push(Sql::Text(fts_term(word)));
            } else {
                matching.push_str(&format!(" NOT {}", fts_term(word)));
            }
        }
        if matching.is_empty() {
            let sql = format!(
                "SELECT n.path, n.title, n.excerpt, 0.0 FROM notes n WHERE 1 = 1{filters} ORDER BY n.sort_time DESC LIMIT ? OFFSET ?"
            );
            args.push(Sql::Integer(limit as i64));
            args.push(Sql::Integer(offset as i64));
            return self.hits(&sql, args);
        }
        let sql = format!(
            "SELECT n.path, n.title,
                    snippet(fts, 2, char(1), char(2), '…', 14),
                    bm25(fts, 10.0, 6.0, 1.0)
             FROM fts JOIN notes n ON n.id = fts.rowid
             WHERE fts MATCH ?{filters}
             ORDER BY bm25(fts, 10.0, 6.0, 1.0), n.sort_time DESC
             LIMIT ? OFFSET ?"
        );
        let mut all = vec![Sql::Text(matching)];
        all.extend(args);
        all.push(Sql::Integer(limit as i64));
        all.push(Sql::Integer(offset as i64));
        self.hits(&sql, all)
    }

    fn hits(&self, sql: &str, args: Vec<Sql>) -> Result<Vec<Hit>, Error> {
        let mut statement = self.db.prepare_cached(sql)?;
        let rows = statement.query_map(params_from_iter(args), |row| {
            Ok(Hit {
                path: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get(2)?,
                score: -row.get::<_, f64>(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Notes whose title or alias matches `typed`, as a quick switcher wants
    /// them: a prefix first, then a word's prefix, then anywhere, then the
    /// letters in order; among equals, the most recently changed. Each
    /// note once, with the alias that matched when it was an alias.
    pub fn switch(
        &self,
        typed: &str,
        limit: usize,
    ) -> Result<Vec<(String, String, Option<String>)>, Error> {
        let typed = typed.trim().to_lowercase();
        let mut scored: Vec<(u8, i64, String, String, Option<String>)> = Vec::new();
        {
            let mut statement = self.db.prepare_cached(
                "SELECT n.path, n.title, n.sort_time, NULL FROM notes n
                 UNION ALL
                 SELECT n.path, n.title, n.sort_time, a.alias FROM aliases a JOIN notes n ON n.id = a.note",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?;
            for row in rows {
                let (path, title, mtime, alias) = row?;
                let name = alias.as_deref().unwrap_or(&title);
                if let Some(rank) = rank(&typed, &name.to_lowercase()) {
                    let rank = rank * 2 + u8::from(alias.is_some());
                    scored.push((rank, -mtime, path, title, alias));
                }
            }
        }
        scored.sort();
        let mut seen = std::collections::HashSet::new();
        Ok(scored
            .into_iter()
            .filter(|(_, _, path, ..)| seen.insert(path.clone()))
            .take(limit)
            .map(|(_, _, path, title, alias)| (path, title, alias))
            .collect())
    }

    /// A folder's notes (all with `""`), a page at a time.
    pub fn list(
        &self,
        folder: &str,
        order: Order,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Listed>, Error> {
        let order = match order {
            Order::Modified => "sort_time DESC, path",
            Order::Title => "title_key, path",
        };
        let prefix = if folder.is_empty() {
            "%".to_string()
        } else {
            format!(
                "{}/%",
                like_escape(&folder.trim_end_matches('/').to_lowercase())
            )
        };
        let sql = format!(
            "SELECT path, title, excerpt, sort_time, words FROM notes WHERE path_key LIKE ?1 ESCAPE '\\' ORDER BY {order} LIMIT ?2 OFFSET ?3"
        );
        let mut statement = self.db.prepare_cached(&sql)?;
        let rows = statement.query_map(params![prefix, limit as i64, offset as i64], |row| {
            Ok(Listed {
                path: row.get(0)?,
                title: row.get(1)?,
                excerpt: row.get(2)?,
                mtime: row.get(3)?,
                words: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// How many notes a folder holds (all with `""`).
    pub fn count(&self, folder: &str) -> Result<i64, Error> {
        let prefix = if folder.is_empty() {
            "%".to_string()
        } else {
            format!(
                "{}/%",
                like_escape(&folder.trim_end_matches('/').to_lowercase())
            )
        };
        Ok(self.db.query_row(
            "SELECT count(*) FROM notes WHERE path_key LIKE ?1 ESCAPE '\\'",
            [prefix],
            |row| row.get(0),
        )?)
    }

    /// Links from other notes into `path`, newest note first.
    pub fn backlinks(
        &self,
        path: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Backlink>, Error> {
        let Some(id) = self.row_of(path)? else {
            return Ok(Vec::new());
        };
        let mut statement = self.db.prepare_cached(
            "SELECT s.path, s.title, l.line, l.context, l.kind FROM links l JOIN notes s ON s.id = l.note
             WHERE l.dst = ?1 AND l.note != ?1 ORDER BY s.sort_time DESC, s.path, l.line LIMIT ?2 OFFSET ?3",
        )?;
        let rows = statement.query_map(params![id, limit as i64, offset as i64], |row| {
            Ok(Backlink {
                path: row.get(0)?,
                title: row.get(1)?,
                line: row.get(2)?,
                context: row.get(3)?,
                kind: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The links in `path`, in order, each with where it resolves.
    pub fn outgoing(&self, path: &str) -> Result<Vec<Outgoing>, Error> {
        let mut statement = self.db.prepare_cached(
            "SELECT l.target, l.anchor, l.kind, l.line, d.path FROM links l JOIN notes s ON s.id = l.note
             LEFT JOIN notes d ON d.id = l.dst WHERE s.path = ?1 ORDER BY l.line",
        )?;
        let rows = statement.query_map([path], |row| {
            Ok(Outgoing {
                target: row.get(0)?,
                anchor: row.get(1)?,
                kind: row.get(2)?,
                line: row.get(3)?,
                path: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Notes that name `path`'s title or an alias in their text without
    /// linking to it.
    pub fn mentions(&self, path: &str, limit: usize) -> Result<Vec<Mention>, Error> {
        let Some(id) = self.row_of(path)? else {
            return Ok(Vec::new());
        };
        let mut names: Vec<String> =
            vec![
                self.db
                    .query_row("SELECT title FROM notes WHERE id = ?1", [id], |row| {
                        row.get(0)
                    })?,
            ];
        {
            let mut statement = self
                .db
                .prepare_cached("SELECT alias FROM aliases WHERE note = ?1")?;
            for alias in statement.query_map([id], |row| row.get::<_, String>(0))? {
                names.push(alias?);
            }
        }
        let phrases: Vec<String> = names
            .iter()
            .filter(|name| name.chars().count() >= 3)
            .map(|name| fts_term(name))
            .collect();
        if phrases.is_empty() {
            return Ok(Vec::new());
        }
        let matching = format!("body : ({})", phrases.join(" OR "));
        let mut statement = self.db.prepare_cached(
            "SELECT n.path, n.title, snippet(fts, 2, char(1), char(2), '…', 12) FROM fts JOIN notes n ON n.id = fts.rowid
             WHERE fts MATCH ?1 AND n.id != ?2 AND NOT EXISTS (SELECT 1 FROM links l WHERE l.note = n.id AND l.dst = ?2)
             ORDER BY bm25(fts) LIMIT ?3",
        )?;
        let rows = statement.query_map(params![matching, id, limit as i64], |row| {
            Ok(Mention {
                path: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Links to notes that do not exist, most linked first. Attachments
    /// (a target with an extension other than `.md`) are left out.
    pub fn unresolved(&self, limit: usize) -> Result<Vec<Unresolved>, Error> {
        let mut statement = self.db.prepare_cached(
            "SELECT min(l.target), count(*), group_concat(DISTINCT s.path) FROM links l JOIN notes s ON s.id = l.note
             WHERE l.dst IS NULL AND l.target_key != '' GROUP BY l.target_key ORDER BY count(*) DESC, l.target_key",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (target, count, from) = row?;
            if is_attachment(&target) {
                continue;
            }
            let from: Vec<String> = from
                .unwrap_or_default()
                .split(',')
                .take(5)
                .map(str::to_string)
                .collect();
            out.push(Unresolved {
                target,
                count,
                from,
            });
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// Notes nothing else links to, oldest change first, optionally only
    /// under `folder` (`wiki`, say).
    pub fn orphans(&self, folder: &str, limit: usize) -> Result<Vec<Listed>, Error> {
        let prefix = if folder.is_empty() {
            "%".to_string()
        } else {
            format!("{}/%", like_escape(&folder.to_lowercase()))
        };
        let mut statement = self.db.prepare_cached(
            "SELECT n.path, n.title, n.excerpt, n.sort_time, n.words FROM notes n WHERE n.path_key LIKE ?1 ESCAPE '\\'
             AND NOT EXISTS (SELECT 1 FROM links l WHERE l.dst = n.id AND l.note != n.id) ORDER BY n.sort_time LIMIT ?2",
        )?;
        let rows = statement.query_map(params![prefix, limit as i64], |row| {
            Ok(Listed {
                path: row.get(0)?,
                title: row.get(1)?,
                excerpt: row.get(2)?,
                mtime: row.get(3)?,
                words: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every tag with how many notes carry it, most used first. Nested tags
    /// count on their own (`a/b` is not counted under `a`).
    pub fn tags(&self) -> Result<Vec<TagCount>, Error> {
        let mut statement = self.db.prepare_cached("SELECT min(tag), count(DISTINCT note) FROM tags GROUP BY key ORDER BY count(DISTINCT note) DESC, key")?;
        let rows = statement.query_map([], |row| {
            Ok(TagCount {
                tag: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Tasks across the vault, open ones only unless `done` is true.
    pub fn tasks(&self, done: bool, offset: usize, limit: usize) -> Result<Vec<TaskRow>, Error> {
        let mut statement = self.db.prepare_cached(
            "SELECT n.path, n.title, t.line, t.done, t.text FROM tasks t JOIN notes n ON n.id = t.note
             WHERE (?1 OR t.done = 0) ORDER BY n.sort_time DESC, t.line LIMIT ?2 OFFSET ?3",
        )?;
        let rows = statement.query_map(params![done, limit as i64, offset as i64], |row| {
            Ok(TaskRow {
                path: row.get(0)?,
                title: row.get(1)?,
                line: row.get(2)?,
                done: row.get(3)?,
                text: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One note's details, without its text.
    pub fn info(&self, path: &str) -> Result<Option<NoteInfo>, Error> {
        let found = self
            .db
            .query_row(
                "SELECT id, path, title, note_id, kind, owner, props, words, sort_time FROM notes WHERE path = ?1",
                [path],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, path, title, note_id, kind, owner, props, words, mtime)) = found else {
            return Ok(None);
        };
        let strings = |sql: &str| -> Result<Vec<String>, Error> {
            let mut statement = self.db.prepare_cached(sql)?;
            let rows = statement.query_map([id], |row| row.get::<_, String>(0))?;
            Ok(rows.collect::<Result<_, _>>()?)
        };
        let aliases = strings("SELECT alias FROM aliases WHERE note = ?1 ORDER BY rowid")?;
        let tags = strings("SELECT tag FROM tags WHERE note = ?1 ORDER BY rowid")?;
        let headings = {
            let mut statement = self.db.prepare_cached(
                "SELECT level, text, line FROM headings WHERE note = ?1 ORDER BY line",
            )?;
            let rows =
                statement.query_map([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let backlinks = self.db.query_row(
            "SELECT count(*) FROM links WHERE dst = ?1 AND note != ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(Some(NoteInfo {
            path,
            title,
            id: note_id,
            kind,
            owner,
            aliases,
            tags,
            props: serde_json::from_str(&props).unwrap_or(Value::Null),
            headings,
            words,
            mtime,
            backlinks,
        }))
    }

    /// The path of the note `target` (as a link would write it) resolves to.
    pub fn resolve_path(&self, target: &str, from: &str) -> Result<Option<String>, Error> {
        let key = crate::text::key(target);
        let tx = self.db.unchecked_transaction()?;
        let resolver = super::Resolver::load(&tx)?;
        let found = match resolver.resolve(&key, from) {
            Some(id) => tx
                .query_row("SELECT path FROM notes WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()?,
            None => None,
        };
        tx.finish()?;
        Ok(found)
    }
}

/// How well `typed` matches `name` (both lowercase): 0 a prefix, 1 a word's
/// prefix, 2 anywhere, 3 the letters in order; None when it does not.
fn rank(typed: &str, name: &str) -> Option<u8> {
    if typed.is_empty() || name.starts_with(typed) {
        return Some(0);
    }
    if name
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(typed))
    {
        return Some(1);
    }
    if name.contains(typed) {
        return Some(2);
    }
    let mut letters = name.chars();
    typed
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|wanted| letters.any(|c| c == wanted))
        .then_some(3)
}

fn is_attachment(target: &str) -> bool {
    let name = target.rsplit('/').next().unwrap_or(target);
    match name.rsplit_once('.') {
        Some((stem, extension)) => {
            !stem.is_empty()
                && !extension.eq_ignore_ascii_case("md")
                && (1..=5).contains(&extension.len())
                && extension.chars().all(|c| c.is_ascii_alphanumeric())
        },
        None => false,
    }
}

fn like_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_split_into_words_and_filters() {
        let parsed = parse_query(
            "spaced \"forgetting curve\" tag:Learning #memory path:wiki/ type:concept -anki rep",
        );
        assert_eq!(parsed.words, vec!["spaced", "forgetting curve", "rep"]);
        assert_eq!(parsed.tags, vec!["learning", "memory"]);
        assert_eq!(parsed.paths, vec!["wiki/"]);
        assert_eq!(parsed.kinds, vec!["concept"]);
        assert_eq!(parsed.excluded, vec!["anki"]);
        assert!(parsed.open_end);
        assert!(!parse_query("done ").open_end);
    }

    #[test]
    fn switcher_ranks() {
        assert_eq!(rank("spa", "spaced repetition"), Some(0));
        assert_eq!(rank("rep", "spaced repetition"), Some(1));
        assert_eq!(rank("ced", "spaced repetition"), Some(2));
        assert_eq!(rank("sprt", "spaced repetition"), Some(3));
        assert_eq!(rank("xyz", "spaced repetition"), None);
    }

    #[test]
    fn attachments_have_other_extensions() {
        assert!(is_attachment("Pic.png"));
        assert!(is_attachment("raw/assets/paper.pdf"));
        assert!(!is_attachment("Note.md"));
        assert!(!is_attachment("Dr. Who"));
        assert!(!is_attachment("Version 1.0 notes"));
    }
}
