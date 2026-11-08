//! Folders and files of the checkout. Reads and writes stay inside it,
//! through links too.

use {
    serde_json::{Value, json},
    sha2::{Digest, Sha256},
    std::{
        fmt::Write as _,
        fs,
        path::{Path, PathBuf},
        time::Instant,
    },
};

const MAX_LISTED: usize = 400;
const MAX_FILE: u64 = 1024 * 1024;
/// How far into a file a NUL says it is binary.
const BINARY_WINDOW: usize = 8000;

/// All files of a folder, as a search last found them.
pub(super) struct Listing {
    pub(super) at: String,
    pub(super) time: Instant,
    pub(super) files: Vec<String>,
}

/// `path` under `root`: `{ name, dir }`, folders first.
pub(super) fn list_dir(root: &str, path: &str) -> Vec<Value> {
    let Ok(names) = fs::read_dir(Path::new(root).join(path)) else {
        return Vec::new();
    };
    let mut entries: Vec<(bool, String)> = names
        .flatten()
        .map(|entry| {
            let dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            (dir, entry.file_name().to_string_lossy().into_owned())
        })
        .filter(|(_, name)| name != ".git" && name != ".DS_Store")
        .collect();
    entries.sort_by_key(|(dir, name)| (!*dir, name.to_lowercase()));
    entries.truncate(MAX_LISTED);
    let rows = entries.into_iter();
    rows.map(|(dir, name)| json!({ "name": name, "dir": dir }))
        .collect()
}

/// A path inside the checkout, through links too.
fn inside(root: &str, path: &str) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let real = fs::canonicalize(root.join(path)).map_err(|error| error.to_string())?;
    if real.starts_with(&root) {
        Ok(real)
    } else {
        Err("That file is outside this checkout.".to_owned())
    }
}

fn version_of(text: &str) -> String {
    let mut version = String::new();
    for byte in &Sha256::digest(text.as_bytes())[..6] {
        let _ = write!(version, "{byte:02x}");
    }
    version
}

/// A file as `app.repo.file`, or why it can't be read.
pub(super) fn open_file(root: &str, path: &str) -> Result<Value, String> {
    let full = inside(root, path)?;
    let size = fs::metadata(&full)
        .map_err(|error| error.to_string())?
        .len();
    if size > MAX_FILE {
        return Ok(json!({ "path": path, "tooLarge": true }));
    }
    let bytes = fs::read(&full).map_err(|error| error.to_string())?;
    if bytes.iter().take(BINARY_WINDOW).any(|byte| *byte == 0) {
        return Ok(json!({ "path": path, "binary": true }));
    }
    let text = String::from_utf8_lossy(&bytes);
    Ok(json!({ "path": path, "text": text, "version": version_of(&text) }))
}

/// Saves `text` unless the file changed since `version`: the file as it now
/// stands, and whether it was saved.
pub(super) fn save_file(
    root: &str,
    path: &str,
    text: &str,
    version: Option<&str>,
) -> Result<(Value, bool), String> {
    let full = inside(root, path)?;
    let bytes = fs::read(&full).map_err(|error| error.to_string())?;
    let current = String::from_utf8_lossy(&bytes);
    let stale =
        version.is_some_and(|version| !version.is_empty() && version_of(&current) != version);
    if stale {
        // What's on disk now, so a second save knowingly replaces it.
        let file = json!({
            "path": path, "text": current, "version": version_of(&current), "conflict": true,
        });
        return Ok((file, false));
    }
    fs::write(&full, text).map_err(|error| error.to_string())?;
    let file = json!({ "path": path, "text": text, "version": version_of(text), "saved": true });
    Ok((file, true))
}

/// Every file under `root`, for a folder git doesn't know.
pub(super) fn walk(root: &str, dir: &str, depth: usize, files: &mut Vec<String>) {
    for entry in list_dir(root, dir) {
        let name = entry["name"].as_str().unwrap_or_default();
        let path = if dir.is_empty() {
            name.to_owned()
        } else {
            format!("{dir}/{name}")
        };
        let is_dir = entry["dir"] == true;
        if is_dir && depth < 4 && !name.starts_with('.') && name != "node_modules" {
            walk(root, &path, depth + 1, files);
        } else if !is_dir {
            files.push(path);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn folders_come_first_and_git_stays_out() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_string_lossy();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir(dir.path().join("zed")).unwrap();
        fs::write(dir.path().join("Apple"), "").unwrap();
        fs::write(dir.path().join("banana"), "").unwrap();
        let names: Vec<String> = list_dir(&root, "")
            .iter()
            .map(|row| row["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["zed", "Apple", "banana"]);
    }

    #[test]
    fn a_link_out_of_the_checkout_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        fs::create_dir(&root).unwrap();
        fs::write(dir.path().join("secret"), "x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("secret"), root.join("link")).unwrap();
        let root = root.to_string_lossy();
        assert!(open_file(&root, "../secret").is_err());
        #[cfg(unix)]
        assert!(open_file(&root, "link").is_err());
    }

    #[test]
    fn a_binary_file_and_a_big_one_are_not_opened() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_string_lossy();
        fs::write(dir.path().join("bin"), [1, 0, 2]).unwrap();
        fs::write(dir.path().join("big"), vec![b'x'; 2 * 1024 * 1024]).unwrap();
        assert_eq!(open_file(&root, "bin").unwrap()["binary"], true);
        assert_eq!(open_file(&root, "big").unwrap()["tooLarge"], true);
    }

    #[test]
    fn a_version_is_short_and_follows_the_text() {
        assert_eq!(version_of("a").len(), 12);
        assert_ne!(version_of("a"), version_of("b"));
    }
}
