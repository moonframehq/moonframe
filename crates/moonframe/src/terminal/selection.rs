//! Text selected in a terminal's visible screen: a drag from one cell to
//! another, a double-clicked word, or a triple-clicked line, read back as
//! the text a terminal would copy. Pure: cells in, ranges and text out.

use {moonframe_terminal::Screen, std::ops::Range};

/// A cell of the visible screen, column and row from the top left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Spot {
    pub(crate) row: u16,
    pub(crate) column: u16,
}

/// From where a press started to where the pointer is, either way round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    anchor: Spot,
    head: Spot,
    /// A double- or triple-clicked word or line, which selects its cells
    /// even when it is one cell wide.
    picked: bool,
}

impl Selection {
    /// A press of `clicks` at `at`: one selects nothing until the pointer
    /// moves, two the word under it, three its line.
    pub(crate) fn press(screen: &Screen, at: Spot, clicks: usize) -> Self {
        match clicks {
            2 => Self::word(screen, at),
            count if count >= 3 => Self::line(screen, at),
            _ => Self::at(at),
        }
    }

    fn at(at: Spot) -> Self {
        Self {
            anchor: at,
            head: at,
            picked: false,
        }
    }

    /// The run of non-blank cells around `at`.
    fn word(screen: &Screen, at: Spot) -> Self {
        let cells = screen.row(at.row);
        let blank = |column: u16| {
            cells
                .get(usize::from(column))
                .is_none_or(|cell| !cell.spacer && cell.ch.is_whitespace())
        };
        if blank(at.column) {
            return Self::at(at);
        }
        let start = (0..at.column)
            .rev()
            .take_while(|column| !blank(*column))
            .last()
            .unwrap_or(at.column);
        let end = (at.column..screen.cols)
            .take_while(|column| !blank(*column))
            .last()
            .unwrap_or(at.column);
        Self {
            anchor: Spot {
                row: at.row,
                column: start,
            },
            head: Spot {
                row: at.row,
                column: end,
            },
            picked: true,
        }
    }

    fn line(screen: &Screen, at: Spot) -> Self {
        Self {
            anchor: Spot {
                row: at.row,
                column: 0,
            },
            head: Spot {
                row: at.row,
                column: screen.cols.saturating_sub(1),
            },
            picked: true,
        }
    }

    pub(crate) fn extend(&mut self, to: Spot) {
        self.head = to;
    }

    /// Whether this is a press whose pointer never left the cell it
    /// pressed, rather than a word or line, however short.
    pub(crate) fn is_empty(&self) -> bool {
        !self.picked && self.anchor == self.head
    }

    fn ordered(&self) -> (Spot, Spot) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// Each selected row's columns, as a terminal selects: to the end of
    /// the first row, whole rows between, and the last row up to the head.
    pub(crate) fn rows(&self, width: u16) -> Vec<(u16, Range<u16>)> {
        if self.is_empty() || width == 0 {
            return Vec::new();
        }
        let (start, end) = self.ordered();
        (start.row..=end.row)
            .map(|row| {
                let from = if row == start.row {
                    start.column
                } else {
                    0
                };
                let to = if row == end.row {
                    end.column.saturating_add(1).min(width)
                } else {
                    width
                };
                (row, from.min(to)..to)
            })
            .collect()
    }

    /// The selected text: each row's cells with trailing blanks dropped,
    /// rows joined by newlines. A wide character's spacer is copied once.
    pub(crate) fn text(&self, screen: &Screen) -> String {
        self.rows(screen.cols)
            .into_iter()
            .filter(|(row, _)| *row < screen.rows)
            .map(|(row, columns)| {
                let cells = screen.row(row);
                let text: String = columns
                    .filter_map(|column| cells.get(usize::from(column)))
                    .filter(|cell| !cell.spacer)
                    .map(|cell| cell.ch)
                    .collect();
                text.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use {super::*, moonframe_terminal::Cell};

    fn screen(lines: &[&str]) -> Screen {
        let cols = lines.iter().map(|line| line.len()).max().unwrap_or(0) as u16;
        let cells = lines
            .iter()
            .flat_map(|line| {
                let mut row: Vec<Cell> = line
                    .chars()
                    .map(|ch| Cell {
                        ch,
                        ..Cell::default()
                    })
                    .collect();
                row.resize(usize::from(cols), Cell::default());
                row
            })
            .collect();
        Screen {
            cols,
            rows: lines.len() as u16,
            cells,
            ..Screen::default()
        }
    }

    fn spot(row: u16, column: u16) -> Spot {
        Spot { row, column }
    }

    #[test]
    fn a_click_selects_nothing() {
        let screen = screen(&["hello"]);
        let selection = Selection::press(&screen, spot(0, 1), 1);
        assert!(selection.is_empty());
        assert_eq!(selection.text(&screen), "");
    }

    #[test]
    fn a_drag_selects_across_rows_either_way() {
        let screen = screen(&["hello world", "second line"]);
        let mut selection = Selection::press(&screen, spot(1, 5), 1);
        selection.extend(spot(0, 6));
        assert_eq!(selection.text(&screen), "world\nsecond");
    }

    #[test]
    fn a_double_click_selects_the_word() {
        let screen = screen(&["cargo build --release"]);
        let selection = Selection::press(&screen, spot(0, 8), 2);
        assert_eq!(selection.text(&screen), "build");
    }

    #[test]
    fn a_triple_click_selects_the_line_without_trailing_blanks() {
        let screen = screen(&["short", "a longer line"]);
        let selection = Selection::press(&screen, spot(0, 2), 3);
        assert_eq!(selection.text(&screen), "short");
    }
}
