//! The engine on a real vault: `tests/fixtures/vault`, a small vault of
//! the edge cases Obsidian vaults have (aliases, case, duplicate names,
//! code and comments that hide links, unicode names, hidden folders),
//! copied to a temporary folder for each test.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use {
    moonframe_notes::{Order, Saved, Vault, parse},
    std::{fs, path::Path},
    tempfile::TempDir,
};

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The index of the vault in `temp`, kept beside it so tests share nothing.
fn index_of(temp: &TempDir) -> std::path::PathBuf {
    temp.path().with_extension("index.sqlite")
}

fn vault() -> (TempDir, Vault) {
    let temp = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault"),
        temp.path(),
    );
    let mut vault = Vault::open_with_index(temp.path(), &index_of(&temp)).unwrap();
    vault.scan().unwrap();
    (temp, vault)
}

fn paths<T>(rows: &[T], path: impl Fn(&T) -> &str) -> Vec<String> {
    let mut out: Vec<String> = rows.iter().map(|row| path(row).to_string()).collect();
    out.sort();
    out
}

const SPACED: &str = "notes/Spaced repetition.md";

#[test]
fn scans_notes_but_not_hidden_files() {
    let (temp, mut vault) = vault();
    assert_eq!(vault.index().count("").unwrap(), 13);
    assert_eq!(vault.index().count("wiki").unwrap(), 3);
    // Nothing changed: a second scan reads nothing.
    let report = vault.scan().unwrap();
    assert_eq!((report.files, report.parsed, report.removed), (13, 0, 0));
    assert!(index_of(&temp).exists());
    assert!(!temp.path().join(".commonplace/index.sqlite").exists());
    let cached = moonframe_notes::default_index(temp.path());
    assert!(cached.ends_with("index.sqlite") && cached.to_string_lossy().contains("commonplace"));
}

#[test]
fn parses_obsidian_syntax_and_ignores_code_and_comments() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/vault")
            .join(SPACED),
    )
    .unwrap();
    let note = parse(SPACED, &source);
    assert_eq!(note.title, "Spaced repetition");
    assert_eq!(note.aliases, vec!["SRS"]);
    assert_eq!(note.kind.as_deref(), Some("concept"));
    assert_eq!(note.tags, vec!["learning", "memory", "learning/methods"]);
    let targets: Vec<&str> = note.links.iter().map(|link| link.target.as_str()).collect();
    assert_eq!(targets, vec![
        "Make It Stick",
        "Forgetting curve",
        "Desirable difficulties",
        "",
        "raw/assets/curve.png",
        "Anki workflow.md"
    ]);
    let headings: Vec<&str> = note
        .headings
        .iter()
        .map(|heading| heading.text.as_str())
        .collect();
    assert_eq!(headings, vec![
        "Spaced repetition",
        "Why it works",
        "How I use it"
    ]);
    assert_eq!(
        note.blocks
            .iter()
            .map(|block| block.anchor.as_str())
            .collect::<Vec<_>>(),
        vec!["why"]
    );
    assert!(
        note.excerpt
            .starts_with("Reviewing just before you forget beats cramming, see Forgetting curve."),
        "{}",
        note.excerpt
    );
}

#[test]
fn backlinks_come_by_name_alias_and_any_case() {
    let (_temp, vault) = vault();
    let backlinks = vault.index().backlinks(SPACED, 0, 50).unwrap();
    assert_eq!(paths(&backlinks, |row| &row.path), vec![
        "journal/2026-11-10.md",
        "notes/Anki workflow.md",
        "notes/Anki workflow.md",
        "wiki/Forgetting curve.md"
    ]);
    let journal = backlinks
        .iter()
        .find(|row| row.path.starts_with("journal"))
        .unwrap();
    assert_eq!(journal.line, 2);
    assert!(journal.context.contains("update [[Spaced repetition]]"));
    let info = vault.index().info(SPACED).unwrap().unwrap();
    assert_eq!(info.backlinks, 4);
}

#[test]
fn duplicate_names_resolve_to_the_nearest() {
    let (_temp, vault) = vault();
    let index = vault.index();
    let from_journal = index.outgoing("journal/2026-11-10.md").unwrap();
    let duplicate = from_journal
        .iter()
        .find(|link| link.target == "Duplicate")
        .unwrap();
    assert_eq!(duplicate.path.as_deref(), Some("wiki/Duplicate.md"));
    let from_notes = index.outgoing("notes/Anki workflow.md").unwrap();
    let duplicate = from_notes
        .iter()
        .find(|link| link.target == "Duplicate")
        .unwrap();
    assert_eq!(duplicate.path.as_deref(), Some("notes/Duplicate.md"));
    let block = from_notes
        .iter()
        .find(|link| link.target == "2026-11-10")
        .unwrap();
    assert_eq!(
        (block.path.as_deref(), block.anchor.as_deref()),
        (Some("journal/2026-11-10.md"), Some("^g7k2"))
    );
    let by_path = index.outgoing("wiki/Forgetting curve.md").unwrap();
    assert_eq!(
        by_path
            .iter()
            .find(|link| link.target == "notes/Duplicate")
            .unwrap()
            .path
            .as_deref(),
        Some("notes/Duplicate.md")
    );
}

#[test]
fn dangling_links_leave_attachments_out() {
    let (_temp, vault) = vault();
    let unresolved = vault.index().unresolved(10).unwrap();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].target, "Hermann Ebbinghaus");
    assert_eq!(unresolved[0].count, 2);
}

#[test]
fn search_ranks_titles_and_filters() {
    let (_temp, vault) = vault();
    let index = vault.index();
    let hits = index.search("spaced rep", 0, 10).unwrap();
    assert_eq!(hits[0].path, SPACED);
    let tagged = index.search("tag:learning", 0, 10).unwrap();
    assert_eq!(paths(&tagged, |hit| &hit.path), vec![SPACED]);
    let in_wiki = index.search("path:wiki ebbinghaus", 0, 10).unwrap();
    assert_eq!(paths(&in_wiki, |hit| &hit.path), vec![
        "wiki/Forgetting curve.md"
    ]);
    let concepts = index.search("type:concept", 0, 10).unwrap();
    assert_eq!(concepts.len(), 2);
    let without = index.search("spaced -coffee", 0, 10).unwrap();
    assert!(without.iter().all(|hit| !hit.path.contains("Café")));
    // Diacritics fold: "cafe" finds "Café".
    assert!(
        index
            .search("cafe", 0, 10)
            .unwrap()
            .iter()
            .any(|hit| hit.path.contains("Café"))
    );
    // Code and comments are text to search, though not links.
    assert!(!index.search("\"not a link\"", 0, 10).unwrap().is_empty());
}

#[test]
fn switcher_finds_aliases_and_fuzzy_names() {
    let (_temp, vault) = vault();
    let index = vault.index();
    let found = index.switch("srs", 5).unwrap();
    assert_eq!(found[0].0, SPACED);
    assert_eq!(found[0].2.as_deref(), Some("SRS"));
    assert_eq!(index.switch("dup", 5).unwrap().len(), 2);
    assert_eq!(
        index.switch("frgcrv", 5).unwrap()[0].0,
        "wiki/Forgetting curve.md"
    );
}

#[test]
fn mentions_tags_tasks_and_orphans() {
    let (_temp, vault) = vault();
    let index = vault.index();
    let mentions = index.mentions(SPACED, 10).unwrap();
    assert_eq!(paths(&mentions, |row| &row.path), vec!["notes/Café ☕.md"]);
    let tags: Vec<String> = index
        .tags()
        .unwrap()
        .into_iter()
        .map(|tag| tag.tag)
        .collect();
    for tag in [
        "learning",
        "memory",
        "learning/methods",
        "reading",
        "coffee",
        "rituals",
    ] {
        assert!(tags.contains(&tag.to_string()), "{tag} in {tags:?}");
    }
    assert!(!tags.contains(&"notatag".to_string()) && !tags.contains(&"hidden".to_string()));
    let open = index.tasks(false, 0, 10).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].text, "import my old Bear notes");
    assert_eq!(index.tasks(true, 0, 10).unwrap().len(), 2);
    let orphans = paths(&index.orphans("wiki", 10).unwrap(), |row| &row.path);
    assert!(!orphans.contains(&"wiki/Forgetting curve.md".to_string()));
    let listed = index.list("notes", Order::Title, 0, 10).unwrap();
    assert_eq!(
        listed.first().map(|row| row.title.as_str()),
        Some("Anki workflow")
    );
}

#[test]
fn rename_rewrites_links_but_keeps_aliases_anchors_and_text() {
    let (temp, mut vault) = vault();
    let renamed = vault.rename(SPACED, "notes/Spacing effect").unwrap();
    assert_eq!(renamed.to, "notes/Spacing effect.md");
    assert_eq!(renamed.links, 3);
    let read = |path: &str| fs::read_to_string(temp.path().join(path)).unwrap();
    assert!(read("journal/2026-11-10.md").contains("update [[Spacing effect]]"));
    let anki = read("notes/Anki workflow.md");
    assert!(anki.contains("[[SRS]]"), "{anki}");
    assert!(
        anki.contains("or [[Spacing effect]] in lowercase"),
        "{anki}"
    );
    assert!(read("wiki/Forgetting curve.md").contains("[[Spacing effect#How I use it|review]]"));
    assert!(!temp.path().join(SPACED).exists());
    assert_eq!(
        vault
            .index()
            .backlinks("notes/Spacing effect.md", 0, 50)
            .unwrap()
            .len(),
        4
    );

    // A markdown link follows a move into another folder.
    vault
        .rename("notes/Anki workflow.md", "notes/methods/Anki.md")
        .unwrap();
    assert!(read("notes/Spacing effect.md").contains("[the Anki way](methods/Anki.md)"));
    assert!(
        vault
            .index()
            .unresolved(10)
            .unwrap()
            .iter()
            .all(|row| row.target != "Anki workflow.md")
    );
}

#[test]
fn creating_a_note_resolves_links_waiting_for_it() {
    let (_temp, mut vault) = vault();
    let text = vault
        .create("wiki/Hermann Ebbinghaus.md", "German psychologist.\n")
        .unwrap();
    assert!(text.starts_with("---\nid: "));
    assert!(vault.index().unresolved(10).unwrap().is_empty());
    assert_eq!(
        vault
            .index()
            .backlinks("wiki/Hermann Ebbinghaus.md", 0, 10)
            .unwrap()
            .len(),
        2
    );
    assert!(vault.create("wiki/Hermann Ebbinghaus.md", "again").is_err());
}

#[test]
fn trashing_a_note_leaves_its_links_dangling() {
    let (temp, mut vault) = vault();
    vault.trash("wiki/Duplicate.md").unwrap();
    assert!(
        temp.path()
            .join(".commonplace/trash/wiki/Duplicate.md")
            .exists()
    );
    // The journal's [[Duplicate]] now finds the other one.
    let links = vault.index().outgoing("journal/2026-11-10.md").unwrap();
    assert_eq!(
        links
            .iter()
            .find(|link| link.target == "Duplicate")
            .unwrap()
            .path
            .as_deref(),
        Some("notes/Duplicate.md")
    );
}

#[test]
fn writes_merge_with_changes_made_elsewhere() {
    let (temp, mut vault) = vault();
    let path = "notes/Duplicate.md";
    let base = vault.read(path).unwrap();
    // Obsidian appends a line while our editor changes the first one.
    fs::write(temp.path().join(path), format!("{base}Added elsewhere.\n")).unwrap();
    let saved = vault
        .write(
            path,
            "The notes one, edited.\n",
            Some(&base),
            "2026-11-10 1412",
        )
        .unwrap();
    assert!(matches!(saved, Saved::Merged { .. }), "{saved:?}");
    assert_eq!(
        vault.read(path).unwrap(),
        "The notes one, edited.\nAdded elsewhere.\n"
    );
    assert!(!vault.index().search("edited", 0, 5).unwrap().is_empty());
    // Outside the vault's notes, writes are refused.
    assert!(vault.write("../escape.md", "x", None, "s").is_err());
    assert!(vault.write(".obsidian/x.md", "x", None, "s").is_err());
}

#[test]
fn edits_on_disk_are_picked_up_by_refresh_and_scan() {
    let (temp, mut vault) = vault();
    fs::write(
        temp.path().join("notes/New.md"),
        "Links to [[Spaced repetition]].\n",
    )
    .unwrap();
    vault.refresh("notes/New.md").unwrap();
    assert_eq!(vault.index().backlinks(SPACED, 0, 50).unwrap().len(), 5);
    fs::remove_file(temp.path().join("notes/New.md")).unwrap();
    let report = vault.scan().unwrap();
    assert_eq!(report.removed, 1);
    assert_eq!(vault.index().backlinks(SPACED, 0, 50).unwrap().len(), 4);
}

#[test]
fn a_lost_index_is_rebuilt_and_journaled_writes_finish() {
    let temp = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault"),
        temp.path(),
    );
    let index = index_of(&temp);
    {
        let mut vault = Vault::open_with_index(temp.path(), &index).unwrap();
        vault.scan().unwrap();
    }
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", index.display()));
    }
    fs::write(&index, b"not a database").unwrap();
    // A write that was journaled but not done when the app stopped.
    let journal = temp.path().join(".commonplace/journal");
    fs::create_dir_all(&journal).unwrap();
    fs::write(
        journal.join("pending.json"),
        r#"{"path":"inbox.md","text":"- recovered\n"}"#,
    )
    .unwrap();
    let mut vault = Vault::open_with_index(temp.path(), &index).unwrap();
    assert_eq!(
        fs::read_to_string(temp.path().join("inbox.md")).unwrap(),
        "- recovered\n"
    );
    let report = vault.scan().unwrap();
    assert_eq!(report.parsed, 13);
    assert_eq!(vault.index().backlinks(SPACED, 0, 50).unwrap().len(), 4);
}
