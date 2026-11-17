//! Bringing the index in step with the folder: every markdown file is
//! looked at by size and time, read again only when those changed, and
//! parsed again only when its text did. Files are parsed on every core;
//! the index is written in one transaction.

use {
    crate::{
        Error,
        index::{Index, Stamp},
        parse::{self, Note},
    },
    std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        thread,
        time::UNIX_EPOCH,
    },
    walkdir::WalkDir,
};

/// Largest file indexed; bigger ones are left out (and reported).
pub const MAX_FILE: u64 = 4 * 1024 * 1024;

/// What a scan did.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub files: usize,
    pub parsed: usize,
    pub removed: usize,
    /// Files skipped: too large, or not UTF-8.
    pub skipped: Vec<String>,
}

/// Whether `relative` (a vault path) is a note the vault indexes: a `.md`
/// file outside hidden folders (`.git`, `.obsidian`, `.commonplace`).
pub fn is_note(relative: &str) -> bool {
    let is_markdown = Path::new(relative)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
    is_markdown
        && !relative
            .split('/')
            .any(|part| part.starts_with('.') || part.is_empty())
}

/// The vault path of `file` under `root`, `/`-separated.
pub fn relative(root: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(root).ok()?;
    let parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

pub(crate) fn stamp_of(file: &Path) -> Option<(i64, i64)> {
    let meta = fs::metadata(file).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some((
        i64::try_from(mtime).unwrap_or(i64::MAX),
        i64::try_from(meta.len()).unwrap_or(i64::MAX),
    ))
}

/// Every note file under `root`, with its size and time.
fn walk(root: &Path) -> Vec<(String, PathBuf, i64, i64)> {
    let mut out = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0 || !entry.file_name().to_string_lossy().starts_with('.')
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(relative) = relative(root, entry.path()) else {
            continue;
        };
        if !is_note(&relative) {
            continue;
        }
        if let Some((mtime, size)) = stamp_of(entry.path()) {
            out.push((relative, entry.path().to_path_buf(), mtime, size));
        }
    }
    out
}

/// Brings `index` in step with every file under `root`.
pub(crate) fn scan(root: &Path, index: &mut Index) -> Result<Report, Error> {
    let known: HashMap<String, Stamp> = index.stamps()?.into_iter().collect();
    let files = walk(root);
    let mut report = Report {
        files: files.len(),
        ..Report::default()
    };
    let mut seen = std::collections::HashSet::with_capacity(files.len());
    let mut to_read = Vec::new();
    for (relative, file, mtime, size) in files {
        seen.insert(relative.clone());
        match known.get(&relative) {
            Some(stamp) if stamp.mtime == mtime && stamp.size == size => {},
            _ => to_read.push((relative, file, mtime, size)),
        }
    }
    let removed: Vec<String> = known
        .keys()
        .filter(|path| !seen.contains(*path))
        .cloned()
        .collect();
    report.removed = removed.len();

    let results = read_all(to_read);
    let mut changed: Vec<(Note, Stamp)> = Vec::new();
    let mut touched = Vec::new();
    for result in results {
        match result {
            Read::Note(note, stamp) => {
                if known
                    .get(&note.path)
                    .is_some_and(|old| old.hash == stamp.hash)
                {
                    touched.push((note.path.clone(), stamp));
                } else {
                    changed.push((*note, stamp));
                }
            },
            Read::Skipped(path) => report.skipped.push(path),
        }
    }
    report.parsed = changed.len();
    index.apply(&changed, &removed, &touched)?;
    report.skipped.sort();
    Ok(report)
}

/// Reads and indexes one file again (after a write or a watcher's event).
/// A file gone, too large or not a note is removed from the index.
pub(crate) fn refresh(root: &Path, index: &mut Index, relative: &str) -> Result<(), Error> {
    let file = root.join(relative);
    if !is_note(relative) || !file.is_file() {
        return index.apply(&[], &[relative.to_string()], &[]);
    }
    match read_one(relative.to_string(), &file) {
        Read::Note(note, stamp) => index.apply(&[(*note, stamp)], &[], &[]),
        Read::Skipped(path) => index.apply(&[], &[path], &[]),
    }
}

enum Read {
    Note(Box<Note>, Stamp),
    Skipped(String),
}

fn read_one(relative: String, file: &Path) -> Read {
    let Some((mtime, size)) = stamp_of(file) else {
        return Read::Skipped(relative);
    };
    if size as u64 > MAX_FILE {
        return Read::Skipped(relative);
    }
    match fs::read_to_string(file) {
        Ok(source) => {
            let note = parse::parse(&relative, &source);
            let stamp = Stamp {
                mtime,
                size,
                hash: note.hash.clone(),
            };
            Read::Note(Box::new(note), stamp)
        },
        Err(_) => Read::Skipped(relative),
    }
}

/// Reads and parses files on every core.
fn read_all(files: Vec<(String, PathBuf, i64, i64)>) -> Vec<Read> {
    if files.len() < 64 {
        return files
            .into_iter()
            .map(|(relative, file, ..)| read_one(relative, &file))
            .collect();
    }
    let workers = thread::available_parallelism()
        .map_or(4, |count| count.get())
        .min(16);
    let chunk = files.len().div_ceil(workers);
    let mut chunks: Vec<Vec<(String, PathBuf, i64, i64)>> = Vec::new();
    let mut files = files.into_iter().peekable();
    while files.peek().is_some() {
        chunks.push(files.by_ref().take(chunk).collect());
    }
    thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .into_iter()
                        .map(|(relative, file, ..)| read_one(relative, &file))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_markdown_outside_hidden_folders() {
        assert!(is_note("notes/A.md"));
        assert!(is_note("B.MD"));
        assert!(!is_note(".obsidian/x.md"));
        assert!(!is_note("notes/.hidden.md"));
        assert!(!is_note("raw/paper.pdf"));
    }
}
