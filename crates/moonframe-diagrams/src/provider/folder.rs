//! A folder of diagram files, opened once per scan and read through that
//! handle. On Unix each step under the project and every file is opened
//! relative to the step before it, never following a link, so a folder
//! swapped for a link after it was opened is not read through: its text
//! would be shared as `app.diagrams`.

use std::{
    ffi::OsString,
    fs::{File, Metadata},
    io,
    path::{Path, PathBuf},
};

pub(super) struct Folder {
    path: PathBuf,
    #[cfg(unix)]
    fd: rustix::fd::OwnedFd,
}

#[cfg(unix)]
mod open {
    use rustix::fs::{Mode, OFlags};

    /// A folder, to list and to open files and folders in.
    pub const FOLDER: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::CLOEXEC);
    /// A file of the folder's own: not through a link, and without
    /// waiting on a pipe put in its place (it is then refused as not a
    /// file).
    pub const FILE: OFlags = OFlags::RDONLY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::NOCTTY)
        .union(OFlags::CLOEXEC);
    pub const NONE: Mode = Mode::empty();
}

impl Folder {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// A plain file of this folder's own (a link, a pipe or a folder is
    /// refused), open, with what it was when opened.
    pub(super) fn open_file(&self, name: &std::ffi::OsStr) -> io::Result<(File, Metadata)> {
        let file = self.open_own(name)?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(io::Error::other("not a file"));
        }
        Ok((file, meta))
    }
}

#[cfg(unix)]
impl Folder {
    /// `dir`, the agents' folder (the app's own, so it may be a link).
    pub(super) fn open(dir: &Path) -> io::Result<Self> {
        let fd = rustix::fs::open(dir, open::FOLDER, open::NONE)?;
        Ok(Self {
            path: dir.to_owned(),
            fd,
        })
    }

    /// The project's `.moonframe/diagrams`, unless it (or `.moonframe`)
    /// is a link: each opened inside the one before.
    pub(super) fn project(project: &Path) -> Option<Self> {
        use rustix::fs::{OFlags, openat};
        let own = open::FOLDER | OFlags::NOFOLLOW;
        let root = rustix::fs::open(project, open::FOLDER, open::NONE).ok()?;
        let moonframe = openat(&root, ".moonframe", own, open::NONE).ok()?;
        let fd = openat(&moonframe, "diagrams", own, open::NONE).ok()?;
        Some(Self {
            path: project.join(".moonframe").join("diagrams"),
            fd,
        })
    }

    /// The names in the folder, at most `most`.
    pub(super) fn names(&self, most: usize) -> Vec<OsString> {
        use std::os::unix::ffi::OsStrExt as _;
        let Ok(listing) = rustix::fs::Dir::read_from(&self.fd) else {
            return Vec::new();
        };
        listing
            .map_while(Result::ok)
            .filter(|e| !matches!(e.file_name().to_bytes(), b"." | b".."))
            .map(|e| std::ffi::OsStr::from_bytes(e.file_name().to_bytes()).to_owned())
            .take(most)
            .collect()
    }

    fn open_own(&self, name: &std::ffi::OsStr) -> io::Result<File> {
        let fd = rustix::fs::openat(&self.fd, name, open::FILE, open::NONE)?;
        Ok(File::from(fd))
    }
}

#[cfg(not(unix))]
impl Folder {
    /// `dir`, the agents' folder (the app's own, so it may be a link).
    pub(super) fn open(dir: &Path) -> io::Result<Self> {
        if !std::fs::metadata(dir)?.is_dir() {
            return Err(io::Error::other("not a folder"));
        }
        Ok(Self {
            path: dir.to_owned(),
        })
    }

    /// The project's `.moonframe/diagrams`, unless it (or `.moonframe`)
    /// is a link, or leads out of the project.
    pub(super) fn project(project: &Path) -> Option<Self> {
        let dir = project.join(".moonframe").join("diagrams");
        let linked =
            |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
        if linked(&project.join(".moonframe")) || linked(&dir) {
            return None;
        }
        match (project.canonicalize(), dir.canonicalize()) {
            (Ok(root), Ok(inside)) if inside.starts_with(&root) => Some(Self { path: dir }),
            _ => None,
        }
    }

    /// The names in the folder, at most `most`.
    pub(super) fn names(&self, most: usize) -> Vec<OsString> {
        let Ok(listing) = std::fs::read_dir(&self.path) else {
            return Vec::new();
        };
        listing
            .flatten()
            .map(|e| e.file_name())
            .take(most)
            .collect()
    }

    /// Opened only when it was a file of its own just before, and the
    /// file opened is still that file.
    fn open_own(&self, name: &std::ffi::OsStr) -> io::Result<File> {
        let path = self.path.join(name);
        let looked = std::fs::symlink_metadata(&path)?;
        if !looked.file_type().is_file() {
            return Err(io::Error::other("not a file"));
        }
        let file = File::open(&path)?;
        let opened = file.metadata()?;
        if looked.len() != opened.len() || looked.modified().ok() != opened.modified().ok() {
            return Err(io::Error::other("changed while it was read"));
        }
        Ok(file)
    }
}
