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
    /// Read by the session's reader thread until the program ends.
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
}

/// One terminal's program, attached.
///
/// Dropping it lets go of the program: a pseudo-terminal's shell ends with
/// it, while tmux and herdr-core keep theirs running to attach again.
pub trait Pane: Send + Sync {
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
}

/// A pane's output: blocks for the next chunk, and ends with the program.
pub type Output = Box<dyn Iterator<Item = Vec<u8>> + Send>;
