//! Where a terminal's program runs. A [`Backend`] opens a [`Pane`]: the
//! program's output as bytes, and a way to send it keys and a size. The
//! [`Session`](crate::Session) emulates those bytes whatever runs them, so a
//! shell on a local pseudo-terminal, a tmux session, or a herdr-core
//! terminal all paint the same way.

use {crate::Size, std::path::PathBuf};

/// What runs terminals: a local pseudo-terminal, tmux, herdr-core.
///
/// Opening may block briefly (it starts or reaches the program); every
/// backend is shared, so it is `Send + Sync`.
pub trait Backend: Send + Sync {
    /// Its short name, shown as a hint in the terminal's corner.
    fn name(&self) -> &'static str;

    /// The terminal kept as `open.key`: attached again when it still runs
    /// (a backend that outlives the app), started otherwise.
    fn open(&self, open: &Open) -> Result<Opened, String>;
}

/// An attached terminal: the program, and what it prints.
pub struct Opened {
    pub pane: Box<dyn Pane>,
    /// Read until the program ends or the connection drops.
    pub output: Output,
}

/// The terminal a view asks a backend for.
#[derive(Clone, Debug)]
pub struct Open {
    /// Stable across launches: the mod and the element's `id`.
    pub key: String,
    /// The folder a new one starts in; the home folder when `None`.
    pub cwd: Option<PathBuf>,
    pub size: Size,
    /// Variables a new one starts with, on top of the backend's own; one
    /// that is attached again keeps those it started with.
    pub env: Vec<(String, String)>,
}

/// One terminal's program, attached.
///
/// Dropping it lets go of the program: a pseudo-terminal's shell ends with
/// it, while tmux and herdr-core keep theirs running to attach again.
pub trait Pane: Send + Sync {
    /// Whether the program is confirmed to have ended. Asked on the reader
    /// thread after output ends; a lost connection alone must return false.
    /// May block while asking the backend. Unknown means disconnected, so
    /// the user can try attaching again instead of losing the tab.
    fn exited(&self) -> bool {
        false
    }

    /// Sends what the user typed or pasted.
    fn write(&self, bytes: &[u8]);

    /// Fits the program to `size`.
    fn resize(&self, size: Size);

    /// Whether the backend answers the program's queries itself (cursor
    /// position, device attributes), so the emulator's answers must not
    /// reach the program a second time.
    fn answers_queries(&self) -> bool {
        false
    }

    /// The folder its program is in now (the foreground program's, where
    /// the backend knows it), when the backend can tell. It may block
    /// briefly: it is asked off the window's thread.
    fn cwd(&self) -> Option<PathBuf> {
        None
    }
}

/// A pane's output: blocks for the next chunk, ending with the program or
/// its connection. [`Pane::exited`] distinguishes the two.
pub type Output = Box<dyn Iterator<Item = Vec<u8>> + Send>;
