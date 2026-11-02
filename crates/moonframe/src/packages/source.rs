//! What `moonframe mod install` and an app's `requires` name: a repository
//! or a folder, and the id it installs as.

use {
    super::{git::git, printable},
    std::{
        env,
        path::{Path, PathBuf},
    },
};

/// What `install` was given: where to clone from, and the id it goes by.
#[derive(Debug, PartialEq)]
pub(super) struct Source {
    pub(super) id: String,
    pub(super) url: String,
    pub(super) reference: Option<String>,
}

impl Source {
    /// `github.com/owner/repo`, `owner/repo` (on GitHub), an `https://`,
    /// `ssh://` or `git@host:path` URL, or a folder; any of them may end in
    /// `@<ref>`: a branch, tag, or commit.
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (text, reference) = split_reference(text);
        let reference = match reference {
            Some("") => return Err(format!("`{text}@` names no ref")),
            // Git would read it as one of its options.
            Some(reference) if reference.starts_with('-') => {
                return Err(format!("`{}` is not a ref", printable(reference)));
            },
            reference => reference.map(str::to_owned),
        };
        if is_folder(text) {
            let folder = expand_home(text);
            let folder = std::path::absolute(&folder).unwrap_or(folder);
            let name = folder
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("`{text}` names no folder"))?;
            return Ok(Self {
                id: format!("local/{}", strip_git(name)),
                url: folder.display().to_string(),
                reference,
            });
        }
        let (url, path) = if let Some((scheme, rest)) = text.split_once("://") {
            if !["https", "http", "ssh", "git", "file"].contains(&scheme) {
                return Err(format!("cannot clone from {scheme}:// URLs"));
            }
            // Without `user@` and `:port`.
            let rest = rest.rsplit_once('@').map_or(rest, |(_, rest)| rest);
            let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
            let host = host.split(':').next().unwrap_or(host);
            (text.to_owned(), format!("{host}/{path}"))
        } else if let Some((login, path)) = text.split_once(':')
            && let Some((_, host)) = login.split_once('@')
        {
            (text.to_owned(), format!("{host}/{path}"))
        } else if text.split('/').count() == 2 && !text.contains('.') {
            (
                format!("https://github.com/{text}"),
                format!("github.com/{text}"),
            )
        } else {
            (format!("https://{text}"), text.to_owned())
        };
        let id = strip_git(path.trim_matches('/'));
        let parts: Vec<&str> = id.split('/').collect();
        let valid = parts.len() >= 2
            && parts.iter().all(|part| {
                !part.is_empty()
                    && *part != "."
                    && *part != ".."
                    && part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            });
        if !valid {
            return Err(format!(
                "`{text}` is not a repository: try github.com/owner/repo, a git URL, or a folder"
            ));
        }
        Ok(Self {
            id: id.to_owned(),
            url,
            reference,
        })
    }

    /// The folder name the package is cloned to, which names a single mod.
    pub(super) fn name(&self) -> &str {
        package_name(&self.id)
    }
}

/// The last part of a package's id: the folder name it is cloned to.
pub(super) fn package_name(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

/// Whether `text` names a folder rather than a repository. `C:\` and
/// `\\server\` are folders too, on Windows.
fn is_folder(text: &str) -> bool {
    text.starts_with(['/', '.', '~']) || Path::new(text).is_absolute()
}

/// `text` without its `@<ref>`, and the ref. A repository's ref follows the
/// first `@` in its path, so a branch may hold `/` (`owner/repo@feature/x`),
/// unlike `git@host:` or `ssh://user@host/`, whose `@` comes before it. A
/// folder's is in its last part, since a folder may hold `@` anywhere.
fn split_reference(text: &str) -> (&str, Option<&str>) {
    let at = if is_folder(text) {
        let last = text.rfind(['/', '\\']).map_or(0, |slash| slash + 1);
        text[last..].rfind('@').map(|at| last + at)
    } else {
        let body = text.find("://").map_or(0, |scheme| scheme + 3);
        text[body..].find('/').and_then(|slash| {
            let path = body + slash;
            text[path..].find('@').map(|at| path + at)
        })
    };
    match at {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    }
}

/// Where `source`, a requirement in the moonframe.toml of the app in
/// `folder`, points when it is a package shipped beside the app: a `./` or
/// `../` path, which must stay inside the app's git repository (its folder,
/// outside one), so a downloaded app cannot reach the user's other folders.
/// `None` for a repository to clone. Other folders, `~` and `file://` are
/// refused: what an app requires must come with it, or from a repository.
pub(crate) fn beside(folder: Option<&Path>, source: &str) -> Result<Option<PathBuf>, String> {
    let source = source.trim();
    let shown = printable(source);
    if !(source.starts_with("./") || source.starts_with("../")) {
        if is_folder(source) || source.starts_with("file:") {
            return Err(format!(
                "`{shown}`: an app requires repositories, or packages shipped inside its own \
                 repository as ./ or ../ paths"
            ));
        }
        return Ok(None);
    }
    let folder = folder.ok_or_else(|| format!("`{shown}`: no app folder to find it beside"))?;
    let folder = folder
        .canonicalize()
        .map_err(|error| format!("{}: {error}", folder.display()))?;
    let root = git(&folder, &["rev-parse", "--show-toplevel"])
        .ok()
        .and_then(|root| PathBuf::from(root).canonicalize().ok())
        .filter(|root| folder.starts_with(root))
        .unwrap_or_else(|| folder.clone());
    let path = normalize(&folder.join(source));
    // Through any link it holds, where it really is.
    let path = path.canonicalize().unwrap_or(path);
    if !path.starts_with(&root) {
        return Err(format!(
            "`{shown}` leaves the app's repository, {}",
            root.display()
        ));
    }
    if path == folder {
        return Err(format!("`{shown}` is the app itself"));
    }
    Ok(Some(path))
}

/// `path` with its `.` and `..` parts worked out, without touching disk.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {},
            std::path::Component::ParentDir => {
                normal.pop();
            },
            part => normal.push(part),
        }
    }
    normal
}

fn strip_git(path: &str) -> &str {
    path.strip_suffix(".git").unwrap_or(path)
}

pub(super) fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            env::var_os("HOME").map(PathBuf::from).map_or_else(
                || PathBuf::from(path),
                |home| home.join(rest.trim_start_matches('/')),
            )
        },
        _ => PathBuf::from(path),
    }
}
