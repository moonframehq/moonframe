//! Terminals for moonframe: a program a [`Backend`] runs (a shell on a
//! local [`Pty`] here; tmux and herdr-core in `moonframe-mux`), read by a
//! terminal emulator, and published as [`Screen`] snapshots for the window
//! to paint. Two emulators sit behind one [`Emulator`] interface, each
//! behind a feature (both on by default):
//!
//! - **`alacritty`**: [`alacritty_terminal`], pure Rust.
//! - **`ghostty`**: [`libghostty_vt`], Ghostty's emulator, built from
//!   Ghostty's source with Zig.
//!
//! A [`Session`] runs its emulator on a thread of its own (libghostty's
//! types cannot leave the thread that made them); the window only ever sees
//! the latest snapshot, and writes keys straight to the shell.

#[cfg(feature = "alacritty")]
mod alacritty;
#[cfg(feature = "ghostty")]
mod ghostty;
mod keys;
#[cfg(feature = "alacritty")]
mod palette;
mod pane;
mod pty;
mod session;

pub use {
    keys::{Key, encode, paste},
    pane::{Backend, Open, Opened, Output, Pane},
    pty::Pty,
    session::{Folder, Session},
};

/// Which emulator reads the shell's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Engine {
    Alacritty,
    Ghostty,
}

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            Self::Alacritty => "alacritty",
            Self::Ghostty => "ghostty",
        }
    }

    /// Whether this build includes it.
    pub fn available(self) -> bool {
        match self {
            Self::Alacritty => cfg!(feature = "alacritty"),
            Self::Ghostty => cfg!(feature = "ghostty"),
        }
    }

    /// A new emulator of this kind; `None` when the build leaves it out.
    fn emulator(self, size: Size) -> Option<Box<dyn Emulator>> {
        match self {
            #[cfg(feature = "alacritty")]
            Self::Alacritty => Some(Box::new(alacritty::Alacritty::new(size))),
            #[cfg(feature = "ghostty")]
            Self::Ghostty => ghostty::Ghostty::new(size)
                .inspect_err(|error| tracing::warn!("cannot start libghostty: {error}"))
                .ok()
                .map(|emulator| Box::new(emulator) as Box<dyn Emulator>),
            #[allow(
                unreachable_patterns,
                reason = "every engine is matched when all are built"
            )]
            _ => None,
        }
    }
}

/// The grid in cells, and a cell's size in pixels for programs that ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl Default for Size {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            cell_width: 8,
            cell_height: 16,
        }
    }
}

/// The 16 named colors a theme gives the terminal, as RGB.
pub type Ansi = [(u8, u8, u8); 16];

/// A color a cell asks for; `Default` means the theme's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Color {
    #[default]
    Default,
    Rgb(u8, u8, u8),
}

/// One cell of the screen, its colors already resolved (inverse applied).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// The right half of a wide character drawn by the cell before it.
    pub spacer: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            spacer: false,
        }
    }
}

/// What the window paints: the visible grid, the cursor, and the modes
/// that change what keys send.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Screen {
    pub cols: u16,
    pub rows: u16,
    /// Row after row.
    pub cells: Vec<Cell>,
    /// Column and row, when it shows.
    pub cursor: Option<(u16, u16)>,
    /// Arrow keys send application sequences (DECCKM).
    pub app_cursor: bool,
    /// Pastes are wrapped so the shell knows they are not typed.
    pub bracketed_paste: bool,
    /// The shell has exited.
    pub exited: bool,
    /// Output ended without a confirmed shell exit; Enter attaches again.
    pub disconnected: bool,
}

impl Screen {
    pub fn row(&self, row: u16) -> &[Cell] {
        let start = usize::from(row) * usize::from(self.cols);
        self.cells
            .get(start..start + usize::from(self.cols))
            .unwrap_or(&[])
    }
}

/// A terminal emulator: reads what the shell prints, answers its queries,
/// and describes the screen.
trait Emulator {
    fn feed(&mut self, bytes: &[u8]);
    fn resize(&mut self, size: Size);
    /// Lines up (negative) or down through the scrollback.
    fn scroll(&mut self, lines: i32);
    /// The theme's 16 colors, or xterm's when `None`. Colors the shell set
    /// itself (OSC 4) stay.
    fn set_palette(&mut self, palette: Option<Ansi>);
    fn screen(&mut self) -> Screen;
    /// What the emulator must send back to the shell, such as replies to
    /// device queries.
    fn replies(&mut self) -> Vec<u8>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
