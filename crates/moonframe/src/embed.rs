//! App folders compiled into the binary, laid out on disk where the layers
//! can read them: the toolbox (the starter's `app/`, whose components,
//! providers and stylesheet every app built on Moonframe stands on) and,
//! when a host embeds it, the host's own app folder.
//!
//! Each is written once per content, to `~/.cache/<name>/<label>/<hash>`,
//! so a new build brings its own files and an unchanged one reuses them.

use {
    crate::{launch::AppSource, paths},
    rust_embed::RustEmbed,
    std::{
        fmt::Write as _,
        fs, io,
        path::{Path, PathBuf},
    },
};

/// The starter's app folder: the bundle's, else `app` in the current
/// folder (running from the repository), else the toolbox.
pub(crate) fn builtin() -> PathBuf {
    if let Some(app) = paths::resources().map(|folder| folder.join("app")) {
        return app;
    }
    let here = PathBuf::from("app");
    if here.join("main.luau").is_file() {
        return here;
    }
    toolbox_folder()
}

/// What an app on the library stands on: the toolbox its bundle ships,
/// else this checkout's `app/` (edits show live), else the copy compiled
/// into the binary, written to the cache.
pub(crate) fn toolbox_folder() -> PathBuf {
    if let Some(toolbox) = paths::resources()
        .map(|folder| folder.join("toolbox"))
        .filter(|toolbox| toolbox.is_dir())
    {
        return toolbox;
    }
    if let Some(source) = toolbox_source() {
        return source;
    }
    toolbox(&paths::cache())
        .inspect_err(|error| tracing::warn!("cannot write the toolbox: {error}"))
        .unwrap_or_else(|_| PathBuf::from("app"))
}

/// An app on the library's own folder: its bundle's, else where `source`
/// says.
pub(crate) fn host_app(source: &AppSource) -> PathBuf {
    if let Some(app) = paths::resources().map(|folder| folder.join("app")) {
        return app;
    }
    match source {
        AppSource::Dir(dir) => dir.clone(),
        AppSource::Embedded(write) => write(&paths::cache()).unwrap_or_else(|error| {
            tracing::warn!("cannot write the app's files: {error}");
            PathBuf::from("app")
        }),
    }
}

/// The starter's app folder: what a host stands on when it runs from
/// neither a bundle nor a checkout of this repository.
#[derive(RustEmbed)]
#[folder = "../../app"]
#[exclude = "providers/*/target/*"]
#[exclude = "providers/*/Cargo.lock"]
#[exclude = ".DS_Store"]
#[exclude = "*/.DS_Store"]
pub(crate) struct Toolbox;

/// The toolbox's folder in this checkout, when the binary runs where it
/// was built: edits to it then show live.
pub(crate) fn toolbox_source() -> Option<PathBuf> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    source
        .join("components")
        .is_dir()
        .then(|| source.canonicalize().unwrap_or(source))
}

/// The interface wasm providers build against. Their crates name it as
/// `../../../wit/provider.wit`, outside the app folder, so it is written
/// there beside the toolbox.
const WIT: &str = include_str!("../../../wit/provider.wit");

/// Writes the toolbox under `cache/toolbox/<hash>`, with the interface its
/// wasm providers build against where they look for it.
pub(crate) fn toolbox(cache: &Path) -> io::Result<PathBuf> {
    let folder = materialize::<Toolbox>(cache, "toolbox")?;
    let wit = cache.join("toolbox/wit/provider.wit");
    if fs::read_to_string(&wit).ok().as_deref() != Some(WIT) {
        if let Some(parent) = wit.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&wit, WIT)?;
    }
    Ok(folder)
}

/// Marks a folder written whole: one without it was cut short, and is
/// written again.
const DONE: &str = ".moonframe-embedded";

/// Writes `E`'s files under `cache/<label>/<hash>`, unless they are there
/// already, and returns that folder.
pub(crate) fn materialize<E: RustEmbed>(cache: &Path, label: &str) -> io::Result<PathBuf> {
    let mut names: Vec<String> = E::iter().map(|name| name.into_owned()).collect();
    names.sort();
    let mut hasher = Fnv::default();
    for name in &names {
        hasher.write(name.as_bytes());
        if let Some(file) = E::get(name) {
            hasher.write(&file.metadata.sha256_hash());
        }
    }
    let folder = cache.join(label).join(hasher.hex());
    if folder.join(DONE).is_file() {
        return Ok(folder);
    }
    // Written beside the folder, then moved into place: another process
    // starting at once finds the whole folder or none.
    let partial = cache
        .join(label)
        .join(format!(".{}-{}", hasher.hex(), std::process::id()));
    if partial.exists() {
        fs::remove_dir_all(&partial)?;
    }
    for name in &names {
        let Some(file) = E::get(name) else {
            continue;
        };
        let path = partial.join(safe(name)?);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.data)?;
        #[cfg(unix)]
        if name.ends_with(".sh") {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        }
    }
    fs::create_dir_all(&partial)?;
    fs::write(partial.join(DONE), "")?;
    match fs::rename(&partial, &folder) {
        Ok(()) => Ok(folder),
        // Another process put it there first.
        Err(_) if folder.join(DONE).is_file() => {
            let _ = fs::remove_dir_all(&partial);
            Ok(folder)
        },
        Err(error) => Err(error),
    }
}

/// `name` as a relative path that stays inside the folder.
fn safe(name: &str) -> io::Result<PathBuf> {
    let path = Path::new(name);
    let plain = path
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)));
    if plain {
        Ok(path.to_owned())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("an embedded file outside its folder: {name}"),
        ))
    }
}

/// FNV-1a over 64 bits: enough to tell one build's files from another's.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
        // Between fields, so `ab` + `c` differs from `a` + `bc`.
        self.0 ^= 0xff;
        self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
    }

    fn hex(&self) -> String {
        let mut text = String::new();
        let _ = write!(text, "{:016x}", self.0);
        text
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_toolbox_is_written_once_and_whole() {
        let cache = tempfile::tempdir().unwrap();
        let folder = toolbox(cache.path()).unwrap();
        assert!(folder.join("main.luau").is_file());
        // Where the system provider's crate looks for it.
        assert!(
            folder
                .join("providers/system/../../../wit/provider.wit")
                .is_file()
        );
        assert!(folder.join("style.luau").is_file());
        assert!(folder.join("components/button-group.luau").is_file());
        assert!(folder.join("providers/git/provider.toml").is_file());
        assert!(!folder.join("providers/system/target").exists());
        // The same files: the same folder, left as it is.
        let marker = folder.join("components/.kept");
        fs::write(&marker, "").unwrap();
        assert_eq!(
            materialize::<Toolbox>(cache.path(), "toolbox").unwrap(),
            folder
        );
        assert!(marker.is_file());
    }

    #[test]
    fn names_never_leave_the_folder() {
        assert!(safe("mods/a/main.luau").is_ok());
        assert!(safe("../evil").is_err());
        assert!(safe("/etc/passwd").is_err());
    }
}
