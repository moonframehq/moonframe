//! What changes slowly: HEAD's commit, recent branches, worktrees, the
//! remote, and stashes. Each reads one or two git commands.

use {
    crate::Repo,
    serde_json::{Value, json},
};

/// Entries in `branches` and `worktrees`.
const MAX_BRANCHES: usize = 20;
const MAX_WORKTREES: usize = 30;

/// HEAD's commit, or null before the first.
pub(crate) fn head(repo: &Repo) -> Value {
    let Some(text) = repo.text(
        &[
            "log",
            "-1",
            "--no-show-signature",
            "--format=%H%x00%h%x00%an%x00%ae%x00%ct%x00%s%x00",
            "HEAD",
        ],
        "read HEAD",
    ) else {
        return Value::Null;
    };
    let fields: Vec<&str> = text.split('\0').collect();
    let [sha, short, author, email, time, subject, ..] = fields[..] else {
        return Value::Null;
    };
    if sha.is_empty() {
        return Value::Null;
    }
    json!({
        "sha": sha, "short": short, "subject": subject, "author": author,
        "email": email, "time": time.parse::<i64>().unwrap_or(0),
    })
}

/// "ahead 2, behind 1", "ahead 2", "behind 1", "gone" or "".
pub(crate) fn track(text: &str) -> (u64, u64, bool) {
    let count = |word: &str| {
        text.split(", ")
            .find_map(|part| part.strip_prefix(word))
            .and_then(|count| count.trim().parse().ok())
            .unwrap_or(0)
    };
    (count("ahead "), count("behind "), text.trim() == "gone")
}

/// Recent local branches, newest commit first.
pub(crate) fn branches(repo: &Repo) -> Vec<Value> {
    let count = format!("--count={MAX_BRANCHES}");
    let Some(text) = repo.text(
        &[
            "for-each-ref",
            "--sort=-committerdate",
            &count,
            "--format=%(refname:short)%1f%(objectname:short)%1f%(committerdate:unix)%1f%(upstream:short)%1f%(upstream:track,nobracket)%1f%(HEAD)",
            "refs/heads/",
        ],
        "list branches",
    ) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\x1f').collect();
            let [name, head, time, upstream, tracking, current] = fields[..] else {
                return None;
            };
            if name.is_empty() {
                return None;
            }
            let (ahead, behind, gone) = track(tracking);
            let mut branch = json!({
                "name": name, "head": head, "time": time.parse::<i64>().unwrap_or(0),
                "ahead": ahead, "behind": behind, "gone": gone,
                "current": current.trim() == "*",
            });
            if !upstream.is_empty()
                && let Value::Object(fields) = &mut branch
            {
                fields.insert("upstream".to_owned(), json!(upstream));
            }
            Some(branch)
        })
        .collect()
}

/// One worktree as `git worktree list --porcelain` describes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Worktree {
    pub(crate) path: String,
    pub(crate) head: String,
    pub(crate) branch: String,
    pub(crate) detached: bool,
    pub(crate) locked: bool,
    pub(crate) prunable: bool,
    pub(crate) bare: bool,
    pub(crate) changed: u64,
    pub(crate) ahead: u64,
    pub(crate) behind: u64,
}

impl Worktree {
    /// `current` takes the fresh counts of the checkout the app runs in.
    pub(crate) fn json(&self, current: Option<(u64, u64, u64)>) -> Value {
        let (changed, ahead, behind) = current.unwrap_or((self.changed, self.ahead, self.behind));
        json!({
            "path": self.path, "branch": self.branch, "head": self.head,
            "current": current.is_some(), "changed": changed, "ahead": ahead, "behind": behind,
            "detached": self.detached, "locked": self.locked, "prunable": self.prunable,
            "bare": self.bare,
        })
    }
}

/// `worktree list --porcelain -z`: records of `key value` lines, each
/// worktree starting with `worktree <path>`.
pub(crate) fn parse_worktrees(text: &str) -> Vec<Worktree> {
    let mut worktrees: Vec<Worktree> = Vec::new();
    for line in text.split(['\0', '\n']) {
        if let Some(path) = line.strip_prefix("worktree ") {
            worktrees.push(Worktree {
                path: path.to_owned(),
                ..Worktree::default()
            });
            continue;
        }
        let Some(worktree) = worktrees.last_mut() else {
            continue;
        };
        if let Some(head) = line.strip_prefix("HEAD ") {
            head.clone_into(&mut worktree.head);
        } else if let Some(branch) = line.strip_prefix("branch ") {
            branch
                .strip_prefix("refs/heads/")
                .unwrap_or(branch)
                .clone_into(&mut worktree.branch);
        } else if line == "detached" {
            worktree.detached = true;
        } else if line.starts_with("locked") {
            worktree.locked = true;
        } else if line.starts_with("prunable") {
            worktree.prunable = true;
        } else if line == "bare" {
            worktree.bare = true;
        }
    }
    worktrees.truncate(MAX_WORKTREES);
    worktrees
}

/// Every worktree, with how far each branch is from its upstream and how
/// many files each has changed.
pub(crate) fn worktrees(repo: &Repo) -> Vec<Worktree> {
    let Some(text) = repo.text(&["worktree", "list", "--porcelain", "-z"], "list worktrees") else {
        return Vec::new();
    };
    let mut worktrees = parse_worktrees(&text);
    for worktree in &mut worktrees {
        if worktree.prunable || worktree.bare {
            continue;
        }
        if !worktree.branch.is_empty() {
            let reference = format!("refs/heads/{}", worktree.branch);
            let tracking = repo
                .text(
                    &[
                        "for-each-ref",
                        "--format=%(upstream:track,nobracket)",
                        &reference,
                    ],
                    "read a worktree's upstream",
                )
                .unwrap_or_default();
            (worktree.ahead, worktree.behind, _) = track(&tracking);
        }
        worktree.changed = Repo::at(&worktree.path)
            .text(&["status", "--porcelain"], "read a worktree's changes")
            .map_or(0, |text| text.lines().count() as u64);
    }
    worktrees
}

/// The remote the branch tracks, else `origin`, else the first one.
pub(crate) fn remote(repo: &Repo, branch: &str) -> Value {
    let configured = (branch != "HEAD")
        .then(|| {
            repo.text(
                &["config", "--get", &format!("branch.{branch}.remote")],
                "read",
            )
        })
        .flatten()
        .filter(|name| !name.is_empty() && name != ".");
    let name = configured.or_else(|| {
        repo.text(&["config", "--get", "remote.origin.url"], "read")
            .map(|_| "origin".to_owned())
            .or_else(|| {
                repo.text(&["remote"], "list remotes")
                    .and_then(|names| names.lines().next().map(str::to_owned))
            })
    });
    let Some(name) = name.filter(|name| !name.is_empty()) else {
        return Value::Null;
    };
    let Some(url) = repo.text(&["remote", "get-url", &name], "read the remote") else {
        return Value::Null;
    };
    remote_json(&name, &url)
}

/// `url` without the user and password a `scheme://user:token@host/...`
/// URL may carry, which mods must never see.
fn without_credentials(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
    let end = rest.find('/').unwrap_or(rest.len());
    match rest[..end].rfind('@') {
        Some(at) => format!("{scheme}://{}", &rest[at + 1..]),
        None => url.to_owned(),
    }
}

/// `name` and `url` with the host, owner and repository the URL names:
/// scp-like `user@host:owner/repo`, or `scheme://[user@]host[:port]/owner/repo`.
/// Credentials in the URL are left out.
pub fn remote_json(name: &str, url: &str) -> Value {
    let url = without_credentials(url);
    let url = url.as_str();
    let (host, path) = if let Some((_, rest)) = url.split_once("://") {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = host.rsplit('@').next().unwrap_or(host);
        (host.split(':').next().unwrap_or(host), path)
    } else if let Some((host, path)) = url.split_once(':') {
        (host.rsplit('@').next().unwrap_or(host), path)
    } else {
        ("", url)
    };
    let path = path.trim_end_matches('/');
    let path = path
        .strip_suffix(".git")
        .unwrap_or(path)
        .trim_start_matches('/');
    let kind = if host.contains("github") {
        "github"
    } else if host.contains("gitlab") {
        "gitlab"
    } else {
        "other"
    };
    let (owner, repo) = path.rsplit_once('/').unwrap_or(("", path));
    let mut remote = json!({
        "name": name, "url": url, "host": host, "kind": kind, "owner": owner, "repo": repo,
    });
    if !host.is_empty()
        && !owner.is_empty()
        && let Value::Object(fields) = &mut remote
    {
        fields.insert(
            "web_url".to_owned(),
            json!(format!("https://{host}/{path}")),
        );
    }
    remote
}

pub(crate) fn stashes(repo: &Repo) -> u64 {
    repo.text(
        &["rev-list", "--walk-reflogs", "--count", "refs/stash"],
        "count stashes",
    )
    .and_then(|count| count.trim().parse().ok())
    .unwrap_or(0)
}
