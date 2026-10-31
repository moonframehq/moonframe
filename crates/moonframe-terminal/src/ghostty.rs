//! The `ghostty` engine: Ghostty's emulator through [`libghostty_vt`]. Its
//! types cannot leave the thread that made them, so a session makes it on
//! its emulator thread and keeps it there.

use {
    crate::{Ansi, Cell, Color, Emulator, Screen, Size},
    libghostty_vt::{
        Terminal,
        render::{CellIterator, RenderState, RowIterator},
        style::{RgbColor, Underline},
        terminal::{Mode, Options, ScrollViewport},
    },
    std::{cell::RefCell, rc::Rc},
};

/// Lines kept above the screen.
const SCROLLBACK: usize = 10_000;

pub(crate) struct Ghostty {
    terminal: Terminal<'static, 'static>,
    render: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
    replies: Rc<RefCell<Vec<u8>>>,
    size: Size,
}

impl Ghostty {
    pub(crate) fn new(size: Size) -> libghostty_vt::error::Result<Self> {
        let mut terminal = Terminal::new(Options {
            cols: size.cols,
            rows: size.rows,
            max_scrollback: SCROLLBACK,
        })?;
        terminal.resize(
            size.cols,
            size.rows,
            u32::from(size.cell_width),
            u32::from(size.cell_height),
        )?;
        // Replies to the shell's queries (cursor position, modes, ...).
        let replies = Rc::new(RefCell::new(Vec::new()));
        let sink = replies.clone();
        terminal.on_pty_write(move |_, data| sink.borrow_mut().extend_from_slice(data))?;
        Ok(Self {
            terminal,
            render: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
            replies,
            size,
        })
    }

    fn snapshot(&mut self) -> libghostty_vt::error::Result<Screen> {
        let (cols, rows) = (self.size.cols, self.size.rows);
        let app_cursor = self.terminal.mode(Mode::DECCKM)?;
        let bracketed_paste = self.terminal.mode(Mode::BRACKETED_PASTE)?;
        let snapshot = self.render.update(&self.terminal)?;
        let cursor = if snapshot.cursor_visible()? {
            snapshot
                .cursor_viewport()?
                .map(|cursor| (cursor.x, cursor.y))
        } else {
            None
        };
        let mut cells = Vec::with_capacity(usize::from(cols) * usize::from(rows));
        let mut text = String::new();
        let mut row_it = self.rows.update(&snapshot)?;
        while let Some(row) = row_it.next() {
            let mut cell_it = self.cells.update(row)?;
            let start = cells.len();
            while let Some(cell) = cell_it.next() {
                let mut out = Cell {
                    fg: color(cell.fg_color()?),
                    bg: color(cell.bg_color()?),
                    ..Cell::default()
                };
                if cell.graphemes_len()? > 0 {
                    cell.graphemes_utf8(&mut text)?;
                    out.ch = text.chars().next().unwrap_or(' ');
                }
                if cell.has_styling()? {
                    let style = cell.style()?;
                    out.bold = style.bold;
                    out.italic = style.italic;
                    out.underline = style.underline != Underline::None;
                    out.strike = style.strikethrough;
                    if style.invisible {
                        out.ch = ' ';
                    }
                    if style.inverse {
                        std::mem::swap(&mut out.fg, &mut out.bg);
                        if out.fg == Color::Default {
                            out.fg = Color::Rgb(0, 0, 0);
                        }
                    }
                }
                cells.push(out);
            }
            // Every row is `cols` wide, whatever the iterator gave.
            cells.resize(start + usize::from(cols), Cell::default());
        }
        cells.resize(usize::from(cols) * usize::from(rows), Cell::default());
        Ok(Screen {
            cols,
            rows,
            cells,
            cursor,
            app_cursor,
            bracketed_paste,
            exited: false,
        })
    }
}

impl Emulator for Ghostty {
    fn feed(&mut self, bytes: &[u8]) {
        self.terminal.vt_write(bytes);
    }

    fn resize(&mut self, size: Size) {
        match self.terminal.resize(
            size.cols,
            size.rows,
            u32::from(size.cell_width),
            u32::from(size.cell_height),
        ) {
            Ok(()) => self.size = size,
            Err(error) => tracing::warn!("libghostty cannot resize: {error}"),
        }
    }

    fn scroll(&mut self, lines: i32) {
        self.terminal
            .scroll_viewport(ScrollViewport::Delta(lines as isize));
    }

    fn set_palette(&mut self, palette: Option<Ansi>) {
        // The default layer: entries the shell set itself (OSC 4) stay.
        let result = self
            .terminal
            .set_default_color_palette(None)
            .and_then(|terminal| {
                let Some(palette) = palette else {
                    return Ok(terminal);
                };
                let mut colors = terminal.default_color_palette()?;
                for (slot, (r, g, b)) in colors.0.iter_mut().zip(palette) {
                    *slot = RgbColor { r, g, b };
                }
                terminal.set_default_color_palette(Some(colors))
            });
        if let Err(error) = result {
            tracing::warn!("libghostty cannot set the palette: {error}");
        }
    }

    fn screen(&mut self) -> Screen {
        self.snapshot().unwrap_or_else(|error| {
            tracing::warn!("libghostty cannot describe the screen: {error}");
            Screen::default()
        })
    }

    fn replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut *self.replies.borrow_mut())
    }
}

/// A resolved color, or the theme's default when the cell sets none.
fn color(rgb: Option<RgbColor>) -> Color {
    rgb.map_or(Color::Default, |rgb| Color::Rgb(rgb.r, rgb.g, rgb.b))
}
