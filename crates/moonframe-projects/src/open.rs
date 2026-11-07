//! What is shown of the open project: its changes, folders, one file and
//! last commits. Paths come from mods, and links from cloned repositories,
//! so only what stays inside the project is read.

use {
    crate::{
        MAX_DIRS,
        git::{command, git},
        last_name,
        scan::mtime,
    },
    serde_json::{Map, Value, json},
    std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        io::{Read, Write},
        path::Path,
        process::Stdio,
        thread,
    },
};

/// Entries in `current.files`.
const MAX_FILES: usize = 300;
/// Entries listed per folder.
const MAX_ENTRIES: usize = 400;
/// Lines of a file read.
const MAX_LINES: usize = 3000;
/// Characters a line keeps.
const MAX_LINE: usize = 400;
/// Bytes of a file read.
const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Commits in `current.log`.
const MAX_LOG: usize = 30;
/// An untracked file past this size counts no lines.
const MAX_COUNTED: u64 = 512 * 1024;
/// How much of a file is looked through for a NUL, to call it binary.
const SNIFF: usize = 8192;

/// A file's change: status, lines added, lines deleted.
type Change = (String, u64, u64);

/// The commit a branch's changes count from: where it left the default branch.
fn branch_base(dir: &str) -> Option<String> {
    [
        "origin/HEAD",
        "origin/main",
        "origin/master",
        "main",
        "master",
    ]
    .iter()
    .find_map(|name| {
        let base = git(dir, &["merge-base", "HEAD", name])?;
        let base: String = base.chars().take_while(char::is_ascii_hexdigit).collect();
        (!base.is_empty()).then_some(base)
    })
}

/// Lines in an untracked file, if it is small enough to count.
fn counted(full: &str) -> u64 {
    let small = fs::metadata(full).is_ok_and(|file| file.is_file() && file.len() < MAX_COUNTED);
    if !small {
        return 0;
    }
    let data = fs::read(full).unwrap_or_default();
    let lines = data.iter().filter(|byte| **byte == b'\n').count() as u64;
    lines + u64::from(data.last().is_some_and(|byte| *byte != b'\n'))
}

/// `current`'s changes, by `scope`: what is uncommitted, what the branch
/// changed, or the last commit. Untracked files are added ones.
pub(crate) fn changes(dir: &str, scope: &str) -> Map<String, Value> {
    let base;
    let span: Vec<&str> = match scope {
        "last" => vec!["HEAD~1", "HEAD"],
        "branch" => {
            base = branch_base(dir).unwrap_or_else(|| "HEAD".to_owned());
            vec![base.as_str()]
        },
        _ => vec!["HEAD"],
    };
    let diff = |flag: &str| {
        let mut args = vec!["diff"];
        args.extend(&span);
        args.extend([flag, "-z", "--no-renames"]);
        git(dir, &args)
    };
    let mut files: BTreeMap<String, Change> = BTreeMap::new();
    if let Some(names) = diff("--name-status") {
        let mut parts = names.split('\0');
        while let (Some(status), Some(path)) = (parts.next(), parts.next()) {
            let status: String = status.chars().take(1).collect();
            files.insert(path.to_owned(), (status, 0, 0));
        }
    }
    if let Some(numstat) = diff("--numstat") {
        for record in numstat.split('\0') {
            let mut fields = record.splitn(3, '\t');
            if let (Some(added), Some(deleted), Some(path)) =
                (fields.next(), fields.next(), fields.next())
                && let Some(file) = files.get_mut(path)
            {
                file.1 = added.parse().unwrap_or(0);
                file.2 = deleted.parse().unwrap_or(0);
            }
        }
    }
    if scope != "last" {
        let others = ["ls-files", "--others", "--exclude-standard", "-z"];
        let untracked = git(dir, &others).unwrap_or_default();
        for path in untracked.split('\0').filter(|path| !path.is_empty()) {
            if !files.contains_key(path) {
                let lines = counted(&format!("{dir}/{path}"));
                files.insert(path.to_owned(), ("A".to_owned(), lines, 0));
            }
        }
    }
    let added: u64 = files.values().map(|file| file.1).sum();
    let deleted: u64 = files.values().map(|file| file.2).sum();
    let listed: Vec<String> = files
        .iter()
        .take(MAX_FILES)
        .map(|(path, (status, added, deleted))| format!("{status}\t{added}\t{deleted}\t{path}"))
        .collect();
    let summary = json!({
        "files": listed,
        "count": files.len(),
        "truncated": files.len() > MAX_FILES,
        "added": added,
        "deleted": deleted,
    });
    match summary {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// Whether `rel` stays inside the project, by itself: no `..`, not absolute.
pub(crate) fn inside(rel: &str) -> bool {
    !rel.starts_with('/') && !rel.split('/').any(|part| part == "..")
}

/// Whether `full`, links followed, is still inside the repository: a link in
/// a cloned repository could point anywhere the user can read.
fn contained(root: &str, full: &str) -> bool {
    match (fs::canonicalize(root), fs::canonicalize(full)) {
        (Ok(root), Ok(full)) => full.starts_with(root),
        _ => false,
    }
}

/// The names in `given` (NUL-ended paths) git ignores in `root`.
fn ignored(root: &str, given: String) -> BTreeSet<String> {
    let child = command("git")
        .args(["-C", root, "check-ignore", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return BTreeSet::new();
    };
    let (input, output) = (child.stdin.take(), child.stdout.take());
    // Written beside the read, as git answers while it is told.
    let writer = thread::spawn(move || {
        if let Some(mut input) = input {
            let _ = input.write_all(given.as_bytes());
        }
    });
    let mut said = Vec::new();
    if let Some(mut output) = output {
        let _ = output.read_to_end(&mut said);
    }
    let _ = writer.join();
    let _ = child.wait();
    String::from_utf8_lossy(&said)
        .split('\0')
        .filter_map(last_name)
        .map(str::to_owned)
        .collect()
}

/// A folder of the project, folders first: `<kind><name>`, where kind is d
/// (a folder), f (a file), and D or F when git ignores it.
pub(crate) fn list_dir(root: &str, rel: &str) -> Option<Vec<String>> {
    let base = if rel.is_empty() {
        root.to_owned()
    } else {
        format!("{root}/{rel}")
    };
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    for entry in fs::read_dir(&base).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches!(name.as_str(), ".git" | ".DS_Store") || name.contains('\n') {
            continue;
        }
        let folder = Path::new(&base).join(&name).is_dir();
        if folder {
            &mut dirs
        } else {
            &mut files
        }
        .push(name);
    }
    dirs.sort_by_cached_key(|name| (name.to_lowercase(), name.clone()));
    files.sort_by_cached_key(|name| (name.to_lowercase(), name.clone()));
    let entries: Vec<(String, bool)> = dirs
        .into_iter()
        .map(|name| (name, true))
        .chain(files.into_iter().map(|name| (name, false)))
        .take(MAX_ENTRIES)
        .collect();
    let prefix = if rel.is_empty() {
        String::new()
    } else {
        format!("{rel}/")
    };
    let given: String = entries
        .iter()
        .map(|(name, _)| format!("{prefix}{name}\0"))
        .collect();
    let hidden = if entries.is_empty() {
        BTreeSet::new()
    } else {
        ignored(root, given)
    };
    let kinds = entries.into_iter().map(|(name, folder)| {
        let kind = match (folder, hidden.contains(&name)) {
            (true, false) => 'd',
            (false, false) => 'f',
            (true, true) => 'D',
            (false, true) => 'F',
        };
        format!("{kind}{name}")
    });
    Some(kinds.collect())
}

/// One line as the code view shows it: tabs as spaces, no control
/// characters, cut at [`MAX_LINE`].
fn shown(line: &str) -> String {
    line.replace('\t', "    ")
        .chars()
        .filter(|c| !matches!(c, '\u{0}'..='\u{8}' | '\u{b}'..='\u{1f}' | '\u{7f}'))
        .take(MAX_LINE)
        .collect()
}

fn read_text(rel: &str, raw: &[u8]) -> Value {
    if raw.iter().take(SNIFF).any(|byte| *byte == 0) {
        return json!({ "path": rel, "lines": [], "total": 0, "truncated": false,
            "binary": true, "missing": false });
    }
    let text = String::from_utf8_lossy(raw);
    let mut lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let total = lines.len();
    let lines: Vec<String> = lines.iter().take(MAX_LINES).map(|l| shown(l)).collect();
    json!({ "path": rel, "lines": lines, "total": total, "truncated": total > MAX_LINES,
        "binary": false, "missing": false })
}

/// The file being read, as it was when it last changed.
#[derive(Default)]
pub(crate) struct Reader {
    last: Option<(String, u64, Value)>,
}

impl Reader {
    pub(crate) fn read(&mut self, root: &str, rel: &str) -> Value {
        let full = format!("{root}/{rel}");
        let when = mtime(&full);
        if let Some((path, stamp, data)) = &self.last
            && *path == full
            && *stamp == when
        {
            return data.clone();
        }
        let data = if Path::new(&full).is_file() && contained(root, &full) {
            let mut raw = Vec::new();
            match fs::File::open(&full) {
                Ok(file) => {
                    let _ = file.take(MAX_BYTES).read_to_end(&mut raw);
                    read_text(rel, &raw)
                },
                Err(_) => json!({ "path": rel, "lines": [], "total": 0, "truncated": false,
                    "binary": false, "missing": false }),
            }
        } else {
            json!({ "path": rel, "lines": [], "total": 0, "truncated": false,
                "binary": false, "missing": true })
        };
        self.last = Some((full, when, data.clone()));
        data
    }
}

/// The last [`MAX_LOG`] commits of the open project.
pub(crate) fn commit_log(dir: &str) -> Vec<Value> {
    let count = format!("-{MAX_LOG}");
    let args = ["log", count.as_str(), "--format=%h%x09%ct%x09%an%x09%s"];
    let text = git(dir, &args).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, '\t');
            let (sha, when) = (fields.next()?, fields.next()?);
            let (author, subject) = (fields.next()?, fields.next()?);
            Some(json!({
                "sha": sha,
                "time": when.parse::<u64>().unwrap_or(0),
                "author": author.chars().take(60).collect::<String>(),
                "subject": subject.chars().take(200).collect::<String>(),
            }))
        })
        .collect()
}

/// The listings asked for: the root, a composer's folder, and the folders
/// open in the inspector, each once.
pub(crate) fn listings(dir: &str, wanted: &[String]) -> Map<String, Value> {
    let mut dirs = Map::new();
    let mut seen = BTreeSet::new();
    let unique = wanted.iter().filter(|rel| seen.insert(rel.as_str()));
    for rel in unique.filter(|rel| inside(rel)).take(MAX_DIRS + 1) {
        if let Some(entries) = list_dir(dir, rel) {
            dirs.insert(rel.clone(), json!(entries));
        }
    }
    dirs
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn only_paths_inside_the_project_are_inside() {
        for rel in ["", "a", "a/b", "a..b/c"] {
            assert!(inside(rel), "{rel}");
        }
        for rel in ["/etc", "..", "../a", "a/../b", "a/.."] {
            assert!(!inside(rel), "{rel}");
        }
    }

    #[test]
    fn a_line_shows_without_control_characters() {
        assert_eq!(shown("a\tb\u{1}\u{7f}c"), "a    bc");
        assert_eq!(shown(&"é".repeat(500)).chars().count(), 400);
    }

    #[test]
    fn a_file_reads_as_lines_or_as_binary() {
        let read = read_text("f", b"one\r\ntwo\n");
        assert_eq!(read["lines"], json!(["one", "two"]));
        assert_eq!(read["total"], 2);
        assert_eq!(read_text("f", b"a\0b")["binary"], true);
        let many = "x\n".repeat(3001);
        let read = read_text("f", many.as_bytes());
        assert_eq!(read["total"], 3001);
        assert_eq!(read["truncated"], true);
        assert_eq!(read["lines"].as_array().unwrap().len(), 3000);
    }
}
