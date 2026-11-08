//! Installing, linking, updating, enabling, and removing packages.

use {
    super::{
        Contents, DIR, Entry, List,
        git::{checkout, git, head, resolve, short},
        source::{Source, expand_home, package_name},
    },
    std::{
        fs,
        io::ErrorKind,
        path::{Path, PathBuf},
    },
};

/// Asked before a package's code may run: `Err` refuses it.
pub(crate) type Approve<'a> = &'a dyn Fn(&Contents) -> Result<(), String>;

/// Clones `source` into the user's folder and enables it.
pub fn install(user: &Path, source: &str, approve: Approve) -> Result<(String, Contents), String> {
    let source = Source::parse(source)?;
    let mut list = List::read(user)?;
    if list.package.contains_key(&source.id) {
        return Err(format!(
            "{} is already installed; `moonframe mod update {}` fetches the latest",
            source.id,
            source.name()
        ));
    }
    let packages = user.join(DIR);
    let target = packages.join(&source.id);
    if target.exists() {
        return Err(format!(
            "{} already exists; move it away first",
            target.display()
        ));
    }
    // Cloned beside the others, so it moves into place in one rename.
    let incoming = packages.join(format!(".incoming-{}", std::process::id()));
    let _ = fs::remove_dir_all(&incoming);
    fs::create_dir_all(&packages).map_err(|error| format!("{}: {error}", packages.display()))?;
    let installed = (|| {
        git(
            &packages,
            &[
                "clone",
                "--quiet",
                "--",
                &source.url,
                &incoming.display().to_string(),
            ],
        )?;
        if let Some(reference) = &source.reference {
            checkout(&incoming, Some(reference))?;
        }
        let contents = Contents::read(&incoming, source.name())?;
        approve(&contents)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::rename(&incoming, &target).map_err(|error| format!("{}: {error}", target.display()))?;
        Ok(contents)
    })();
    let contents = match installed {
        Ok(contents) => contents,
        Err(problem) => {
            let _ = fs::remove_dir_all(&incoming);
            return Err(problem);
        },
    };
    list.package.insert(
        source.id.clone(),
        Entry {
            url: source.url,
            reference: source.reference,
            link: false,
            enabled: true,
        },
    );
    // Unlisted, the folder would only block the next install.
    if let Err(problem) = list.write(user) {
        let _ = fs::remove_dir_all(&target);
        return Err(problem);
    }
    Ok((source.id, contents))
}

/// Uses `folder` in place as a package, so edits show as they are made.
pub fn link(user: &Path, folder: &str, approve: Approve) -> Result<(String, Contents), String> {
    let folder = expand_home(folder);
    let folder = folder
        .canonicalize()
        .map_err(|error| format!("{}: {error}", folder.display()))?;
    let name = folder
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no name", folder.display()))?;
    let url = folder.display().to_string();
    let mut list = List::read(user)?;
    if let Some(id) = list.linked(&folder) {
        return Err(format!("{id} is already installed"));
    }
    // Two folders of one name, such as `a/shared` and `b/shared`, are two
    // packages.
    let mut id = format!("local/{name}");
    let mut n = 1;
    while list.package.contains_key(&id) {
        n += 1;
        id = format!("local/{name}-{n}");
    }
    let contents = Contents::read(&folder, name)?;
    approve(&contents)?;
    list.package.insert(
        id.clone(),
        Entry {
            url,
            reference: None,
            link: true,
            enabled: true,
        },
    );
    list.write(user)?;
    Ok((id, contents))
}

/// What `update` did to one package.
#[derive(Debug, PartialEq)]
pub(crate) enum Updated {
    Current(String),
    Moved { from: String, to: String },
    Linked,
}

/// Fetches each named package, or all of them, and checks out the latest
/// of what it follows. A package whose new version `approve` refuses stays
/// where it was.
pub(crate) fn update(
    user: &Path,
    query: Option<&str>,
    approve: Approve,
) -> Result<Vec<(String, Result<Updated, String>)>, String> {
    let list = List::read(user)?;
    let ids = match query {
        Some(query) => vec![list.find(query)?],
        None => list.package.keys().cloned().collect(),
    };
    Ok(ids
        .into_iter()
        .map(|id| {
            let entry = &list.package[&id];
            let folder = entry.folder(user, &id);
            let name = package_name(&id);
            let updated = if entry.link {
                Ok(Updated::Linked)
            } else {
                let links = list.links_inside(&folder);
                update_one(&folder, name, entry.reference.as_deref(), &links, approve)
            };
            (id, updated)
        })
        .collect())
}

fn update_one(
    folder: &Path,
    name: &str,
    reference: Option<&str>,
    links: &[PathBuf],
    approve: Approve,
) -> Result<Updated, String> {
    if !folder.is_dir() {
        return Err(format!(
            "{} is missing; remove the package and install it again",
            folder.display()
        ));
    }
    let before = head(folder)?;
    git(folder, &["fetch", "--quiet", "--tags", "--force", "origin"])?;
    // The default branch may have changed; following it is best effort.
    let _ = git(folder, &["remote", "set-head", "origin", "--auto"]);
    let after = resolve(folder, reference)?;
    if after == before {
        return Ok(Updated::Current(short(&after)));
    }
    // The package's folder is a live layer: an open window would start a
    // new provider the moment it appeared there. So the new version is
    // read in a checkout of its own, outside every layer, and reaches the
    // folder only once approved.
    let incoming = folder.with_file_name(format!(".incoming-{}", std::process::id()));
    let _ = fs::remove_dir_all(&incoming);
    let shown = incoming.display().to_string();
    git(
        folder,
        &["worktree", "add", "--quiet", "--detach", &shown, &after],
    )?;
    let approved = Contents::read(&incoming, name)
        .and_then(|mut contents| {
            contents.add_links(&incoming, links)?;
            Ok(contents)
        })
        .and_then(|contents| approve(&contents));
    let _ = git(folder, &["worktree", "remove", "--force", &shown]);
    let _ = fs::remove_dir_all(&incoming);
    let _ = git(folder, &["worktree", "prune"]);
    approved?;
    git(folder, &["checkout", "--quiet", "--detach", &after])
        .map_err(|problem| format!("cannot check out {}: {problem}", short(&after)))?;
    Ok(Updated::Moved {
        from: short(&before),
        to: short(&after),
    })
}

/// Uninstalls a package; a linked folder is left as it is.
pub(crate) fn remove(user: &Path, query: &str) -> Result<String, String> {
    let mut list = List::read(user)?;
    let id = list.find(query)?;
    let Some(entry) = list.package.remove(&id) else {
        return Err(format!("no package `{id}`"));
    };
    // The folder goes first: listed but missing, `update` names it; unlisted
    // but there, it would block the next install.
    if !entry.link {
        let folder = entry.folder(user, &id);
        match fs::remove_dir_all(&folder) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                return Err(format!("{}: {error}", folder.display()));
            },
            _ => {},
        }
        // Drops the owner and host folders it leaves empty.
        let packages = user.join(DIR);
        for parent in folder.ancestors().skip(1) {
            if parent == packages
                || !parent.starts_with(&packages)
                || fs::remove_dir(parent).is_err()
            {
                break;
            }
        }
    }
    list.write(user)?;
    Ok(id)
}

/// Turns a package on or off without uninstalling it.
pub(crate) fn set_enabled(user: &Path, query: &str, enabled: bool) -> Result<String, String> {
    let mut list = List::read(user)?;
    let id = list.find(query)?;
    if let Some(entry) = list.package.get_mut(&id) {
        entry.enabled = enabled;
    }
    list.write(user)?;
    Ok(id)
}

/// One installed package, for `mod list`.
pub struct Installed {
    pub id: String,
    pub(crate) enabled: bool,
    pub link: bool,
    pub reference: Option<String>,
    /// The checked-out commit, abbreviated.
    pub commit: Option<String>,
    pub contents: Result<Contents, String>,
}

pub fn installed(user: &Path) -> Result<Vec<Installed>, String> {
    Ok(List::read(user)?
        .package
        .into_iter()
        .map(|(id, entry)| {
            let folder = entry.folder(user, &id);
            let name = package_name(&id).to_owned();
            Installed {
                commit: head(&folder).ok().map(|commit| short(&commit)),
                contents: if folder.is_dir() {
                    Contents::read(&folder, &name)
                } else {
                    Err(format!("{} is missing", folder.display()))
                },
                id,
                enabled: entry.enabled,
                link: entry.link,
                reference: entry.reference,
            }
        })
        .collect())
}
