//! The `alacritty` engine: [`alacritty_terminal`]'s `Term`, fed by its VT
//! parser.

use {
    crate::{Ansi, Cell, Color, Emulator, Screen, Size, palette},
    alacritty_terminal::{
        Term,
        event::{Event, EventListener},
        grid::{Dimensions, Scroll},
        term::{Config, TermMode, cell::Flags, color::Colors},
        vte::ansi::{self, CursorShape, NamedColor, Processor},
    },
    std::sync::{Arc, Mutex},
};

/// Lines kept above the screen.
const SCROLLBACK: usize = 10_000;
/// Where the dim variants of the eight colors start among named colors.
const DIM_BLACK: usize = NamedColor::DimBlack as usize;
const DIM_WHITE: usize = NamedColor::DimWhite as usize;

pub(crate) struct Alacritty {
    term: Term<Listener>,
    parser: Processor,
    replies: Arc<Mutex<Vec<u8>>>,
    /// The theme's 16 colors, under any the shell sets.
    theme: Option<Ansi>,
}

/// Collects what the emulator wants written back to the shell.
#[derive(Clone)]
struct Listener(Arc<Mutex<Vec<u8>>>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event
            && let Ok(mut replies) = self.0.lock()
        {
            replies.extend_from_slice(text.as_bytes());
        }
    }
}

/// The grid size, as the emulator measures it.
struct Grid(Size);

impl Dimensions for Grid {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.0.rows)
    }

    fn columns(&self) -> usize {
        usize::from(self.0.cols)
    }
}

impl Alacritty {
    pub(crate) fn new(size: Size) -> Self {
        let replies = Arc::new(Mutex::new(Vec::new()));
        let config = Config {
            scrolling_history: SCROLLBACK,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &Grid(size), Listener(replies.clone())),
            parser: Processor::new(),
            replies,
            theme: None,
        }
    }
}

impl Emulator for Alacritty {
    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn resize(&mut self, size: Size) {
        self.term.resize(Grid(size));
    }

    fn scroll(&mut self, lines: i32) {
        // Alacritty counts up as positive.
        self.term.scroll_display(Scroll::Delta(-lines));
    }

    fn set_palette(&mut self, palette: Option<Ansi>) {
        self.theme = palette;
    }

    fn screen(&mut self) -> Screen {
        let cols = u16::try_from(self.term.columns()).unwrap_or(u16::MAX);
        let rows = u16::try_from(self.term.screen_lines()).unwrap_or(u16::MAX);
        let content = self.term.renderable_content();
        let offset = i32::try_from(content.display_offset).unwrap_or(0);
        let mut cells = vec![Cell::default(); usize::from(cols) * usize::from(rows)];
        let mut wrapped = vec![false; usize::from(rows)];
        for indexed in content.display_iter {
            let (Ok(row), column) = (
                usize::try_from(indexed.point.line.0 + offset),
                indexed.point.column.0,
            ) else {
                continue;
            };
            let Some(cell) = cells.get_mut(row * usize::from(cols) + column) else {
                continue;
            };
            let flags = indexed.cell.flags;
            if flags.contains(Flags::WRAPLINE) {
                wrapped[row] = true;
            }
            let (mut fg, mut bg) = (
                resolve(indexed.cell.fg, content.colors, self.theme.as_ref()),
                resolve(indexed.cell.bg, content.colors, self.theme.as_ref()),
            );
            if flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
                // Swapped defaults still need to look swapped.
                if fg == Color::Default {
                    fg = Color::Rgb(0, 0, 0);
                }
            }
            *cell = Cell {
                ch: if flags.contains(Flags::HIDDEN) {
                    ' '
                } else {
                    indexed.cell.c
                },
                fg,
                bg,
                bold: flags.contains(Flags::BOLD),
                italic: flags.contains(Flags::ITALIC),
                underline: flags.intersects(Flags::ALL_UNDERLINES),
                strike: flags.contains(Flags::STRIKEOUT),
                spacer: flags.contains(Flags::WIDE_CHAR_SPACER),
                wrap_spacer: flags.contains(Flags::LEADING_WIDE_CHAR_SPACER),
            };
        }
        let cursor = (content.mode.contains(TermMode::SHOW_CURSOR)
            && content.cursor.shape != CursorShape::Hidden
            && offset == 0)
            .then(|| {
                let point = content.cursor.point;
                (
                    u16::try_from(point.column.0).unwrap_or(0),
                    u16::try_from(point.line.0).unwrap_or(0),
                )
            });
        Screen {
            cols,
            rows,
            cells,
            wrapped,
            cursor,
            app_cursor: content.mode.contains(TermMode::APP_CURSOR),
            bracketed_paste: content.mode.contains(TermMode::BRACKETED_PASTE),
            exited: false,
            disconnected: false,
        }
    }

    fn replies(&mut self) -> Vec<u8> {
        self.replies
            .lock()
            .map(|mut replies| std::mem::take(&mut *replies))
            .unwrap_or_default()
    }
}

/// A cell color as RGB, or the theme's default: the shell may have changed
/// palette entries (`colors`), else the theme's, else xterm's.
fn resolve(color: ansi::Color, colors: &Colors, theme: Option<&Ansi>) -> Color {
    let indexed = |index: usize| {
        let rgb = colors[index]
            .map(|rgb| (rgb.r, rgb.g, rgb.b))
            .or_else(|| theme.and_then(|theme| theme.get(index).copied()))
            .unwrap_or_else(|| palette::rgb(u8::try_from(index).unwrap_or(0)));
        Color::Rgb(rgb.0, rgb.1, rgb.2)
    };
    match color {
        ansi::Color::Spec(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
        ansi::Color::Indexed(index) => indexed(usize::from(index)),
        ansi::Color::Named(named) => match named as usize {
            index @ 0..16 => indexed(index),
            index @ DIM_BLACK..=DIM_WHITE => indexed(index - DIM_BLACK),
            // Foreground, background, cursor: the theme's.
            _ => Color::Default,
        },
    }
}
