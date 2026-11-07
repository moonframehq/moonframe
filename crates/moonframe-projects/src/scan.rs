//! Finding the repositories under the project folders, and what each one's
//! row shows: its branch, changes, work trees and last commit.

use {
    crate::{MAX_WORKTREES, git::git, last_name, tilde},
    serde_json::{Value, json},
    std::{
        collections::{BTreeSet, HashMap},
        fs,
        path::Path,
        thread,
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// Repositories summarised at once.
const AT_ONCE: usize = 8;

/// A repository found, and the folder it groups under.
pub(crate) struct Found {
    pub(crate) path: String,
    pub(crate) group: String,
}

fn seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

pub(crate) fn now() -> u64 {
    seconds(SystemTime::now())
}

pub(crate) fn mtime(path: &str) -> u64 {
    fs::metadata(path)
        .and_then(|file| file.modified())
        .map_or(0, seconds)
}

/// What is in `folder`, without dotfiles, as full paths.
fn children(folder: &str) -> Vec<String> {
    let entries = fs::read_dir(folder).into_iter().flatten().flatten();
    entries
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| !name.starts_with('.'))
        .map(|name| format!("{folder}/{name}"))
        .collect()
}

/// Sorted ignoring case, then by case.
fn sorted(mut paths: Vec<String>) -> Vec<String> {
    paths.sort_by_cached_key(|path| (path.to_lowercase(), path.clone()));
    paths
}

pub(crate) fn find_projects(roots: &[String]) -> Vec<Found> {
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        let first = children(root);
        let second = first.iter().flat_map(|dir| children(dir)).collect();
        for (depth, level) in [(1, sorted(first)), (2, sorted(second))] {
            for dir in level {
                let repository = Path::new(&dir).is_dir() && Path::new(&dir).join(".git").exists();
                let nested = || {
                    seen.iter()
                        .any(|s: &String| dir.starts_with(&format!("{s}/")))
                };
                if !repository || seen.contains(&dir) || nested() {
                    continue;
                }
                // A project two deep groups under its parent: the work trees
                // of one repository, say.
                let group = match dir.rsplit_once('/') {
                    Some((parent, _)) if depth == 2 => parent.to_owned(),
                    _ => root.clone(),
                };
                seen.insert(dir.clone());
                found.push(Found { path: dir, group });
            }
        }
    }
    found
}

/// The git folder of the work tree at `dir`: `.git`, or where a work tree's
/// `.git` file points.
fn git_dir(dir: &str) -> String {
    let dot = format!("{dir}/.git");
    if Path::new(&dot).is_file() {
        let line = fs::read_to_string(&dot).unwrap_or_default();
        let target = line.lines().next().and_then(|l| l.strip_prefix("gitdir:"));
        if let Some(target) = target.map(str::trim).filter(|target| !target.is_empty()) {
            return if Path::new(target).is_absolute() {
                target.to_owned()
            } else {
                format!("{dir}/{target}")
            };
        }
    }
    dot
}

/// The repository's work trees, the main one first, as git lists them.
fn worktrees(list: &str) -> Vec<Value> {
    let mut trees = Vec::new();
    for block in list.split("\n\n") {
        let field = |key: &str| block.lines().find_map(|line| line.strip_prefix(key));
        let Some(path) = field("worktree ").filter(|_| !block.lines().any(|l| l == "bare")) else {
            continue;
        };
        if trees.len() >= MAX_WORKTREES {
            break;
        }
        let branch = field("branch refs/heads/").unwrap_or("HEAD");
        trees.push(json!({ "path": path, "branch": branch, "primary": trees.is_empty() }));
    }
    trees
}

/// The branch, ahead, behind and changed files in `git status`'s output.
fn status(output: &str) -> (String, u64, u64, u64) {
    let (mut branch, mut ahead, mut behind, mut files) = (String::new(), 0, 0, 0);
    for entry in output.split('\0') {
        if let Some(head) = entry.strip_prefix("# branch.head ") {
            branch = if head == "(detached)" {
                "HEAD"
            } else {
                head
            }
            .to_owned();
        } else if let Some(counts) = entry.strip_prefix("# branch.ab +") {
            if let Some((plus, minus)) = counts.split_once(" -") {
                ahead = plus.parse().unwrap_or(0);
                behind = minus.parse().unwrap_or(0);
            }
        } else if ["1 ", "2 ", "u ", "? "]
            .iter()
            .any(|p| entry.starts_with(p))
        {
            files += 1;
        }
    }
    (branch, ahead, behind, files)
}

/// The lines added and deleted in `git diff --numstat`'s output; binary
/// files count none.
fn lines(numstat: &str) -> (u64, u64) {
    numstat.lines().fold((0, 0), |(added, deleted), line| {
        let mut fields = line.split('\t');
        let mut count = || {
            fields
                .next()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0)
        };
        (added + count(), deleted + count())
    })
}

/// A project's row, from four looks at its repository.
fn summarise(project: &Found, when: u64, home: &str) -> Value {
    let dir = project.path.as_str();
    let look = |args: &[&str]| git(dir, args).unwrap_or_default();
    let (branch, ahead, behind, files) = status(&look(&[
        "status",
        "--porcelain=v2",
        "--branch",
        "-z",
        "--untracked-files=normal",
    ]));
    let (added, deleted) = lines(&look(&["diff", "HEAD", "--numstat", "--no-renames"]));
    let log = look(&["log", "-1", "--format=%ct%x09%s"]);
    let (stamp, subject) = log.trim_end().split_once('\t').unwrap_or(("", ""));
    json!({
        "path": dir,
        "name": last_name(dir),
        "home": tilde(&project.group, home),
        "branch": branch,
        "files": files,
        "added": added,
        "deleted": deleted,
        "ahead": ahead,
        "behind": behind,
        "time": when.max(stamp.parse().unwrap_or(0)),
        "subject": subject.chars().take(200).collect::<String>(),
        "worktrees": worktrees(&look(&["worktree", "list", "--porcelain"])),
    })
}

/// Rows as they were when last looked at, by path, with what they depend on.
#[derive(Default)]
pub(crate) struct Summaries {
    cache: HashMap<String, (String, Value)>,
}

impl Summaries {
    /// A row for each project, each only as often as it needs: when its git
    /// folder shows activity, and each half minute, since edits to the work
    /// tree touch nothing there.
    pub(crate) fn of(&mut self, projects: &[Found], home: &str) -> Vec<Value> {
        let mut stale = Vec::new();
        for project in projects {
            let folder = git_dir(&project.path);
            let when = ["index", "HEAD", "logs/HEAD"]
                .iter()
                .map(|file| mtime(&format!("{folder}/{file}")))
                .max()
                .unwrap_or(0);
            // A work tree added or removed touches the main git folder's list.
            let key = format!(
                "{when}:{}:{}:{}",
                now() / 30,
                mtime(&format!("{folder}/worktrees")),
                mtime(&format!("{folder}/.."))
            );
            if self
                .cache
                .get(&project.path)
                .is_none_or(|(old, _)| *old != key)
            {
                stale.push((project, when, key));
            }
        }
        for batch in stale.chunks(AT_ONCE) {
            let rows: Vec<Value> = thread::scope(|scope| {
                let looks: Vec<_> = batch
                    .iter()
                    .map(|(project, when, _)| scope.spawn(move || summarise(project, *when, home)))
                    .collect();
                looks
                    .into_iter()
                    .map(|look| look.join().unwrap_or(Value::Null))
                    .collect()
            });
            for ((project, _, key), row) in batch.iter().zip(rows) {
                self.cache.insert(project.path.clone(), (key.clone(), row));
            }
        }
        projects
            .iter()
            .filter_map(|project| self.cache.get(&project.path))
            .map(|(_, row)| row.clone())
            .filter(|row| !row.is_null())
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_the_branch_and_counts_entries() {
        let output = [
            "# branch.oid abc",
            "# branch.head main",
            "# branch.ab +2 -3",
            "1 .M N... 1 2 3 a b file",
            "2 R. N... a b c d e R100 new",
            "old",
            "? other",
        ]
        .join("\0");
        assert_eq!(status(&output), ("main".to_owned(), 2, 3, 3));
        assert_eq!(status("# branch.head (detached)\0").0, "HEAD");
    }

    #[test]
    fn numstat_adds_up_and_skips_binary_files() {
        assert_eq!(lines("3\t1\ta\n-\t-\tpicture\n2\t0\tb\n"), (5, 1));
    }

    #[test]
    fn worktrees_list_the_main_one_first_without_bare_ones() {
        let list = "worktree /r\nHEAD 1\nbranch refs/heads/main\n\n\
                    worktree /w\nHEAD 2\ndetached\n\nworktree /b\nbare\n";
        let trees = worktrees(list);
        assert_eq!(trees.len(), 2);
        assert_eq!(
            trees[0],
            json!({ "path": "/r", "branch": "main", "primary": true })
        );
        assert_eq!(
            trees[1],
            json!({ "path": "/w", "branch": "HEAD", "primary": false })
        );
    }
}
