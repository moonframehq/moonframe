//! An app package's `moonframe.toml`: its `[app]`, and what each field may
//! hold.

use {
    super::{MANIFEST, printable},
    serde::Deserialize,
    std::{fs, io::ErrorKind, path::Path},
};

/// `moonframe.toml`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    app: App,
}

/// An app package's `[app]`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct App {
    /// The profile it installs as, unless the user names another.
    pub(crate) name: String,
    /// Its name for people: the menu bar's, and a bundle's. The name if
    /// none.
    #[serde(default)]
    pub(crate) title: Option<String>,
    /// Its icon, a PNG in the app's folder (best 1024 pixels square): the
    /// Dock's and a bundle's. `icon.png` if none is named.
    #[serde(default)]
    pub(crate) icon: Option<String>,
    /// Built-in mods it keeps; it brings every other mod it shows.
    #[serde(default)]
    pub(crate) keep: Vec<String>,
    /// Packages it needs, as `moonframe mod install` takes them.
    #[serde(default)]
    pub(crate) requires: Vec<String>,
    /// Its version (`1.2.0`): the About panel's, a bundle's, and the one
    /// Check for Updates compares with its releases.
    #[serde(default)]
    pub(crate) version: Option<String>,
    /// The GitHub repository (`owner/repo`) whose releases update it: the
    /// app menu gets Check for Updates. Needs `version`.
    #[serde(default)]
    pub(crate) updates: Option<String>,
}

impl App {
    /// The `[app]` of the package in `folder`, if it is an app.
    pub(crate) fn read(folder: &Path) -> Result<Option<Self>, String> {
        let path = folder.join(MANIFEST);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
        };
        let app = toml::from_str::<ManifestFile>(&text)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .app;
        if !valid_name(&app.name) {
            return Err(format!(
                "{}: `{}` is not an app name: use a-z, 0-9, - and _",
                path.display(),
                printable(&app.name)
            ));
        }
        // A bundle is a folder named after the title, and its plist and
        // .desktop file hold it, so it must be a plain file name.
        if let Some(title) = &app.title
            && !valid_title(title)
        {
            return Err(format!(
                "{}: `{}` is not an app title: up to 64 characters, without \
                 control characters, / \\ < > & or a leading dot",
                path.display(),
                printable(title)
            ));
        }
        // The icon must stay inside the app's folder.
        if let Some(icon) = &app.icon
            && !valid_icon(icon)
        {
            return Err(format!(
                "{}: `{}` is not an icon path: a file inside the app's folder",
                path.display(),
                printable(icon)
            ));
        }
        // The version goes into a bundle's plist, so it is plain.
        if let Some(version) = &app.version
            && !valid_version(version)
        {
            return Err(format!(
                "{}: `{}` is not a version: digits and dots, such as 1.2.0, \
                 with an optional -pre-release",
                path.display(),
                printable(version)
            ));
        }
        if let Some(repo) = &app.updates {
            if !crate::update::valid_repo(repo) {
                return Err(format!(
                    "{}: `{}` is not a GitHub repository: owner/repo",
                    path.display(),
                    printable(repo)
                ));
            }
            if app.version.is_none() {
                return Err(format!(
                    "{}: `updates` needs the app's `version` to compare releases with",
                    path.display()
                ));
            }
        }
        Ok(Some(app))
    }
}

/// Whether `version` reads as one: `1.2.0`, `1.2.0-beta.1`.
fn valid_version(version: &str) -> bool {
    let (core, pre) = version.split_once('-').unwrap_or((version, ""));
    !core.is_empty()
        && core.split('.').count() <= 4
        && core
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        && pre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        && !(version.contains('-') && pre.is_empty())
}

/// A name for a profile or app: lowercase letters, digits, `-` and `_`.
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
}

/// Whether `title` can name a bundle folder and sit in a plist unescaped.
pub(crate) fn valid_title(title: &str) -> bool {
    !title.trim().is_empty()
        && title.chars().count() <= 64
        && !title.starts_with('.')
        && !title
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '<' | '>' | '&' | ':'))
}

/// Whether `icon` is a relative path that stays inside the app's folder.
fn valid_icon(icon: &str) -> bool {
    !icon.is_empty()
        && !icon.chars().any(char::is_control)
        && Path::new(icon)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}
