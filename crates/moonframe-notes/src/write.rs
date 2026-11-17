//! Writing notes without losing anyone's words (VAULT.md, "Writes").
//!
//! Every write is journaled first, then written to a temporary file beside
//! its target and renamed over it, so a crash leaves either the old file or
//! the new one, and the journal says which writes to finish. A write knows
//! the text it started from: when the file changed on disk since (Obsidian,
//! a sync, git, the agent), edits that don't overlap are merged, and edits
//! that do keep both versions.

use {
    crate::{Error, text},
    serde::{Deserialize, Serialize},
    similar::{Algorithm, DiffOp, capture_diff_slices},
    std::{
        fs,
        io::Write as _,
        path::{Path, PathBuf},
    },
};

/// What a write did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum Saved {
    /// The text went to disk as given.
    Written { hash: String },
    /// The file had changed on disk; both sets of edits are in it now. The
    /// editor should take `text`.
    Merged { hash: String, text: String },
    /// The file had changed on disk where the edit did too. The edit is in
    /// the file; the other version was kept as `copy`.
    Conflict { hash: String, copy: String },
    /// The file already held this text.
    Unchanged { hash: String },
}

/// One write waiting in the journal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Pending {
    pub path: String,
    pub text: String,
}

pub(crate) struct Journal {
    folder: PathBuf,
}

impl Journal {
    pub(crate) fn new(folder: PathBuf) -> Self {
        Self { folder }
    }

    fn entry(&self, path: &str) -> PathBuf {
        self.folder
            .join(format!("{}.json", &text::digest(path)[..24]))
    }

    pub(crate) fn record(&self, pending: &Pending) -> Result<(), Error> {
        fs::create_dir_all(&self.folder)?;
        let json =
            serde_json::to_vec(pending).map_err(|error| Error::Invalid(error.to_string()))?;
        atomic_write(&self.entry(&pending.path), &json)
    }

    pub(crate) fn clear(&self, path: &str) {
        let _ = fs::remove_file(self.entry(path));
    }

    /// Writes still in the journal, from a run that stopped part way.
    pub(crate) fn pending(&self) -> Vec<Pending> {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return Vec::new();
        };
        let mut out: Vec<Pending> = entries
            .flatten()
            .filter_map(|entry| fs::read(entry.path()).ok())
            .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
            .collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

/// Writes `bytes` to `file` through a temporary file and a rename, keeping
/// the file's permissions.
pub(crate) fn atomic_write(file: &Path, bytes: &[u8]) -> Result<(), Error> {
    let folder = file
        .parent()
        .ok_or_else(|| Error::Invalid(format!("no folder for {}", file.display())))?;
    fs::create_dir_all(folder)?;
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = folder.join(format!(".{name}.{}.tmp", &text::ulid()[16..]));
    let result = (|| {
        let mut out = fs::File::create(&temporary)?;
        out.write_all(bytes)?;
        out.sync_all()?;
        if let Ok(meta) = fs::metadata(file) {
            fs::set_permissions(&temporary, meta.permissions())?;
        }
        fs::rename(&temporary, file)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    Ok(result?)
}

/// Writes `text` to `relative` under `root`, given the text the edit
/// started from (`base`; None to write whatever is there). `stamp` names a
/// conflict copy (`2026-11-10 1412`).
pub(crate) fn save(
    root: &Path,
    journal: &Journal,
    relative: &str,
    text_in: &str,
    base: Option<&str>,
    stamp: &str,
) -> Result<Saved, Error> {
    let file = root.join(relative);
    let disk = match fs::read_to_string(&file) {
        Ok(disk) => Some(disk),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if disk.as_deref() == Some(text_in) {
        return Ok(Saved::Unchanged {
            hash: text::digest(text_in),
        });
    }
    let (to_write, outcome_text, conflict) = match (&disk, base) {
        (Some(disk), Some(base)) if disk != base => match merge(base, text_in, disk) {
            Some(merged) => (merged.clone(), Some(merged), None),
            None => (text_in.to_string(), None, Some(disk.clone())),
        },
        _ => (text_in.to_string(), None, None),
    };
    let mut copy = None;
    if let Some(theirs) = conflict {
        let name = conflict_name(relative, stamp, |candidate| root.join(candidate).exists());
        write_journaled(root, journal, &name, &theirs)?;
        copy = Some(name);
    }
    write_journaled(root, journal, relative, &to_write)?;
    let hash = text::digest(&to_write);
    Ok(match (copy, outcome_text) {
        (Some(copy), _) => Saved::Conflict { hash, copy },
        (None, Some(text)) => Saved::Merged { hash, text },
        (None, None) => Saved::Written { hash },
    })
}

pub(crate) fn write_journaled(
    root: &Path,
    journal: &Journal,
    relative: &str,
    text_in: &str,
) -> Result<(), Error> {
    let pending = Pending {
        path: relative.to_string(),
        text: text_in.to_string(),
    };
    journal.record(&pending)?;
    atomic_write(&root.join(relative), text_in.as_bytes())?;
    journal.clear(relative);
    Ok(())
}

/// `Folder/Title (conflict 2026-11-10 1412).md`, numbered if that is taken.
pub(crate) fn conflict_name(relative: &str, stamp: &str, taken: impl Fn(&str) -> bool) -> String {
    let (folder, name) = match relative.rsplit_once('/') {
        Some((folder, name)) => (format!("{folder}/"), name),
        None => (String::new(), relative),
    };
    let stem = text::strip_md(name);
    let mut candidate = format!("{folder}{stem} (conflict {stamp}).md");
    let mut number = 2;
    while taken(&candidate) {
        candidate = format!("{folder}{stem} (conflict {stamp} {number}).md");
        number += 1;
    }
    candidate
}

/// A three-way merge by lines: the edits from `base` to `ours` and from
/// `base` to `theirs`, applied together. None when they change the same
/// lines differently, or both insert at the same place. Unlike git, edits
/// to neighbouring lines merge: in notes, one side appending under a line
/// the other edits is the common case.
pub fn merge(base: &str, ours: &str, theirs: &str) -> Option<String> {
    if ours == theirs {
        return Some(ours.to_string());
    }
    let base_lines: Vec<&str> = base.split_inclusive('\n').collect();
    let our_lines: Vec<&str> = ours.split_inclusive('\n').collect();
    let their_lines: Vec<&str> = theirs.split_inclusive('\n').collect();
    let mut changes: Vec<Change> = Vec::new();
    changes.extend(edits(&base_lines, &our_lines));
    changes.extend(edits(&base_lines, &their_lines));
    changes.sort_by_key(|change| (change.start, change.end));
    // Drop exact duplicates (both sides made the same edit) and refuse overlaps.
    let mut kept: Vec<Change> = Vec::new();
    for change in changes {
        if let Some(last) = kept.last() {
            if *last == change {
                continue;
            }
            if clashes(last, &change) {
                return None;
            }
        }
        kept.push(change);
    }
    let mut out = String::with_capacity(ours.len().max(theirs.len()));
    let mut at = 0;
    for change in kept {
        for line in &base_lines[at..change.start] {
            out.push_str(line);
        }
        out.push_str(&change.lines);
        at = change.end;
    }
    for line in &base_lines[at..] {
        out.push_str(line);
    }
    Some(out)
}

/// Base lines `start..end` replaced by `lines`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Change {
    start: usize,
    end: usize,
    lines: String,
}

/// Whether two edits (sorted, `a` first) cannot both apply.
fn clashes(a: &Change, b: &Change) -> bool {
    let overlap = b.start < a.end;
    let same_insert = a.start == a.end && b.start == b.end && a.start == b.start;
    // An insertion strictly inside a range the other side replaced.
    let inside = b.start == b.end && a.start < b.start && b.start < a.end;
    overlap || same_insert || inside
}

fn edits(base: &[&str], other: &[&str]) -> Vec<Change> {
    let mut out: Vec<Change> = Vec::new();
    for op in capture_diff_slices(Algorithm::Myers, base, other) {
        let change = match op {
            DiffOp::Equal { .. } => continue,
            DiffOp::Delete {
                old_index, old_len, ..
            } => Change {
                start: old_index,
                end: old_index + old_len,
                lines: String::new(),
            },
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => Change {
                start: old_index,
                end: old_index,
                lines: other[new_index..new_index + new_len].concat(),
            },
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => Change {
                start: old_index,
                end: old_index + old_len,
                lines: other[new_index..new_index + new_len].concat(),
            },
        };
        // Join with a change right before it, so one side's edit is one change.
        match out.last_mut() {
            Some(last) if last.end == change.start => {
                last.end = change.end;
                last.lines.push_str(&change.lines);
            },
            _ => out.push(change),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn merges_edits_far_apart() {
        let base = "a\nb\nc\nd\ne\n";
        let ours = "A\nb\nc\nd\ne\n";
        let theirs = "a\nb\nc\nd\nE\n";
        assert_eq!(merge(base, ours, theirs).unwrap(), "A\nb\nc\nd\nE\n");
    }

    #[test]
    fn merges_an_insert_and_an_append() {
        let base = "# T\n\none\n\nthree\n";
        let ours = "# T\n\none\ntwo\n\nthree\n";
        let theirs = "# T\n\none\n\nthree\nfour\n";
        assert_eq!(
            merge(base, ours, theirs).unwrap(),
            "# T\n\none\ntwo\n\nthree\nfour\n"
        );
    }

    #[test]
    fn refuses_edits_to_the_same_line() {
        assert_eq!(merge("a\nb\nc\n", "a\nX\nc\n", "a\nY\nc\n"), None);
        // Both sides adding at the same place.
        assert_eq!(merge("a\nc\n", "a\nX\nc\n", "a\nY\nc\n"), None);
        // One side deleting lines the other edits.
        assert_eq!(merge("a\nb\nc\nd\n", "a\nd\n", "a\nb\nC\nd\n"), None);
    }

    #[test]
    fn merges_neighbouring_lines() {
        assert_eq!(
            merge("a\nb\nc\n", "A\nb\nc\n", "a\nB\nc\n").unwrap(),
            "A\nB\nc\n"
        );
        assert_eq!(
            merge("only\n", "edited\n", "only\nappended\n").unwrap(),
            "edited\nappended\n"
        );
    }

    #[test]
    fn same_edit_on_both_sides_is_kept_once() {
        assert_eq!(
            merge("a\nb\nc\nd\n", "a\nB\nc\nd\n", "a\nB\nc\nD\n").unwrap(),
            "a\nB\nc\nD\n"
        );
        assert_eq!(merge("x\n", "y\n", "y\n").unwrap(), "y\n");
    }

    #[test]
    fn conflict_names_count_up() {
        let taken = |name: &str| name == "notes/A (conflict 2026-11-10 1412).md";
        assert_eq!(
            conflict_name("notes/A.md", "2026-11-10 1412", taken),
            "notes/A (conflict 2026-11-10 1412 2).md"
        );
        assert_eq!(
            conflict_name("B.md", "2026-11-10 1412", |_| false),
            "B (conflict 2026-11-10 1412).md"
        );
    }

    #[test]
    fn save_merges_and_keeps_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let journal = Journal::new(root.join(".commonplace/journal"));
        fs::write(root.join("N.md"), "one\ntwo\nthree\nfour\n").unwrap();
        // Someone else changed the last line since the editor opened it.
        fs::write(root.join("N.md"), "one\ntwo\nthree\nFOUR\n").unwrap();
        let saved = save(
            root,
            &journal,
            "N.md",
            "ONE\ntwo\nthree\nfour\n",
            Some("one\ntwo\nthree\nfour\n"),
            "s",
        )
        .unwrap();
        assert!(matches!(saved, Saved::Merged { .. }));
        assert_eq!(
            fs::read_to_string(root.join("N.md")).unwrap(),
            "ONE\ntwo\nthree\nFOUR\n"
        );

        let saved = save(
            root,
            &journal,
            "N.md",
            "ONE\ntwo\nmine\nFOUR\n",
            Some("one\ntwo\nthree\nfour\n"),
            "s",
        )
        .unwrap();
        let Saved::Conflict { copy, .. } = saved else {
            panic!("expected a conflict")
        };
        assert_eq!(
            fs::read_to_string(root.join("N.md")).unwrap(),
            "ONE\ntwo\nmine\nFOUR\n"
        );
        assert_eq!(
            fs::read_to_string(root.join(&copy)).unwrap(),
            "ONE\ntwo\nthree\nFOUR\n"
        );
        assert!(journal.pending().is_empty());
    }
}
