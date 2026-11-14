//! Which note a link points at (VAULT.md, "Resolving a link"): a path
//! from the vault's root, else a file name, else an alias, ignoring case.
//! Of several notes with one name, the one nearest the linking note's
//! folder wins, then the shortest path.

use {
    crate::Error,
    rusqlite::{Transaction, params},
    std::collections::{HashMap, HashSet},
};

pub(super) struct Resolver {
    /// path key -> row
    paths: HashMap<String, i64>,
    /// title or alias key -> rows with their paths
    names: HashMap<String, Vec<(i64, String)>>,
    /// Every (path key, row), for `folder/name` targets that are not from the root.
    all: Vec<(String, i64, String)>,
}

impl Resolver {
    pub(super) fn load(tx: &Transaction) -> Result<Self, Error> {
        let mut paths = HashMap::new();
        let mut names: HashMap<String, Vec<(i64, String)>> = HashMap::new();
        let mut all = Vec::new();
        {
            let mut statement = tx.prepare("SELECT id, path, path_key, title_key FROM notes")?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            for row in rows {
                let (id, path, path_key, title_key) = row?;
                paths.insert(path_key.clone(), id);
                names.entry(title_key).or_default().push((id, path.clone()));
                all.push((path_key, id, path));
            }
        }
        {
            let mut statement = tx.prepare("SELECT aliases.note, aliases.key, notes.path FROM aliases JOIN notes ON notes.id = aliases.note")?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (id, key, path) = row?;
                let entry = names.entry(key).or_default();
                if !entry.iter().any(|(seen, _)| *seen == id) {
                    entry.push((id, path));
                }
            }
        }
        Ok(Self { paths, names, all })
    }

    /// The row `target_key` (from a note at `from`) points at.
    pub(super) fn resolve(&self, target_key: &str, from: &str) -> Option<i64> {
        if target_key.is_empty() {
            return None;
        }
        if let Some(&id) = self.paths.get(target_key) {
            return Some(id);
        }
        if let Some(candidates) = self.names.get(target_key) {
            return nearest(
                candidates.iter().map(|(id, path)| (*id, path.as_str())),
                from,
            );
        }
        if target_key.contains('/') {
            let suffix = format!("/{target_key}");
            return nearest(
                self.all
                    .iter()
                    .filter(|(key, ..)| key.ends_with(&suffix))
                    .map(|(_, id, path)| (*id, path.as_str())),
                from,
            );
        }
        None
    }

    /// Resolves the links of the given notes.
    pub(super) fn resolve_notes(&self, tx: &Transaction, notes: &[i64]) -> Result<(), Error> {
        for &note in notes {
            self.update(tx, "SELECT links.rowid, links.target_key, notes.path, links.note FROM links JOIN notes ON notes.id = links.note WHERE links.note = ?1", params![note])?;
        }
        Ok(())
    }

    /// Resolves every link written with one of `keys` (or a path ending in one).
    pub(super) fn resolve_keys(
        &self,
        tx: &Transaction,
        keys: &HashSet<String>,
    ) -> Result<(), Error> {
        for key in keys {
            if key.is_empty() {
                continue;
            }
            self.update(
                tx,
                "SELECT links.rowid, links.target_key, notes.path, links.note FROM links JOIN notes ON notes.id = links.note WHERE links.target_key = ?1",
                params![key],
            )?;
            // `[[folder/name]]` written against a note deeper in the vault.
            let name = key.rsplit('/').next().unwrap_or(key);
            self.update(
                tx,
                "SELECT links.rowid, links.target_key, notes.path, links.note FROM links JOIN notes ON notes.id = links.note
                 WHERE links.target_name = ?1 AND links.target_key != ?2",
                params![name, key],
            )?;
        }
        Ok(())
    }

    pub(super) fn resolve_all(&self, tx: &Transaction) -> Result<(), Error> {
        self.update(tx, "SELECT links.rowid, links.target_key, notes.path, links.note FROM links JOIN notes ON notes.id = links.note", [])
    }

    fn update(
        &self,
        tx: &Transaction,
        select: &str,
        args: impl rusqlite::Params,
    ) -> Result<(), Error> {
        let found: Vec<(i64, String, String, i64)> = {
            let mut statement = tx.prepare_cached(select)?;
            let rows = statement.query_map(args, |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            rows.collect::<Result<_, _>>()?
        };
        let mut statement = tx.prepare_cached("UPDATE links SET dst = ?2 WHERE rowid = ?1")?;
        for (rowid, target_key, from, note) in found {
            // `[[#Heading]]` points into its own note.
            let dst = if target_key.is_empty() {
                Some(note)
            } else {
                self.resolve(&target_key, &from)
            };
            statement.execute(params![rowid, dst])?;
        }
        Ok(())
    }
}

/// Of `candidates`, the one sharing the most folders with `from`, then the
/// shortest path, then the first by path.
fn nearest<'a>(candidates: impl Iterator<Item = (i64, &'a str)>, from: &str) -> Option<i64> {
    let from_folders: Vec<&str> = folders(from);
    candidates
        .map(|(id, path)| {
            let shared = folders(path)
                .iter()
                .zip(&from_folders)
                .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
                .count();
            (std::cmp::Reverse(shared), path.len(), path.to_string(), id)
        })
        .min()
        .map(|(_, _, _, id)| id)
}

fn folders(path: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_prefers_shared_folders_then_short_paths() {
        let candidates = [(1, "a/b/Note.md"), (2, "a/Note.md"), (3, "z/Note.md")];
        assert_eq!(
            nearest(candidates.iter().copied(), "a/b/c/From.md"),
            Some(1)
        );
        assert_eq!(nearest(candidates.iter().copied(), "a/x/From.md"), Some(2));
        assert_eq!(nearest(candidates.iter().copied(), "From.md"), Some(2));
    }
}
