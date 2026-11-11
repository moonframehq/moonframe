//! The app as an agent changes it: which folders the window runs on, the
//! one an agent writes in, where it works, and the command that checks the
//! app. The host sets it once at launch, as it alone knows its folders:
//! a provider's own folder may be the toolbox's, under another app.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// What an agent needs to know to change the running app.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Editable {
    /// The app's name for people.
    pub title: String,
    /// The folders the window runs on, lowest first, each with what it is:
    /// `toolbox`, `package`, `app`, `user`.
    pub layers: Vec<(String, PathBuf)>,
    /// The app's own folder: its `main.luau`, mods and style.
    pub app: PathBuf,
    /// Where an agent writes: `app` when it is someone's source, else the
    /// user's folder laid over it, whose files win at the same path.
    pub writable: PathBuf,
    /// Where an agent works: the repository holding `writable`, else it.
    pub workdir: PathBuf,
    /// The command that loads the app headlessly, as the window does, and
    /// reports problems; a shell runs it.
    pub check: String,
    /// What the host adds about changing it (`Moonframe::agent_guide`).
    pub guide: Option<String>,
    /// Where the app keeps state of its own, its profile's if it has one.
    pub state: Option<PathBuf>,
}

impl Editable {
    /// Whether an agent writing in `writable` writes over another folder's
    /// files rather than in the app's own source.
    pub fn overlay(&self) -> bool {
        self.writable != self.app
    }

    /// The toolbox under the app: the components it may copy and change.
    pub fn toolbox(&self) -> Option<&Path> {
        self.layers
            .iter()
            .find(|(role, _)| role == "toolbox")
            .map(|(_, path)| path.as_path())
    }
}

static EDITABLE: OnceLock<Editable> = OnceLock::new();

/// Says what an agent changing this app needs to know; set once, at launch.
pub fn set_editable(editable: Editable) {
    let _ = EDITABLE.set(editable);
}

/// The app as an agent changes it, once the host said.
pub fn editable() -> Option<&'static Editable> {
    EDITABLE.get()
}

/// The git repository holding `folder`, if any.
pub fn repository(folder: &Path) -> Option<PathBuf> {
    let folder = folder.canonicalize().ok()?;
    folder
        .ancestors()
        .find(|folder| folder.join(".git").exists())
        .map(Path::to_owned)
}
