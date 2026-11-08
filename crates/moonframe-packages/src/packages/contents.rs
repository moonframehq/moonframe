//! What a package holds, read from its folder, and the check that it ships
//! source only.

use {
    super::{App, git::git, printable, source::beside},
    moonframe_core::mod_name,
    moonframe_providers::{Kind, Manifest},
    std::{
        fmt, fs,
        path::{Path, PathBuf},
    },
};

/// What a package holds.
#[derive(Debug, Default, PartialEq)]
pub struct Contents {
    /// An app's `[app]`: it brings a shell, and runs in a profile.
    pub app: Option<App>,
    pub(super) mods: Vec<String>,
    providers: Vec<(String, Provider)>,
    components: Vec<String>,
    style: bool,
    /// What an app ships beside it and requires (`./shared`), by the
    /// requirement: approved with the app, since a new version of the app
    /// brings a new version of them.
    beside: Vec<(String, Contents)>,
}

/// How a provider in a package would run.
#[derive(Debug, PartialEq)]
pub(crate) enum Provider {
    /// A command, with the user's access.
    Process(Vec<String>),
    /// A socket it connects to: whatever listens there acts on what it
    /// sends, which may be any program of the user's.
    Socket(String),
    /// A Rust crate the app builds with Cargo, which runs its build code
    /// with the user's access; the component it builds runs sandboxed.
    Source,
    /// The app's own built-in provider of that name, run with the
    /// package's `[settings]`, which may widen what it does (git's
    /// `writers`).
    Native(String),
    /// A JSON file it publishes as it is; nothing runs.
    Static(String),
    Broken(String),
}

impl Contents {
    /// Reads the package in `folder`, which is installed as `name`, and an
    /// app's, what it ships beside it.
    pub(super) fn read(folder: &Path, name: &str) -> Result<Self, String> {
        let mut contents = Self::read_one(folder, name)?;
        let requires = contents
            .app
            .as_ref()
            .map(|app| app.requires.clone())
            .unwrap_or_default();
        for source in requires {
            if let Some(path) = beside(Some(folder), &source)?
                && path.is_dir()
            {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&source)
                    .to_owned();
                let shipped = Self::read_one(&path, &name)?;
                contents.beside.push((source, shipped));
            }
        }
        Ok(contents)
    }

    /// Adds the linked packages at `links` inside `folder` that it does not
    /// already ship beside it: an update must not change what they run
    /// unasked just because it stopped requiring them.
    pub(super) fn add_links(&mut self, folder: &Path, links: &[PathBuf]) -> Result<(), String> {
        for inside in links {
            let source = format!("./{}", inside.display());
            let path = folder.join(inside);
            let listed = self
                .beside
                .iter()
                .any(|(shipped, _)| Path::new(shipped.trim_start_matches("./")) == inside);
            if listed || !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&source)
                .to_owned();
            let linked = Self::read_one(&path, &name)?;
            self.beside.push((source, linked));
        }
        Ok(())
    }

    fn read_one(folder: &Path, name: &str) -> Result<Self, String> {
        refuse_binaries(folder)?;
        let mut contents = Self {
            app: App::read(folder)?,
            ..Self::default()
        };
        if contents.app.is_none() && folder.join("main.luau").is_file() {
            contents.mods.push(mod_name(name));
            return Ok(contents);
        }
        contents.mods = names(&folder.join("mods"), |path| {
            path.join("main.luau").is_file()
        });
        for provider in names(&folder.join("providers"), |path| {
            path.join("provider.toml").is_file()
        }) {
            let dir = folder.join("providers").join(&provider);
            let how = match Manifest::load(&provider, &dir) {
                Ok(Manifest {
                    kind: Kind::Process { command },
                    ..
                }) => Provider::Process(command),
                Ok(Manifest {
                    kind: Kind::Socket { path },
                    ..
                }) => Provider::Socket(path),
                Ok(Manifest {
                    kind: Kind::Native,
                    settings,
                    ..
                }) => Provider::Native(settings.to_string()),
                Ok(Manifest {
                    kind: Kind::Static { file },
                    ..
                }) => Provider::Static(file),
                Ok(_) => Provider::Source,
                Err(problem) => Provider::Broken(problem.to_string()),
            };
            contents.providers.push((provider, how));
        }
        contents.components = names(&folder.join("components"), |path| {
            path.extension()
                .is_some_and(|extension| extension == "luau")
        })
        .into_iter()
        .map(|name| name.trim_end_matches(".luau").to_owned())
        .collect();
        contents.style = folder.join("style.luau").is_file();
        if contents.app.is_some() && !folder.join("main.luau").is_file() {
            return Err(format!(
                "{} is an app without a shell: it needs a main.luau",
                folder.display()
            ));
        }
        if contents == Self::default() {
            return Err(format!(
                "{} is not a Moonframe package: it has no main.luau, mods/, providers/, \
                 components/, or style.luau",
                folder.display()
            ));
        }
        Ok(contents)
    }

    /// Its mods named in `writers`, the mods built-in providers trust with
    /// privileged messages (acp's agent starts, git's commit and push, docs'
    /// saves, usage's Keychain and SSH): a package's mod of that name takes the place of
    /// the user's, and with it that right.
    pub(crate) fn writers<'a>(&'a self, writers: &'a [String]) -> impl Iterator<Item = &'a str> {
        self.mods
            .iter()
            .filter(|name| writers.contains(name))
            .map(String::as_str)
    }

    /// The providers that would run code with the user's access.
    pub(crate) fn trusted(&self) -> impl Iterator<Item = (&str, &Provider)> {
        self.providers
            .iter()
            .filter(|(_, how)| {
                matches!(
                    how,
                    Provider::Process(_)
                        | Provider::Socket(_)
                        | Provider::Source
                        | Provider::Native(_)
                )
            })
            .map(|(name, how)| (name.as_str(), how))
    }

    /// This package, then each one it ships beside it, with the requirement
    /// that names it.
    pub(crate) fn parts(&self) -> impl Iterator<Item = (Option<&str>, &Self)> {
        std::iter::once((None, self)).chain(
            self.beside
                .iter()
                .map(|(source, contents)| (Some(source.as_str()), contents)),
        )
    }
}

/// Files a package may hold that are not text: images, which a reviewer can
/// look at, known by how they start rather than by their name, so a
/// program cannot pass as `icon.png`.
const IMAGES: &[(&[&str], &[u8])] = &[
    (&["png"], b"\x89PNG\r\n\x1a\n"),
    (&["jpg", "jpeg"], b"\xff\xd8\xff"),
    (&["gif"], b"GIF8"),
    (&["webp"], b"RIFF"),
    (&["ico"], b"\0\0\x01\0"),
    (&["icns"], b"icns"),
];

/// Whether `bytes`, a file named with `extension`, are the image its name
/// says.
fn is_image(bytes: &[u8], extension: &str) -> bool {
    let Some((_, magic)) = IMAGES
        .iter()
        .find(|(extensions, _)| extensions.contains(&extension))
    else {
        return false;
    };
    bytes.starts_with(magic) && (extension != "webp" || bytes.get(8..12) == Some(b"WEBP"))
}

/// Refuses a package holding anything but source and images, such as a
/// prebuilt `provider.wasm` or an executable a provider's command could
/// run: everything a package runs must be readable, by its users and their
/// agents, before it is installed. A wasm provider ships its crate, which
/// the app builds. A link is refused too: what it points to, outside the
/// package, is not what was reviewed.
fn refuse_binaries(folder: &Path) -> Result<(), String> {
    let mut opaque: Vec<String> = files(folder)
        .into_iter()
        .filter_map(|relative| {
            let path = folder.join(&relative);
            let shown = printable(&relative.display().to_string());
            if path.symlink_metadata().is_ok_and(|meta| meta.is_symlink()) {
                return Some(format!("{shown} (a link)"));
            }
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase);
            if extension.as_deref() == Some("wasm") {
                return Some(shown);
            }
            let Ok(bytes) = fs::read(&path) else {
                return Some(shown);
            };
            let readable =
                is_text(&bytes) || extension.is_some_and(|extension| is_image(&bytes, &extension));
            (!readable).then_some(shown)
        })
        .collect();
    if opaque.is_empty() {
        return Ok(());
    }
    opaque.sort();
    Err(format!(
        "a package ships source only, so it can be reviewed before it runs; {} holds {}. \
         A wasm provider ships its crate (Cargo.toml and src/), which the app builds",
        folder.display(),
        opaque.join(", ")
    ))
}

/// Every file and link in `folder`, relative to it: in a git repository,
/// those it tracks and those it would (all of them, in a fresh clone), so a
/// linked folder's ignored build output does not count; elsewhere, every
/// one. A link is listed, not followed.
fn files(folder: &Path) -> Vec<PathBuf> {
    let listed = git(
        folder,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    );
    if let Ok(listed) = listed
        && folder.join(".git").exists()
    {
        return listed
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            // Deleted but not yet committed.
            .filter(|path| {
                folder
                    .join(path)
                    .symlink_metadata()
                    .is_ok_and(|meta| meta.is_file() || meta.is_symlink())
            })
            .collect();
    }
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let Ok(entries) = fs::read_dir(folder.join(&relative)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = relative.join(entry.file_name());
            match entry.file_type() {
                Ok(kind) if kind.is_dir() && entry.file_name() != ".git" => pending.push(path),
                Ok(kind) if kind.is_file() || kind.is_symlink() => found.push(path),
                _ => {},
            }
        }
    }
    found
}

/// Whether a file's `bytes` read as text: valid UTF-8 without NUL bytes.
fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

/// The entries of `dir` that `keep` accepts, by name.
fn names(dir: &Path, keep: impl Fn(&Path) -> bool) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| keep(path))
        .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .collect();
    found.sort();
    found
}

impl fmt::Display for Contents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = |names: &[String]| {
            names
                .iter()
                .map(|name| printable(name))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut parts = Vec::new();
        if let Some(app) = &self.app {
            parts.push(format!("the app {}, with its shell", printable(&app.name)));
            if !app.keep.is_empty() {
                parts.push(format!("built-in mods it keeps: {}", list(&app.keep)));
            }
        }
        if !self.mods.is_empty() {
            parts.push(format!("mods: {}", list(&self.mods)));
        }
        if !self.providers.is_empty() {
            let providers: Vec<String> = self
                .providers
                .iter()
                .map(|(name, how)| format!("{} ({how})", printable(name)))
                .collect();
            parts.push(format!("providers: {}", providers.join(", ")));
        }
        if !self.components.is_empty() {
            parts.push(format!("components: {}", list(&self.components)));
        }
        if self.style {
            parts.push("a stylesheet".to_owned());
        }
        if let Some(app) = self.app.as_ref().filter(|app| !app.requires.is_empty()) {
            parts.push(format!("requires: {}", list(&app.requires)));
        }
        for (source, contents) in &self.beside {
            parts.push(format!("ships {}, with {contents}", printable(source)));
        }
        f.write_str(&parts.join("; "))
    }
}

impl fmt::Display for Provider {
    /// Each argument of a command quoted on its own, so where one ends shows.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(command) => {
                let words: Vec<String> = command.iter().map(|word| format!("{word:?}")).collect();
                write!(f, "runs [{}] with your access", words.join(" "))
            },
            Self::Socket(path) => write!(f, "talks to the socket {}", printable(path)),
            Self::Source => f.write_str("built with Cargo, with your access"),
            Self::Native(settings) => write!(
                f,
                "replaces the app's built-in provider of this name, with settings {}",
                printable(settings)
            ),
            Self::Static(file) => write!(f, "publishes {}, running nothing", printable(file)),
            Self::Broken(problem) => write!(f, "broken: {}", printable(problem)),
        }
    }
}
