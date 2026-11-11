//! Bringing notes in from other apps. Each source reads its notes into
//! [`SourceNote`]s; [`write`] puts them in the vault as markdown files
//! with their dates in frontmatter, saves their pictures under
//! `raw/assets/<source>/`, and remembers what it wrote in
//! `.commonplace/imports/<source>.json`, so importing again updates what
//! changed at the source and never overwrites a note edited since.

pub(crate) mod apple_notes;
pub(crate) mod export;
mod html;

use {
    crate::{Error, STATE, text, write::atomic_write},
    base64::Engine as _,
    serde::{Deserialize, Serialize},
    std::{
        collections::{BTreeMap, HashSet},
        fs,
        path::{Path, PathBuf},
    },
};

/// One note as another app has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceNote {
    /// The source's own id, stable across imports.
    pub id: String,
    pub title: String,
    /// Folders from the source's top, outermost first.
    pub folder: Vec<String>,
    /// The body as HTML; None when it could not be read (a locked note).
    pub html: Option<String>,
    /// A sample of the note's plain text, to match it by when its body
    /// comes from elsewhere.
    pub plain: Option<String>,
    /// The body as markdown, when the source exported it so (Notes.app's
    /// Markdown export): used instead of `html`.
    pub markdown: Option<String>,
    /// Files the markdown links to: where they are, and the vault path they
    /// go to.
    pub attachments: Vec<(PathBuf, String)>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub locked: bool,
}

/// What an import did, or would do in a dry run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    pub source: String,
    pub dry_run: bool,
    pub read: usize,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: usize,
    /// Changed at the source and in the vault since the last import: left as they are.
    pub kept: Vec<String>,
    /// Notes not imported, with why.
    pub skipped: Vec<(String, String)>,
    /// Pictures and other attachments brought in.
    pub pictures: usize,
    /// How a Markdown export and Notes.app came together, when one was given.
    pub joined: Option<export::Joined>,
}

/// How far an import is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "phase", rename_all = "lowercase")]
pub enum Progress {
    /// Notes read from Notes.app so far, of about how many, and the folder
    /// just read.
    Reading {
        done: usize,
        total: usize,
        folder: String,
    },
    /// Matching the export's files to the notes.
    Matching,
    /// Notes written (or, in a dry run, checked) so far.
    Writing { done: usize, total: usize },
}

/// `progress` as Notes.app's reader wants to hear: notes read, of how
/// many, and the folder.
fn reading<P: FnMut(Progress)>(progress: &mut P) -> impl FnMut(usize, usize, &str) + '_ {
    move |done, total, folder| {
        progress(Progress::Reading {
            done,
            total,
            folder: folder.to_string(),
        })
    }
}

/// Imports Notes.app into the vault at `root` (folder `into`): every note
/// read from Notes.app, with the bodies and attachments of its Markdown
/// export in `export` when given (see `export`). `progress` hears how far
/// it is, phase by phase.
pub fn apple_notes(
    root: &Path,
    into: &str,
    export: Option<&Path>,
    dry_run: bool,
    mut progress: impl FnMut(Progress),
) -> Result<ImportReport, Error> {
    let scratch = std::env::temp_dir().join(format!("commonplace-notes-export-{}", text::ulid()));
    let mut joined = None;
    let mut notes;
    if let Some(folder) = export {
        // The export has the bodies: read only enough of each note to match it.
        notes = apple_notes::export(&scratch, apple_notes::Mode::Sample, reading(&mut progress))?;
        progress(Progress::Matching);
        let (all, how) = export::join(notes, folder, "apple-notes", "From the export")?;
        notes = all;
        // Notes the export lacks: their bodies from Notes.app, one at a time.
        let missing: Vec<String> = notes
            .iter()
            .filter(|note| {
                note.markdown.is_none() && !note.locked && !note.id.starts_with("export:")
            })
            .map(|note| note.id.clone())
            .collect();
        let mut bodies = apple_notes::bodies(&scratch, &missing, reading(&mut progress))?;
        for note in &mut notes {
            if let Some(html) = bodies.remove(&note.id) {
                note.html = Some(html);
            }
        }
        joined = Some(how);
    } else {
        notes = apple_notes::export(&scratch, apple_notes::Mode::Full, reading(&mut progress))?;
    }
    let mut report = write(root, "apple-notes", into, &notes, dry_run, |done, total| {
        progress(Progress::Writing { done, total });
    })?;
    report.joined = joined;
    Ok(report)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Manifest {
    notes: BTreeMap<String, Imported>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Imported {
    path: String,
    /// The hash of what was written, to tell whether the note was edited since.
    hash: String,
    modified: Option<String>,
}

/// Where imported notes go: `into` (a vault folder) and the source's
/// folders under it. A source's default folder ("Notes") adds none.
fn place(into: &str, folder: &[String], title: &str, taken: &mut HashSet<String>) -> String {
    // `into` is a vault folder: each part made safe, so none can climb out.
    let mut parts: Vec<String> = into
        .split(['/', '\\'])
        .filter(|part| !part.trim().is_empty() && *part != "." && *part != "..")
        .map(|part| safe_name(part, "Imported"))
        .collect();
    let defaults = ["Notes", "Notes.app"];
    for (index, name) in folder
        .iter()
        .filter(|name| !name.trim().is_empty())
        .enumerate()
    {
        if index == 0 && folder.len() == 1 && defaults.contains(&name.as_str()) {
            continue;
        }
        parts.push(safe_name(name, "Folder"));
    }
    let base = parts.join("/");
    let stem = safe_name(title, "Untitled");
    let mut number = 1;
    loop {
        let name = if number == 1 {
            format!("{stem}.md")
        } else {
            format!("{stem} {number}.md")
        };
        let path = if base.is_empty() {
            name
        } else {
            format!("{base}/{name}")
        };
        if taken.insert(path.to_lowercase()) {
            return path;
        }
        number += 1;
    }
}

/// Whether `path` (a vault path) stays inside the vault: relative, no `..`
/// or `.` parts, no hidden ones.
fn inside(path: &str) -> bool {
    !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != ".." && !part.starts_with('.'))
}

/// `name` as a file name every system and Obsidian accept: no `/ \ : * ? "
/// < > |`, no `# ^ [ ]` (they mean something in links), no leading dot.
pub(crate) fn safe_name(name: &str, fallback: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(
                c,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | '^' | '[' | ']'
            ) || c.is_control()
            {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_start_matches('.').trim();
    let cleaned: String = cleaned.chars().take(120).collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

/// One note to write: which, where, and whether it replaces an earlier import.
struct Job {
    index: usize,
    path: String,
    update: bool,
}

/// Converts and writes each job's note on every core, each file dated as
/// the note was last changed. Gives, per job in order, the hash of what was
/// written and how many attachments came with it, or None when the note
/// could not be converted. `progress` hears how many are done.
fn run_jobs(
    jobs: &[Job],
    notes: &[SourceNote],
    source: &str,
    root: &Path,
    dry_run: bool,
    mut progress: impl FnMut(usize),
) -> Result<Vec<Option<(String, usize)>>, Error> {
    let workers = std::thread::available_parallelism()
        .map_or(4, |count| count.get())
        .clamp(1, 16);
    let chunk = jobs.len().div_ceil(workers).max(1);
    let (sender, receiver) = std::sync::mpsc::channel::<usize>();
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .chunks(chunk)
            .map(|part| {
                let sender = sender.clone();
                scope.spawn(move || -> Result<Vec<Option<(String, usize)>>, Error> {
                    let mut out = Vec::with_capacity(part.len());
                    for job in part {
                        let note = &notes[job.index];
                        let mut pictures = 0;
                        let outcome = match guarded(note, source, root, dry_run, &mut pictures)? {
                            Some(rendered) => {
                                if !dry_run {
                                    let file = root.join(&job.path);
                                    atomic_write(&file, rendered.as_bytes())?;
                                    if let Some(when) =
                                        note.modified.as_deref().and_then(text::parse_time)
                                    {
                                        // Finder, Obsidian and the index sort by it.
                                        let _ = fs::File::options()
                                            .write(true)
                                            .open(&file)
                                            .and_then(|file| file.set_modified(when));
                                    }
                                }
                                Some((text::digest(&rendered), pictures))
                            },
                            None => None,
                        };
                        out.push(outcome);
                        let _ = sender.send(1);
                    }
                    Ok(out)
                })
            })
            .collect();
        drop(sender);
        let mut finished = 0;
        for step in receiver {
            finished += step;
            progress(finished);
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err(Error::Invalid("an import worker stopped".to_string())))
            })
            .collect::<Vec<_>>()
    });
    let mut all = Vec::with_capacity(jobs.len());
    for part in results {
        all.extend(part?);
    }
    Ok(all)
}

/// [`render`], with a note whose conversion fails (a bug, never the
/// user's doing) left out rather than stopping the whole import.
fn guarded(
    note: &SourceNote,
    source: &str,
    root: &Path,
    dry_run: bool,
    pictures: &mut usize,
) -> Result<Option<String>, Error> {
    let mut counted = *pictures;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        render(note, source, root, dry_run, &mut counted)
    }));
    match result {
        Ok(rendered) => {
            *pictures = counted;
            rendered.map(Some)
        },
        Err(_) => Ok(None),
    }
}

/// The note as the markdown file it becomes, with its pictures saved (or,
/// in a dry run, counted).
fn render(
    note: &SourceNote,
    source: &str,
    root: &Path,
    dry_run: bool,
    pictures: &mut usize,
) -> Result<String, Error> {
    if let Some(markdown) = &note.markdown {
        for (from, to) in &note.attachments {
            if !dry_run {
                let file = root.join(to);
                if !file.exists() {
                    match fs::read(from) {
                        Ok(bytes) => atomic_write(&file, &bytes)?,
                        // A link to a file the export lacks stays a dangling link.
                        Err(_) => continue,
                    }
                }
            }
            *pictures += 1;
        }
        return Ok(frontmatter(note, source) + &without_title(markdown, &note.title));
    }
    let (body, found) = html::to_markdown(note.html.as_deref().unwrap_or(""));
    let mut body = body;
    for picture in found {
        let replacement = match picture
            .src
            .strip_prefix("data:")
            .and_then(|rest| rest.split_once(";base64,"))
        {
            Some((mime, data)) => {
                let extension = match mime {
                    "image/jpeg" | "image/jpg" => "jpg",
                    "image/gif" => "gif",
                    "image/heic" => "heic",
                    "image/tiff" => "tiff",
                    "image/webp" => "webp",
                    "application/pdf" => "pdf",
                    _ => "png",
                };
                match base64::engine::general_purpose::STANDARD.decode(data.trim()) {
                    Ok(bytes) => {
                        let name = format!("{}.{extension}", &text::digest_bytes(&bytes)[..16]);
                        let relative = format!("raw/assets/{source}/{name}");
                        if !dry_run {
                            let file = root.join(&relative);
                            if !file.exists() {
                                atomic_write(&file, &bytes)?;
                            }
                        }
                        *pictures += 1;
                        format!("![[{relative}]]")
                    },
                    Err(_) => String::new(),
                }
            },
            None => format!("![]({})", picture.src.replace(' ', "%20")),
        };
        body = body.replace(&picture.placeholder, &replacement);
    }
    Ok(frontmatter(note, source) + &without_title(&body, &note.title))
}

/// `body` without its first line when that line is the note's title (as
/// Notes.app writes it), since the file name carries it.
fn without_title(body: &str, title: &str) -> String {
    // Empty headings (`#` and spaces) above the title go too.
    let body = body.trim_start_matches(['\n', '\r']);
    let mut rest = body;
    while let Some((line, after)) = rest.split_once('\n') {
        if line.trim().trim_start_matches('#').trim().is_empty() && line.trim().starts_with('#') {
            rest = after;
        } else {
            break;
        }
    }
    let body = rest;
    let mut lines = body.lines();
    let first = lines.next().unwrap_or("");
    let plain = first
        .trim_start_matches('#')
        .trim()
        .trim_matches('*')
        .trim();
    let body = if !plain.is_empty() && export::match_key(plain) == export::match_key(title) {
        lines
            .collect::<Vec<_>>()
            .join("\n")
            .trim_start_matches('\n')
            .to_string()
            + "\n"
    } else {
        body.to_string()
    };
    let body = body.trim_matches('\n');
    if body.is_empty() {
        String::new()
    } else {
        format!("{body}\n")
    }
}

/// The frontmatter an imported note starts with: a new id, its dates, and
/// where it came from.
fn frontmatter(note: &SourceNote, source: &str) -> String {
    let mut front = String::from("---\n");
    front.push_str(&format!("id: {}\n", text::ulid()));
    if let Some(created) = &note.created {
        front.push_str(&format!("created: {created}\n"));
    }
    if let Some(modified) = &note.modified {
        front.push_str(&format!("updated: {modified}\n"));
    }
    front.push_str(&format!("source: {source}\n"));
    front.push_str(&format!(
        "{source}_id: {}\n",
        serde_json::to_string(&note.id).unwrap_or_default()
    ));
    front.push_str("---\n");
    front
}

/// Writes `notes` from `source` (`apple-notes`) into the vault at `root`,
/// under the vault folder `into`. `progress` hears (done, total).
pub fn write(
    root: &Path,
    source: &str,
    into: &str,
    notes: &[SourceNote],
    dry_run: bool,
    mut progress: impl FnMut(usize, usize),
) -> Result<ImportReport, Error> {
    let manifest_file = root
        .join(STATE)
        .join("imports")
        .join(format!("{source}.json"));
    let mut manifest: Manifest = fs::read(&manifest_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut report = ImportReport {
        source: source.to_string(),
        dry_run,
        read: notes.len(),
        ..ImportReport::default()
    };
    // Paths taken: every file in the vault, so imports never overwrite one.
    let mut taken: HashSet<String> = HashSet::new();
    for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
        if let Some(relative) = crate::scan::relative(root, entry.path()) {
            taken.insert(relative.to_lowercase());
        }
    }
    for known in manifest.notes.values() {
        taken.insert(known.path.to_lowercase());
    }
    // Every file written stays in the vault: a note's place, or an
    // attachment under raw/assets/.
    for note in notes {
        for (_, to) in &note.attachments {
            if !inside(to) || !to.starts_with("raw/assets/") {
                return Err(Error::Invalid(format!(
                    "an attachment would go outside the vault's assets: {to}"
                )));
            }
        }
    }
    // 1. What to do with each note, in order (places must stay unique).
    let mut jobs: Vec<Job> = Vec::new();
    for (index, note) in notes.iter().enumerate() {
        if note.markdown.is_none() && (note.locked || note.html.is_none()) {
            report.skipped.push((
                note.title.clone(),
                if note.locked {
                    "locked with a password".to_string()
                } else {
                    "its text could not be read".to_string()
                },
            ));
            continue;
        }
        match manifest.notes.get(&note.id).cloned() {
            Some(known) => {
                if known.modified == note.modified && root.join(&known.path).exists() {
                    report.unchanged += 1;
                    continue;
                }
                let current = fs::read_to_string(root.join(&known.path)).ok();
                if current
                    .as_deref()
                    .is_some_and(|current| text::digest(current) != known.hash)
                {
                    report.kept.push(known.path.clone());
                    continue;
                }
                jobs.push(Job {
                    index,
                    path: known.path,
                    update: true,
                });
            },
            None => {
                let path = place(into, &note.folder, &note.title, &mut taken);
                jobs.push(Job {
                    index,
                    path,
                    update: false,
                });
            },
        }
    }
    // 2. Converted and written on every core.
    let total = jobs.len();
    let done = run_jobs(&jobs, notes, source, root, dry_run, |finished| {
        progress(finished, total)
    })?;
    // 3. Recorded in order.
    for (job, outcome) in jobs.iter().zip(done) {
        let note = &notes[job.index];
        let Some((hash, pictures)) = outcome else {
            report
                .skipped
                .push((note.title.clone(), "could not be converted".to_string()));
            continue;
        };
        report.pictures += pictures;
        manifest.notes.insert(note.id.clone(), Imported {
            path: job.path.clone(),
            hash,
            modified: note.modified.clone(),
        });
        if job.update {
            report.updated.push(job.path.clone());
        } else {
            report.created.push(job.path.clone());
        }
    }
    progress(total, total);
    if !dry_run {
        let json = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        atomic_write(&manifest_file, &json)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn note(id: &str, title: &str, folder: &[&str], html: &str, modified: &str) -> SourceNote {
        SourceNote {
            id: id.to_string(),
            title: title.to_string(),
            folder: folder.iter().map(|name| name.to_string()).collect(),
            html: Some(html.to_string()),
            plain: None,
            markdown: None,
            attachments: Vec::new(),
            created: Some("2024-03-01T10:00:00.000Z".to_string()),
            modified: Some(modified.to_string()),
            locked: false,
        }
    }

    #[test]
    fn imports_stay_in_the_vault() {
        let mut taken = HashSet::new();
        assert_eq!(place("../../etc", &[], "x", &mut taken), "etc/x.md");
        assert_eq!(
            place("/abs/./notes", &[], "y", &mut taken),
            "abs/notes/y.md"
        );
        assert_eq!(
            place(
                "notes",
                &["..".to_string(), "a/../b".to_string()],
                "z",
                &mut taken
            ),
            "notes/Folder/a .. b/z.md"
        );
        assert!(inside("raw/assets/apple-notes/p.png"));
        assert!(!inside("raw/assets/../../x") && !inside("/x") && !inside("raw/.git/x"));
    }

    #[test]
    fn names_are_safe_and_unique() {
        assert_eq!(safe_name("a/b: c? #d [e]", "x"), "a b c d e");
        assert_eq!(safe_name("..hidden", "x"), "hidden");
        assert_eq!(safe_name("   ", "Untitled"), "Untitled");
        let mut taken = HashSet::new();
        assert_eq!(
            place("notes", &["Notes".to_string()], "Idea", &mut taken),
            "notes/Idea.md"
        );
        assert_eq!(
            place("notes", &["Notes".to_string()], "idea", &mut taken),
            "notes/idea 2.md"
        );
        assert_eq!(
            place(
                "notes",
                &["Work".to_string(), "2024".to_string()],
                "Plan",
                &mut taken
            ),
            "notes/Work/2024/Plan.md"
        );
    }

    #[test]
    fn imports_then_updates_only_what_was_not_edited() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let png = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG fake");
        let first = vec![
            note(
                "x-coredata://1",
                "Groceries",
                &["Notes"],
                "<div>Groceries</div><div>milk</div>",
                "m1",
            ),
            note(
                "x-coredata://2",
                "Trip",
                &["Travel"],
                &format!(
                    "<div><h1>Trip</h1></div><div>Lisbon <img src=\"data:image/png;base64,{png}\"></div>"
                ),
                "m1",
            ),
            SourceNote {
                locked: true,
                html: None,
                ..note("x-coredata://3", "Secret", &["Notes"], "", "m1")
            },
        ];
        let report = write(root, "apple-notes", "notes", &first, false, |_, _| {}).unwrap();
        assert_eq!(report.created, vec![
            "notes/Groceries.md",
            "notes/Travel/Trip.md"
        ]);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.pictures, 1);
        let groceries = fs::read_to_string(root.join("notes/Groceries.md")).unwrap();
        assert!(groceries.contains("source: apple-notes\n"));
        assert!(groceries.ends_with("---\nmilk\n"), "{groceries}");
        let trip = fs::read_to_string(root.join("notes/Travel/Trip.md")).unwrap();
        assert!(trip.contains("Lisbon ![[raw/assets/apple-notes/"), "{trip}");

        // Again, unchanged: nothing written.
        let again = write(root, "apple-notes", "notes", &first, false, |_, _| {}).unwrap();
        assert_eq!(
            (again.created.len(), again.updated.len(), again.unchanged),
            (0, 0, 2)
        );

        // Both changed in Notes; Groceries was also edited in the vault.
        fs::write(root.join("notes/Groceries.md"), "mine now\n").unwrap();
        let changed = vec![
            note(
                "x-coredata://1",
                "Groceries",
                &["Notes"],
                "<div>Groceries</div><div>milk, eggs</div>",
                "m2",
            ),
            note(
                "x-coredata://2",
                "Trip",
                &["Travel"],
                "<div>Trip</div><div>Porto</div>",
                "m2",
            ),
        ];
        let report = write(root, "apple-notes", "notes", &changed, false, |_, _| {}).unwrap();
        assert_eq!(report.kept, vec!["notes/Groceries.md"]);
        assert_eq!(report.updated, vec!["notes/Travel/Trip.md"]);
        assert_eq!(
            fs::read_to_string(root.join("notes/Groceries.md")).unwrap(),
            "mine now\n"
        );
        assert!(
            fs::read_to_string(root.join("notes/Travel/Trip.md"))
                .unwrap()
                .ends_with("Porto\n")
        );
    }

    #[test]
    fn a_dry_run_writes_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let report = write(
            temp.path(),
            "apple-notes",
            "notes",
            &[note("1", "A", &["Notes"], "<div>A</div>", "m")],
            true,
            |_, _| {},
        )
        .unwrap();
        assert_eq!(report.created, vec!["notes/A.md"]);
        assert!(!temp.path().join("notes").exists());
        assert!(!temp.path().join(STATE).exists());
    }
}
