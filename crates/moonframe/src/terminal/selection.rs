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

/// A cell, word or line, with inclusive ends in screen order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Span {
    start: Spot,
    end: Spot,
}

impl Span {
    fn at(at: Spot) -> Self {
        Self { start: at, end: at }
    }

    /// The run of non-blank cells around `at`, carried on across rows the
    /// terminal soft-wrapped, so a long URL or path is picked whole.
    fn word(screen: &Screen, at: Spot) -> Self {
        let blank = |spot: Spot| {
            screen
                .row(spot.row)
                .get(usize::from(spot.column))
                .is_none_or(|cell| !cell.spacer && !cell.wrap_spacer && cell.ch.is_whitespace())
        };
        if blank(at) {
            return Self::at(at);
        }
        let mut start = at;
        while let Some(before) = before(screen, start).filter(|spot| !blank(*spot)) {
            start = before;
        }
        let mut end = at;
        while let Some(after) = after(screen, end).filter(|spot| !blank(*spot)) {
            end = after;
        }
        Self { start, end }
    }

    fn line(screen: &Screen, at: Spot) -> Self {
        Self {
            start: Spot {
                row: at.row,
                column: 0,
            },
            end: Spot {
                row: at.row,
                column: screen.cols.saturating_sub(1),
            },
        }
    }
}

/// The cell before `spot` in reading order, through a soft wrap but not
/// a hard line break.
fn before(screen: &Screen, spot: Spot) -> Option<Spot> {
    if spot.column > 0 {
        return Some(Spot {
            column: spot.column - 1,
            ..spot
        });
    }
    let row = spot.row.checked_sub(1)?;
    screen.is_wrapped(row).then(|| Spot {
        row,
        column: screen.cols.saturating_sub(1),
    })
}

/// The cell after `spot` in reading order, through a soft wrap but not a
/// hard line break.
fn after(screen: &Screen, spot: Spot) -> Option<Spot> {
    if spot.column + 1 < screen.cols {
        return Some(Spot {
            column: spot.column + 1,
            ..spot
        });
    }
    (screen.is_wrapped(spot.row) && spot.row + 1 < screen.rows).then(|| Spot {
        row: spot.row + 1,
        column: 0,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    // keep-sorted start
    Cell,
    Line,
    Word,
    // keep-sorted end
}

impl Mode {
    fn span(self, screen: &Screen, at: Spot) -> Span {
        match self {
            Self::Cell => Span::at(at),
            Self::Line => Span::line(screen, at),
            Self::Word => Span::word(screen, at),
        }
    }
}

/// The initial cell, word or line and the one under the pointer. The
/// initial span stays selected as the pointer moves in either direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    anchor: Span,
    head: Span,
    mode: Mode,
}

impl Selection {
    /// A press of `clicks` at `at`: one selects nothing until the pointer
    /// moves, two the word under it, three its line.
    pub(crate) fn press(screen: &Screen, at: Spot, clicks: usize) -> Self {
        let mode = match clicks {
            2 => Mode::Word,
            count if count >= 3 => Mode::Line,
            _ => Mode::Cell,
        };
        let anchor = mode.span(screen, at);
        Self {
            anchor,
            head: anchor,
            mode,
        }
    }

    pub(crate) fn extend(&mut self, screen: &Screen, to: Spot) {
        self.head = self.mode.span(screen, to);
    }

    /// Whether this is a press whose pointer never left the cell it
    /// pressed, rather than a word or line, however short.
    pub(crate) fn is_empty(&self) -> bool {
        self.mode == Mode::Cell && self.anchor == self.head
    }

    fn ordered(&self) -> (Spot, Spot) {
        (
            self.anchor.start.min(self.head.start),
            self.anchor.end.max(self.head.end),
        )
    }

    /// Each selected row's columns, as a terminal selects: to the end of
    /// the first row, whole rows between, and the last row up to the head.
    pub(crate) fn rows(&self, screen: &Screen) -> Vec<(u16, Range<u16>)> {
        let width = screen.cols;
        if self.is_empty() || width == 0 || screen.rows == 0 {
            return Vec::new();
        }
        let (start, end) = self.ordered();
        (start.row..=end.row.min(screen.rows - 1))
            .map(|row| {
                let from = if row == start.row {
                    start.column
                } else {
                    0
                };
                let mut to = if row == end.row {
                    end.column.saturating_add(1).min(width)
                } else {
                    width
                };
                let mut from = from.min(to);
                let cells = screen.row(row);
                // Both copying and painting include the whole wide glyph,
                // regardless of which half either end of the drag hit.
                if from < to {
                    if cells.get(usize::from(from)).is_some_and(|cell| cell.spacer) {
                        from = from.saturating_sub(1);
                    }
                    if to < width && cells.get(usize::from(to)).is_some_and(|cell| cell.spacer) {
                        to += 1;
                    }
                }
                (row, from..to)
            })
            .collect()
    }

    /// Soft-wrapped rows join without a newline or trimming real spaces.
    /// Hard line endings lose trailing blanks; wide spacers are never text.
    pub(crate) fn text(&self, screen: &Screen) -> String {
        let mut text = String::new();
        let mut rows = self.rows(screen).into_iter().peekable();
        while let Some((row, columns)) = rows.next() {
            let cells = screen.row(row);
            let line: String = columns
                .filter_map(|column| cells.get(usize::from(column)))
                .filter(|cell| !cell.spacer && !cell.wrap_spacer)
                .map(|cell| cell.ch)
                .collect();
            let continues = rows.peek().is_some();
            if continues && screen.is_wrapped(row) {
                text.push_str(&line);
            } else {
                text.push_str(line.trim_end());
                if continues {
                    text.push('\n');
                }
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use {super::*, moonframe_terminal::Cell};

    fn screen(lines: &[&str]) -> Screen {
        let cols = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0) as u16;
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
        selection.extend(&screen, spot(0, 6));
        assert_eq!(selection.text(&screen), "world\nsecond");
    }

    #[test]
    fn a_double_click_selects_the_word() {
        let screen = screen(&["cargo build --release"]);
        let selection = Selection::press(&screen, spot(0, 8), 2);
        assert_eq!(selection.text(&screen), "build");
    }

    #[test]
    fn a_double_click_selects_a_word_across_soft_wraps_from_either_part() {
        let mut screen = screen(&["see https://", "host/abc def", "next"]);
        screen.wrapped = vec![true, false, false];
        for at in [spot(0, 6), spot(1, 2)] {
            let selection = Selection::press(&screen, at, 2);
            assert_eq!(selection.text(&screen), "https://host/abc", "{at:?}");
        }
        // A hard line break ends the word.
        screen.wrapped = vec![false, false, false];
        let selection = Selection::press(&screen, spot(1, 2), 2);
        assert_eq!(selection.text(&screen), "host/abc");
    }

    #[test]
    fn a_triple_click_selects_the_line_without_trailing_blanks() {
        let screen = screen(&["short", "a longer line"]);
        let selection = Selection::press(&screen, spot(0, 2), 3);
        assert_eq!(selection.text(&screen), "short");
    }

    #[test]
    fn word_drags_keep_the_original_word_and_extend_by_words() {
        let screen = screen(&["cargo build --release"]);
        let mut selection = Selection::press(&screen, spot(0, 8), 2);
        for (column, expected) in [
            (8, "build"), // Pointer jitter inside the pressed cell.
            (6, "build"), // Moving inside the original word.
            (17, "build --release"),
            (2, "cargo build"), // Crossing back past the original word.
            (8, "build"),
        ] {
            selection.extend(&screen, spot(0, column));
            assert_eq!(selection.text(&screen), expected);
        }
    }

    #[test]
    fn line_drags_keep_the_original_line_and_extend_by_lines() {
        let screen = screen(&["first", "hello world", "last"]);
        let mut selection = Selection::press(&screen, spot(1, 2), 3);
        for (at, expected) in [
            (spot(1, 2), "hello world"),
            (spot(1, 0), "hello world"),
            (spot(2, 1), "hello world\nlast"),
            (spot(0, 3), "first\nhello world"),
            (spot(1, 2), "hello world"),
        ] {
            selection.extend(&screen, at);
            assert_eq!(selection.text(&screen), expected);
        }
    }

    #[test]
    fn copying_distinguishes_soft_wraps_from_hard_line_breaks() {
        let mut screen = screen(&["https://", "host/abc", "def", "next"]);
        screen.wrapped = vec![true, true, false, false];
        for (from, to) in [(spot(0, 0), spot(3, 3)), (spot(3, 3), spot(0, 0))] {
            let mut selection = Selection::press(&screen, from, 1);
            selection.extend(&screen, to);
            assert_eq!(selection.text(&screen), "https://host/abcdef\nnext");
        }
        let mut selection = Selection::press(&screen, spot(0, 4), 1);
        selection.extend(&screen, spot(1, 2));
        assert_eq!(selection.text(&screen), "s://hos");
    }

    #[test]
    fn copying_preserves_spaces_at_soft_wraps_and_empty_hard_lines() {
        let mut screen = screen(&["echo  ", "hello ", "", "next"]);
        screen.wrapped = vec![true, false, false, false];
        let mut selection = Selection::press(&screen, spot(0, 0), 1);
        selection.extend(&screen, spot(3, 3));
        assert_eq!(selection.text(&screen), "echo  hello\n\nnext");
    }

    #[test]
    fn wide_characters_are_copied_and_highlighted_whole_in_both_directions() {
        // Explicit spacer cells, as the engines publish them.
        let mut screen = screen(&["界 x", "a界 "]);
        screen.cells[1].spacer = true;
        screen.cells[5].spacer = true;
        for (from, to, expected, ranges) in [
            (spot(0, 1), spot(0, 2), "界x", vec![(0, 0..3)]),
            (spot(0, 2), spot(0, 1), "界x", vec![(0, 0..3)]),
            (spot(1, 0), spot(1, 1), "a界", vec![(1, 0..3)]),
            (spot(1, 1), spot(1, 0), "a界", vec![(1, 0..3)]),
            (spot(0, 0), spot(0, 1), "界", vec![(0, 0..2)]),
            (spot(0, 1), spot(0, 0), "界", vec![(0, 0..2)]),
            (
                spot(0, 1),
                spot(1, 1),
                "界x\na界",
                vec![(0, 0..3), (1, 0..3)],
            ),
            (
                spot(1, 1),
                spot(0, 1),
                "界x\na界",
                vec![(0, 0..3), (1, 0..3)],
            ),
        ] {
            let mut selection = Selection::press(&screen, from, 1);
            selection.extend(&screen, to);
            assert_eq!(selection.text(&screen), expected, "{from:?} -> {to:?}");
            assert_eq!(selection.rows(&screen), ranges, "{from:?} -> {to:?}");
        }
        // An ordinary click on either half still selects nothing.
        for column in [0, 1] {
            let selection = Selection::press(&screen, spot(0, column), 1);
            assert!(selection.rows(&screen).is_empty());
            assert_eq!(selection.text(&screen), "");
        }
    }

    #[test]
    fn a_wide_character_wrapping_to_the_next_row_adds_no_padding_to_text() {
        let mut screen = screen(&["ab ", "界 x"]);
        screen.wrapped = vec![true, false];
        screen.cells[2].wrap_spacer = true;
        screen.cells[4].spacer = true;
        let mut selection = Selection::press(&screen, spot(0, 0), 1);
        selection.extend(&screen, spot(1, 2));
        assert_eq!(selection.text(&screen), "ab界x");
    }
}
