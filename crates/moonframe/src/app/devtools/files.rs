//! The Luau files the Developer tools read and save, kept inside the app's
//! folders. A file's folder is opened once, then checked to be inside them
//! by what it is, not only by its name: the folder found at its resolved
//! path must be the very one opened. The file is then read, or written and
//! renamed into place, relative to that open folder and never through a
//! link, so a link swapped in after the check cannot lead out. Elsewhere
//! than macOS and Linux the check is by resolved path.

use {
    crate::paths,
    std::{
        ffi::OsStr,
        path::{Path, PathBuf},
    },
};

/// The text of `file`, or of the user's copy of it when there is one, as
/// that is what the app runs. `roots` are the folders it may be in.
pub(super) fn read(file: &Path, roots: &[PathBuf]) -> Result<String, String> {
    let file = user_copy(file)
        .filter(|copy| copy.symlink_metadata().is_ok_and(|meta| meta.is_file()))
        .unwrap_or_else(|| file.to_path_buf());
    let (folder, name) = split(&file)?;
    platform::read(folder, name, &canonical(roots))
}

/// Writes `text` to `file`, or to the user's copy of it when it ships in
/// the signed app, which that copy then replaces. A temporary file and a
/// rename, so the window never reloads half a file. Returns where it went.
pub(super) fn save(file: &Path, text: &str, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let target = user_copy(file).unwrap_or_else(|| file.to_path_buf());
    let (folder, name) = split(&target)?;
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    platform::save(folder, name, text, &canonical(roots))?;
    tracing::info!("devtools saved {}", target.display());
    Ok(target)
}

/// Where the user's copy of `file` goes when it ships in the signed app.
fn user_copy(file: &Path) -> Option<PathBuf> {
    let folder = &moonframe_providers::installed()?.folder;
    Some(folder.join(paths::overlay(file)?))
}

fn split(file: &Path) -> Result<(&Path, &OsStr), String> {
    match (file.parent(), file.file_name()) {
        (Some(folder), Some(name)) => Ok((folder, name)),
        _ => Err("that is not a file in a folder".to_owned()),
    }
}

/// The roots as they resolve now; one that does not exist holds nothing.
fn canonical(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .collect()
}

/// `folder` resolved, when that is inside one of `roots`.
fn inside(folder: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let resolved = folder
        .canonicalize()
        .map_err(|error| format!("cannot find its folder: {error}"))?;
    if roots.iter().any(|root| resolved.starts_with(root)) {
        Ok(resolved)
    } else {
        Err("that file is not in the app's folders".to_owned())
    }
}

fn too_big() -> String {
    format!("it is over {} bytes", moonframe_core::MAX_SOURCE)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod platform {
    use {
        rustix::fs::{self, AtFlags, FileType, Mode, OFlags},
        std::{
            ffi::{OsStr, OsString},
            io::{Read, Write},
            os::fd::OwnedFd,
            path::{Path, PathBuf},
        },
    };

    fn text(error: impl std::fmt::Display) -> String {
        error.to_string()
    }

    /// `folder`, opened, once it is known to be the folder inside `roots`
    /// its resolved path names: what is done through it stays there.
    fn open_folder(folder: &Path, roots: &[PathBuf]) -> Result<OwnedFd, String> {
        let opened = fs::open(
            folder,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(text)?;
        let resolved = super::inside(folder, roots)?;
        let (held, there) = (
            fs::fstat(&opened).map_err(text)?,
            fs::stat(&resolved).map_err(text)?,
        );
        if (held.st_dev, held.st_ino) != (there.st_dev, there.st_ino) {
            return Err("its folder moved while it was opened".to_owned());
        }
        Ok(opened)
    }

    pub(super) fn read(folder: &Path, name: &OsStr, roots: &[PathBuf]) -> Result<String, String> {
        let folder = open_folder(folder, roots)?;
        let file = fs::openat(
            &folder,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(text)?;
        let stat = fs::fstat(&file).map_err(text)?;
        if !FileType::from_raw_mode(stat.st_mode).is_file() {
            return Err("that is not a file".to_owned());
        }
        if stat.st_size > moonframe_core::MAX_SOURCE as i64 {
            return Err(super::too_big());
        }
        let mut source = String::new();
        std::fs::File::from(file)
            .read_to_string(&mut source)
            .map_err(text)?;
        Ok(source)
    }

    pub(super) fn save(
        folder: &Path,
        name: &OsStr,
        source: &str,
        roots: &[PathBuf],
    ) -> Result<(), String> {
        let folder = open_folder(folder, roots)?;
        let mut temporary = OsString::from(".");
        temporary.push(name);
        temporary.push(".devtools");
        // One a failed save left behind.
        let _ = fs::unlinkat(&folder, &temporary, AtFlags::empty());
        let saved = fs::openat(
            &folder,
            &temporary,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
        )
        .map_err(text)
        .and_then(|file| {
            std::fs::File::from(file)
                .write_all(source.as_bytes())
                .map_err(text)
        })
        .and_then(|()| fs::renameat(&folder, &temporary, &folder, name).map_err(text));
        if saved.is_err() {
            let _ = fs::unlinkat(&folder, &temporary, AtFlags::empty());
        }
        saved
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use std::{
        ffi::OsStr,
        path::{Path, PathBuf},
    };

    pub(super) fn read(folder: &Path, name: &OsStr, roots: &[PathBuf]) -> Result<String, String> {
        let file = super::inside(folder, roots)?.join(name);
        let meta = file.symlink_metadata().map_err(|error| error.to_string())?;
        if !meta.is_file() {
            return Err("that is not a file".to_owned());
        }
        if meta.len() > moonframe_core::MAX_SOURCE as u64 {
            return Err(super::too_big());
        }
        std::fs::read_to_string(&file).map_err(|error| error.to_string())
    }

    pub(super) fn save(
        folder: &Path,
        name: &OsStr,
        source: &str,
        roots: &[PathBuf],
    ) -> Result<(), String> {
        let folder = super::inside(folder, roots)?;
        let mut temporary = std::ffi::OsString::from(".");
        temporary.push(name);
        temporary.push(".devtools");
        let temporary = folder.join(temporary);
        std::fs::write(&temporary, source)
            .and_then(|()| std::fs::rename(&temporary, folder.join(name)))
            .map_err(|error| {
                let _ = std::fs::remove_file(&temporary);
                error.to_string()
            })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{read, save};

    #[test]
    fn a_save_replaces_the_whole_file_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let roots = [dir.path().to_owned()];
        let file = dir.path().join("mods/a/main.luau");
        let written = save(&file, "return mod {}", &roots).unwrap();
        assert_eq!(written, file);
        assert_eq!(read(&file, &roots).unwrap(), "return mod {}");
        save(&file, "return mod { title = \"A\" }", &roots).unwrap();
        assert_eq!(read(&file, &roots).unwrap(), "return mod { title = \"A\" }");
        // No temporary file is left beside it.
        let names: Vec<_> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn nothing_outside_the_roots_is_read_or_written() {
        let app = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let roots = [app.path().to_owned()];
        let secret = outside.path().join("secret.luau");
        std::fs::write(&secret, "secret").unwrap();
        assert!(read(&secret, &roots).is_err());
        assert!(save(&secret, "changed", &roots).is_err());
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "secret");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_the_roots_does_not_lead_out() {
        let app = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let roots = [app.path().to_owned()];
        std::fs::write(outside.path().join("main.luau"), "secret").unwrap();
        // A file linked out, and a folder linked out.
        let file = app.path().join("main.luau");
        std::os::unix::fs::symlink(outside.path().join("main.luau"), &file).unwrap();
        std::os::unix::fs::symlink(outside.path(), app.path().join("mods")).unwrap();
        assert!(read(&file, &roots).is_err());
        let through = app.path().join("mods/main.luau");
        assert!(read(&through, &roots).is_err());
        assert!(save(&through, "changed", &roots).is_err());
        assert_eq!(
            std::fs::read_to_string(outside.path().join("main.luau")).unwrap(),
            "secret"
        );
    }
}
