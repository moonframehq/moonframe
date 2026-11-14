//! What an agent changing the app is told about it, from the host's own
//! account of its folders ([`Editable`]): where it runs and where to write,
//! the one command that checks it, a short guide to Moonframe, the host's
//! own notes, and an index of the app's files with their lengths, so the
//! agent goes straight to a file instead of searching two repositories.

use {
    moonframe_providers::Editable,
    std::{
        fmt::Write as _,
        path::{Path, PathBuf},
    },
};

/// Moonframe's short guide to changing an app.
const GUIDE: &str = include_str!("guide.md");
/// The most files the index lists one by one.
const MAX_FILES: usize = 120;
/// How deep the index looks.
const MAX_DEPTH: usize = 6;
/// A folder with more files than this is summed up by name.
const MAX_IN_FOLDER: usize = 30;

/// The brief for an agent working in `editable`'s app.
pub(crate) fn brief(editable: &Editable) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "## This app: {}\n", editable.title);
    text.push_str(
        "The window runs on these folders, lowest first; a file in a higher one wins over the \
         one at the same path in a lower one:\n",
    );
    for (role, path) in &editable.layers {
        if !role.starts_with("entry:") {
            let _ = writeln!(text, "- {role}: {}", path.display());
        }
    }
    let writable = editable.writable.display();
    if editable.overlay() {
        let _ = writeln!(
            text,
            "\nWrite only in {writable}, the user's folder over the app. The app's own files in \
             {} are read-only to you: to change one, copy it to the same path in {writable} and \
             change the copy. A style.luau there is merged over the app's, so write only the \
             rules that change.",
            editable.app.display()
        );
    } else {
        let _ = writeln!(
            text,
            "\nThe app's own folder is {writable}: change its files there. Leave the toolbox and \
             packages alone; to change one of their files for this app, copy it to the same path \
             in {writable}."
        );
    }
    let entries: Vec<_> = editable
        .layers
        .iter()
        .filter_map(|(role, path)| role.strip_prefix("entry:").map(|name| (name, path)))
        .collect();
    if !entries.is_empty() {
        text.push_str(
            "\nThe following host-selected miniapps are active in this window. They are user \
             source, not app layers: you may read and edit their files when the request targets \
             one. Their provider code and manifests remain guarded and require permission.\n",
        );
        for (name, path) in entries {
            let _ = writeln!(text, "\n### {name}: {}\n", path.display());
            let _ = writeln!(text, "{}", index(path));
        }
    }
    let _ = writeln!(
        text,
        "You work in {}. The user's window reloads by itself as each file is saved.\n",
        editable.workdir.display()
    );
    let _ = writeln!(
        text,
        "Check the app with `{}`. It loads the app headlessly, as the window does, in about a \
         second, and prints each problem with its file and line. Run it after your edits until \
         it reports none. It is the only check you need: no build, no other recipe.\n",
        editable.check
    );
    if let Some(guide) = editable
        .guide
        .as_deref()
        .filter(|guide| !guide.trim().is_empty())
    {
        let _ = writeln!(text, "## From the app's authors\n\n{}\n", guide.trim());
    }
    text.push_str(GUIDE.trim());
    text.push_str("\n\n");
    let indexed = if editable.overlay() {
        &editable.app
    } else {
        &editable.writable
    };
    let _ = writeln!(
        text,
        "## Files in {} (lines)\n\n{}",
        indexed.display(),
        index(indexed)
    );
    if editable.overlay() {
        let own = index(&editable.writable);
        if !own.trim().is_empty() {
            let _ = writeln!(text, "\n## Files already in {writable} (lines)\n\n{own}");
        }
    }
    if let Some(toolbox) = editable.toolbox() {
        let _ = writeln!(
            text,
            "\nThe toolbox's components are in {}/components/<name>.luau.",
            toolbox.display()
        );
    }
    text
}

/// The app's Luau and provider manifests under `root`, one per line with
/// its length, and a busy folder (the toolbox's components) as names.
pub(crate) fn index(root: &Path) -> String {
    let mut files = Vec::new();
    walk(root, 0, &mut files);
    files.sort();
    let mut lines = Vec::new();
    let mut listed = 0;
    let mut folders: Vec<(PathBuf, Vec<&PathBuf>)> = Vec::new();
    for file in &files {
        let folder = file.parent().map(Path::to_owned).unwrap_or_default();
        match folders.last_mut() {
            Some((last, members)) if *last == folder => members.push(file),
            _ => folders.push((folder, vec![file])),
        }
    }
    for (folder, members) in folders {
        if members.len() > MAX_IN_FOLDER || listed + members.len() > MAX_FILES {
            let names: Vec<String> = members
                .iter()
                .filter_map(|file| Some(file.file_stem()?.to_string_lossy().into_owned()))
                .collect();
            lines.push(format!(
                "{}/ ({} files): {}",
                relative(root, &folder),
                names.len(),
                names.join(", ")
            ));
            continue;
        }
        for file in members {
            let count = std::fs::read(root.join(file))
                .map(|bytes| bytes.iter().filter(|byte| **byte == b'\n').count())
                .unwrap_or(0);
            lines.push(format!("{} ({count})", relative(root, file)));
            listed += 1;
        }
    }
    lines.join("\n")
}

/// The files worth indexing under `root`, from it.
fn walk(root: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    fn visit(root: &Path, folder: &Path, depth: usize, files: &mut Vec<PathBuf>) {
        if depth > MAX_DEPTH {
            return;
        }
        let Ok(entries) = std::fs::read_dir(root.join(folder)) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // Demos only show components in the gallery.
            if name.starts_with('.')
                || matches!(&*name, "target" | "node_modules" | "profiles" | "demos")
            {
                continue;
            }
            let path = folder.join(&*name);
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => visit(root, &path, depth + 1, files),
                Ok(_) if name.ends_with(".luau") || name == "provider.toml" => files.push(path),
                _ => {},
            }
        }
    }
    visit(root, Path::new(""), depth, files);
}

/// `path` under `root` with `/` between its parts, as the agent writes it.
fn relative(root: &Path, path: &Path) -> String {
    let parts: Vec<String> = path
        .strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A hosted app in a repository of its own, over Moonframe's toolbox.
    fn hosted(root: &Path) -> Editable {
        let app = root.join("notes/crates/notes/app");
        std::fs::create_dir_all(app.join("mods/home")).unwrap();
        std::fs::write(app.join("main.luau"), "return mod {\n}\n").unwrap();
        std::fs::write(app.join("mods/home/main.luau"), "a\nb\nc\n").unwrap();
        std::fs::create_dir_all(app.join("components")).unwrap();
        for index in 0..=MAX_IN_FOLDER {
            std::fs::write(app.join(format!("components/c{index}.luau")), "").unwrap();
        }
        Editable {
            title: "Notes".to_owned(),
            layers: vec![
                ("toolbox".to_owned(), root.join("moonframe/app")),
                ("app".to_owned(), app.clone()),
                ("user".to_owned(), root.join("config/notes")),
            ],
            app: app.clone(),
            writable: app,
            workdir: root.join("notes"),
            check: "/x/target/debug/notes --user /c --check /x/app".to_owned(),
            guide: Some("Pages live in mods/home.".to_owned()),
            state: None,
        }
    }

    #[test]
    fn a_hosted_apps_brief_names_its_own_folders_and_check() {
        let dir = tempfile::tempdir().unwrap();
        let editable = hosted(dir.path());
        let text = brief(&editable);
        assert!(text.contains("## This app: Notes"), "{text}");
        assert!(text.contains(&format!(
            "You work in {}",
            dir.path().join("notes").display()
        )));
        assert!(text.contains("`/x/target/debug/notes --user /c --check /x/app`"));
        assert!(text.contains("Pages live in mods/home."));
        assert!(text.contains("mods/home/main.luau (3)"), "{text}");
        assert!(text.contains("main.luau (2)"), "{text}");
        // A busy folder is summed up by name.
        assert!(
            text.contains(&format!(
                "components/ ({} files): c0, c1",
                MAX_IN_FOLDER + 1
            )),
            "{text}"
        );
        // Nothing of this repository's own tooling, which the host has not.
        for word in [
            "just ",
            "SKILL.md",
            "check-examples",
            "app link",
            "screenshot",
        ] {
            assert!(!text.contains(word), "`{word}` in {text}");
        }
    }

    #[test]
    fn an_overlay_brief_says_to_copy_rather_than_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut editable = hosted(dir.path());
        editable.writable = dir.path().join("config/notes");
        std::fs::create_dir_all(&editable.writable).unwrap();
        std::fs::write(editable.writable.join("style.luau"), "x\n").unwrap();
        let text = brief(&editable);
        assert!(text.contains("are read-only to you"), "{text}");
        assert!(text.contains("style.luau (1)"), "{text}");
    }

    #[test]
    fn a_selected_miniapp_is_explicitly_editable_and_indexed() {
        let dir = tempfile::tempdir().unwrap();
        let mut editable = hosted(dir.path());
        let miniapp = dir.path().join("config/miniapps/foobar");
        std::fs::create_dir_all(&miniapp).unwrap();
        std::fs::write(miniapp.join("main.luau"), "one\ntwo\n").unwrap();
        editable
            .layers
            .push(("entry:user:foobar".to_owned(), miniapp.clone()));

        let text = brief(&editable);
        assert!(text.contains("host-selected miniapps are active"), "{text}");
        assert!(text.contains("user:foobar"), "{text}");
        assert!(text.contains("main.luau (2)"), "{text}");
        assert!(!text.contains(&format!("- entry:user:foobar: {}", miniapp.display())));
    }
}
