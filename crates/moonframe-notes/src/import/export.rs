//! Notes.app's own Markdown export (File > Export as > Markdown, macOS 26),
//! joined with what Notes.app says of each note. The export has the better
//! bodies and every attachment, but is flat and dated the day it was made;
//! Notes.app knows each note's folder, dates and id. Each exported file is
//! matched to its note by title (Unicode-normalized, punctuation ignored,
//! since file systems change both), and among notes sharing a title by how
//! much of their text the two have in common.

use {
    super::{SourceNote, html},
    crate::Error,
    std::{
        collections::HashSet,
        fs,
        path::{Path, PathBuf},
    },
    unicode_normalization::UnicodeNormalization,
};

/// One file of the export.
struct Exported {
    file: String,
    title: String,
    text: String,
}

/// How the export and Notes.app came together.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Joined {
    /// Notes whose body came from the export.
    pub matched: usize,
    /// Notes Notes.app has that the export lacks: converted from Notes.app.
    pub notes_only: usize,
    /// Exported files no note matched: imported as they are, undated.
    pub export_only: Vec<String>,
    /// Notes locked in Notes.app whose (empty) exported file was set aside.
    pub locked: usize,
    /// Empty exported files that matched no note, left out.
    pub empty: usize,
}

/// A title as matching sees it: NFC, lowercase, letters and digits only.
pub(crate) fn match_key(title: &str) -> String {
    title
        .nfc()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// The note's title as the export writes it: its `# Heading` line, else
/// its first line with words.
fn title_of(text: &str) -> String {
    // An empty heading (`#` and spaces) is not a title: Notes writes one
    // above some notes.
    text.lines()
        .map(|line| line.trim().trim_start_matches('#').trim())
        .find(|line| !line.is_empty())
        .map(str::to_string)
        .unwrap_or_default()
}

fn read(folder: &Path) -> Result<Vec<Exported>, Error> {
    let mut out = Vec::new();
    for entry in fs::read_dir(folder)?.flatten() {
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            || !path.is_file()
        {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|error| Error::Invalid(format!("{}: {error}", path.display())))?;
        let file = path
            .file_name()
            .map(|name| name.to_string_lossy().nfc().collect::<String>())
            .unwrap_or_default();
        out.push(Exported {
            title: title_of(&text),
            file,
            text,
        });
    }
    out.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(out)
}

/// The words of `text`, for telling apart notes that share a title.
fn words(text: &str) -> HashSet<String> {
    text.nfc()
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() > 2)
        .map(str::to_lowercase)
        .collect()
}

fn overlap(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        a.intersection(b).count() as f64 / union as f64
    }
}

const MEDIA: &[&str] = &[
    "jpeg", "jpg", "png", "gif", "tiff", "tif", "webp", "heic", "mov", "mp4", "m4v", "svg",
];

/// The export's text with its `Attachments/…` links pointed into the vault
/// (`raw/assets/<source>/…`, as embeds for pictures and films, links for
/// the rest), and the files to copy there.
fn relink(text: &str, folder: &Path, source: &str) -> (String, Vec<(PathBuf, String)>) {
    let mut out = String::with_capacity(text.len());
    let mut files = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("](Attachments/") {
        // The link's `[`, and a `!` before it for an embed.
        let open = rest[..at].rfind('[');
        let close = rest[at..].find(')').map(|end| at + end);
        let (Some(open), Some(close)) = (open, close) else {
            out.push_str(&rest[..at + 2]);
            rest = &rest[at + 2..];
            continue;
        };
        let embed = open > 0 && rest.as_bytes()[open - 1] == b'!';
        let start = if embed {
            open - 1
        } else {
            open
        };
        let label = &rest[open + 1..at];
        let name = percent_decode(&rest[at + "](Attachments/".len()..close]);
        // One plain file name in Attachments, nothing that climbs out of it
        // (`../`) or into a folder: anything else stays as written.
        if !plain_file_name(&name) {
            out.push_str(&rest[..close + 1]);
            rest = &rest[close + 1..];
            continue;
        }
        let relative = format!("raw/assets/{source}/{name}");
        let extension = name.rsplit('.').next().unwrap_or("").to_lowercase();
        out.push_str(&rest[..start]);
        if MEDIA.contains(&extension.as_str()) {
            out.push_str(&format!("![[{relative}]]"));
        } else {
            out.push_str(&format!(
                "[[{relative}|{}]]",
                label.replace(['[', ']', '|'], " ").trim()
            ));
        }
        files.push((folder.join("Attachments").join(&name), relative));
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    (out, files)
}

/// Whether `name` is a single, ordinary file name.
fn plain_file_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(parts.next(), Some(std::path::Component::Normal(_)))
        && parts.next().is_none()
        && !name.contains(['/', '\\'])
        && !name.starts_with('.')
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(byte) = text
                .get(index + 1..index + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Shortest title, in letters and digits, matched by its start alone.
const PREFIX: usize = 12;

/// `notes` (from Notes.app) with the bodies and attachments of the export
/// in `folder` wherever a file matches, plus the export's files that
/// matched none, as notes of their own in `unmatched_folder`.
///
/// A note matches a file by title, in three passes: the file's first line,
/// then its name, then either one starting the other (the export cuts long
/// names with `…`, and a title may run over two lines). Among several
/// files, the one sharing most words with the note wins. A note locked in
/// Notes.app takes its file (which the export leaves empty) and stays out.
pub(crate) fn join(
    mut notes: Vec<SourceNote>,
    folder: &Path,
    source: &str,
    unmatched_folder: &str,
) -> Result<(Vec<SourceNote>, Joined), Error> {
    let exported = read(folder)?;
    let heading_keys: Vec<String> = exported.iter().map(|file| match_key(&file.title)).collect();
    let name_keys: Vec<String> = exported
        .iter()
        .map(|file| match_key(file.file.trim_end_matches(".md").trim_end_matches('…')))
        .collect();
    let mut used = vec![false; exported.len()];
    let mut chosen: Vec<Option<usize>> = vec![None; notes.len()];
    let samples: Vec<HashSet<String>> = notes
        .iter()
        .map(|note| match &note.plain {
            Some(plain) => words(plain),
            None => words(&html::to_markdown(note.html.as_deref().unwrap_or("")).0),
        })
        .collect();
    let exported_words: Vec<HashSet<String>> =
        exported.iter().map(|file| words(&file.text)).collect();
    let passes: [&dyn Fn(&str, usize) -> bool; 3] = [
        &|key, at| !key.is_empty() && heading_keys[at] == key,
        &|key, at| !key.is_empty() && name_keys[at] == key,
        &|key, at| {
            let starts = |long: &str, short: &str| {
                short.chars().count() >= PREFIX && long.starts_with(short)
            };
            [&heading_keys[at], &name_keys[at]]
                .iter()
                .any(|other| starts(key, other) || starts(other, key))
        },
    ];
    for matches in passes {
        // Notes with fewest candidates first, so shared titles choose among what is left.
        let mut open: Vec<(usize, Vec<usize>)> = (0..notes.len())
            .filter(|&index| chosen[index].is_none())
            .map(|index| {
                let key = match_key(&notes[index].title);
                let candidates: Vec<usize> = (0..exported.len())
                    .filter(|&at| !used[at] && matches(&key, at))
                    .collect();
                (index, candidates)
            })
            .filter(|(_, candidates)| !candidates.is_empty())
            .collect();
        open.sort_by_key(|(_, candidates)| candidates.len());
        for (index, candidates) in open {
            let best = candidates
                .into_iter()
                .filter(|&at| !used[at])
                .max_by(|&a, &b| {
                    overlap(&samples[index], &exported_words[a])
                        .partial_cmp(&overlap(&samples[index], &exported_words[b]))
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(b.cmp(&a))
                });
            if let Some(at) = best {
                used[at] = true;
                chosen[index] = Some(at);
            }
        }
    }
    let mut joined = Joined::default();
    for (index, at) in chosen.into_iter().enumerate() {
        let note = &mut notes[index];
        match at {
            // Locked in Notes.app: its file is empty, and it stays out.
            Some(_) if note.locked => joined.locked += 1,
            Some(at) => {
                let (text, files) = relink(&exported[at].text, folder, source);
                note.markdown = Some(text);
                note.attachments = files;
                joined.matched += 1;
            },
            None => joined.notes_only += 1,
        }
    }
    for (at, file) in exported.iter().enumerate() {
        if used[at] {
            continue;
        }
        if file.text.trim().is_empty() {
            joined.empty += 1;
            continue;
        }
        let (text, files) = relink(&file.text, folder, source);
        joined.export_only.push(file.file.clone());
        notes.push(SourceNote {
            id: format!("export:{}", file.file),
            title: if file.title.is_empty() {
                file.file.trim_end_matches(".md").to_string()
            } else {
                file.title.clone()
            },
            folder: vec![unmatched_folder.to_string()],
            html: None,
            plain: None,
            markdown: Some(text),
            attachments: files,
            created: None,
            modified: None,
            locked: false,
        });
    }
    Ok((notes, joined))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn note(id: &str, title: &str, html: &str) -> SourceNote {
        SourceNote {
            id: id.to_string(),
            title: title.to_string(),
            folder: vec!["Travel".to_string()],
            html: Some(html.to_string()),
            plain: None,
            markdown: None,
            attachments: Vec::new(),
            created: Some("2019-05-01T10:00:00.000Z".to_string()),
            modified: Some("2020-01-01T10:00:00.000Z".to_string()),
            locked: false,
        }
    }

    #[test]
    fn titles_match_through_normalization_and_punctuation() {
        // The file system's decomposed é, and `:` dropped from file names.
        assert_eq!(
            match_key("Re\u{301}sumé: 2025 / plan?"),
            match_key("Résumé 2025 plan")
        );
        assert_ne!(match_key("Plan A"), match_key("Plan B"));
    }

    #[test]
    fn links_to_attachments_point_into_the_vault() {
        let (text, files) = relink(
            "See ![Image](Attachments/AB-12.jpeg) and [invoice.pdf](Attachments/CD%2034.pdf).",
            Path::new("/x"),
            "apple-notes",
        );
        assert_eq!(
            text,
            "See ![[raw/assets/apple-notes/AB-12.jpeg]] and [[raw/assets/apple-notes/CD 34.pdf|invoice.pdf]]."
        );
        assert_eq!(
            files[1],
            (
                PathBuf::from("/x/Attachments/CD 34.pdf"),
                "raw/assets/apple-notes/CD 34.pdf".to_string()
            )
        );
    }

    #[test]
    fn links_that_climb_out_of_attachments_are_left_alone() {
        let text = "![x](Attachments/../../secret.png) ![y](Attachments/a%2F..%2Fb.png) ![z](Attachments/.hidden)";
        let (out, files) = relink(text, Path::new("/x"), "apple-notes");
        assert_eq!(out, text);
        assert!(files.is_empty());
    }

    #[test]
    fn long_titles_match_by_their_start_and_locked_notes_stay_out() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path();
        fs::write(
            folder.join("Ajouter un feature flag sur la recurrence….md"),
            "Ajouter un feature flag sur la recurrence de paiement\n\nPlus.\n",
        )
        .unwrap();
        fs::write(
            folder.join("Boring Local SEO (@boringlocalseo).md"),
            "# Boring Local SEO (@boringlocalseo)\n\n854 likes\n",
        )
        .unwrap();
        fs::write(folder.join("Secret.md"), "").unwrap();
        fs::write(folder.join("Empty.md"), "").unwrap();
        let mut locked = note("3", "Secret", "");
        locked.locked = true;
        locked.html = None;
        let notes = vec![
            note(
                "1",
                "Ajouter un feature flag sur la recurrence de paiement sur le portail",
                "<div>x</div>",
            ),
            note(
                "2",
                "Boring Local SEO (@boringlocalseo)\n854 likes · 24 replies",
                "<div>x</div>",
            ),
            locked,
        ];
        let (notes, joined) = join(notes, folder, "apple-notes", "From the export").unwrap();
        assert_eq!(
            (
                joined.matched,
                joined.notes_only,
                joined.locked,
                joined.empty
            ),
            (2, 0, 1, 1)
        );
        assert!(joined.export_only.is_empty(), "{:?}", joined.export_only);
        assert!(notes[2].markdown.is_none());
        assert_eq!(notes.len(), 3);
    }

    #[test]
    fn joins_by_title_then_by_shared_words() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path();
        fs::create_dir_all(folder.join("Attachments")).unwrap();
        fs::write(
            folder.join("Lisbon trip.md"),
            "# Lisbon trip\n\nTram 28 ![Image](Attachments/P1.jpeg)\n",
        )
        .unwrap();
        fs::write(
            folder.join("New Note.md"),
            "# New Note\n\nbuy milk eggs bread\n",
        )
        .unwrap();
        fs::write(
            folder.join("New Note 2.md"),
            "# New Note\n\ncall the plumber tomorrow\n",
        )
        .unwrap();
        fs::write(folder.join("Stray.md"), "Only in the export\n").unwrap();
        let notes = vec![
            note(
                "1",
                "Lisbon trip",
                "<div>Lisbon trip</div><div>Tram 28</div>",
            ),
            note(
                "2",
                "New Note",
                "<div>New Note</div><div>call the plumber tomorrow</div>",
            ),
            note(
                "3",
                "New Note",
                "<div>New Note</div><div>buy milk, eggs and bread</div>",
            ),
            note("4", "Only in Notes", "<div>Only in Notes</div>"),
        ];
        let (notes, joined) = join(notes, folder, "apple-notes", "From the export").unwrap();
        assert_eq!((joined.matched, joined.notes_only), (3, 1));
        assert_eq!(joined.export_only, vec!["Stray.md"]);
        assert!(
            notes[0]
                .markdown
                .as_deref()
                .unwrap()
                .contains("![[raw/assets/apple-notes/P1.jpeg]]")
        );
        assert!(notes[1].markdown.as_deref().unwrap().contains("plumber"));
        assert!(notes[2].markdown.as_deref().unwrap().contains("milk"));
        assert!(notes[3].markdown.is_none());
        let stray = &notes[4];
        assert_eq!(
            (stray.title.as_str(), stray.folder[0].as_str()),
            ("Only in the export", "From the export")
        );
    }
}
