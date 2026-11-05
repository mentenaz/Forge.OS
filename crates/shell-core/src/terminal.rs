//! A fixed-size character grid with a cursor, scrolling and dirty-row tracking.
//!
//! [`Display`] is write-only, so the shell cannot read cells back in order to
//! scroll them. This grid is the shell's own copy of the screen: it scrolls
//! here and redraws only the rows that changed.

use core::fmt;

use platform::{Cell, Display, Size, Style};

/// A character grid with `C` columns and `R` rows of capacity.
///
/// The active size can be smaller after a [`Terminal::resize`]; whatever no
/// longer fits is discarded, never reflowed.
#[derive(Debug)]
pub struct Terminal<const C: usize, const R: usize> {
    cells: [[char; C]; R],
    size: Size,
    col: usize,
    row: usize,
    dirty: [bool; R],
}

impl<const C: usize, const R: usize> Terminal<C, R> {
    /// A blank grid at full capacity with the cursor at the top left.
    pub const fn new() -> Self {
        const {
            assert!(C > 0 && R > 0 && C <= u16::MAX as usize && R <= u16::MAX as usize);
        }
        Self {
            cells: [[' '; C]; R],
            size: Size::new(C as u16, R as u16),
            col: 0,
            row: 0,
            dirty: [true; R],
        }
    }

    /// The active size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// The cursor as `(column, row)`. The column can equal the width.
    pub const fn cursor(&self) -> (usize, usize) {
        (self.col, self.row)
    }

    /// The character at a position, or `None` outside the active grid.
    pub fn char_at(&self, col: usize, row: usize) -> Option<char> {
        if col < self.cols() && row < self.rows() {
            Some(self.cells[row][col])
        } else {
            None
        }
    }

    fn cols(&self) -> usize {
        self.size.cols as usize
    }

    fn rows(&self) -> usize {
        self.size.rows as usize
    }

    fn mark(&mut self, row: usize) {
        if let Some(flag) = self.dirty.get_mut(row) {
            *flag = true;
        }
    }

    /// Change the active size, clamped to between 1 and the capacity.
    /// Anything outside the new size is discarded, not reflowed.
    pub fn resize(&mut self, size: Size) {
        let cols = (size.cols as usize).clamp(1, C);
        let rows = (size.rows as usize).clamp(1, R);
        for (row, line) in self.cells.iter_mut().enumerate() {
            if row >= rows {
                *line = [' '; C];
            } else {
                line[cols..].fill(' ');
            }
        }
        self.size = Size::new(cols as u16, rows as u16);
        self.col = self.col.min(cols);
        self.row = self.row.min(rows - 1);
        self.dirty = [true; R];
    }

    /// Move the cursor, clamped to the active grid.
    pub fn set_cursor(&mut self, col: usize, row: usize) {
        self.mark(self.row);
        self.row = row.min(self.rows() - 1);
        self.col = col.min(self.cols());
        self.mark(self.row);
    }

    /// Write one character at the cursor. `\n` starts a new line, `\r` returns
    /// to column zero and other control characters are ignored. Writing past
    /// the right edge wraps.
    pub fn put_char(&mut self, ch: char) {
        match ch {
            '\n' => self.newline(),
            '\r' => self.set_cursor(0, self.row),
            c if c.is_control() => {}
            c => {
                if self.col >= self.cols() {
                    self.newline();
                }
                self.cells[self.row][self.col] = c;
                self.col += 1;
                self.mark(self.row);
            }
        }
    }

    /// Move to the start of the next row, scrolling at the bottom.
    pub fn newline(&mut self) {
        self.mark(self.row);
        self.col = 0;
        if self.row + 1 < self.rows() {
            self.row += 1;
            self.mark(self.row);
        } else {
            self.scroll_up();
        }
    }

    fn scroll_up(&mut self) {
        let rows = self.rows();
        self.cells[..rows].rotate_left(1);
        self.cells[rows - 1] = [' '; C];
        self.dirty = [true; R];
    }

    /// Blank from the cursor to the end of its row.
    pub fn erase_to_end_of_line(&mut self) {
        let from = self.col.min(C);
        for cell in &mut self.cells[self.row][from..] {
            *cell = ' ';
        }
        self.mark(self.row);
    }

    /// Blank the whole grid and home the cursor.
    pub fn clear(&mut self) {
        self.cells = [[' '; C]; R];
        self.col = 0;
        self.row = 0;
        self.dirty = [true; R];
    }

    /// Draw every changed row, with the cursor cell inverted, then present.
    /// Returns whether anything was drawn.
    pub fn render<D: Display + ?Sized>(&mut self, display: &mut D) -> bool {
        let mut drawn = false;
        for row in 0..self.rows() {
            if !self.dirty[row] {
                continue;
            }
            for col in 0..self.cols() {
                let style = Style {
                    bold: false,
                    inverse: col == self.col && row == self.row,
                };
                display.put(
                    col as u16,
                    row as u16,
                    Cell::new(self.cells[row][col], style),
                );
            }
            self.dirty[row] = false;
            drawn = true;
        }
        if drawn {
            display.present();
        }
        drawn
    }
}

impl<const C: usize, const R: usize> Default for Terminal<C, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const C: usize, const R: usize> fmt::Write for Terminal<C, R> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for ch in text.chars() {
            self.put_char(ch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::string::String;

    use super::*;

    fn row_text<const C: usize, const R: usize>(term: &Terminal<C, R>, row: usize) -> String {
        (0..C).filter_map(|col| term.char_at(col, row)).collect()
    }

    #[test]
    fn writing_past_the_right_edge_wraps() {
        let mut term = Terminal::<4, 3>::new();
        fmt::Write::write_str(&mut term, "abcdef").unwrap();
        assert_eq!(row_text(&term, 0), "abcd");
        assert_eq!(row_text(&term, 1), "ef  ");
    }

    #[test]
    fn newline_at_the_bottom_scrolls() {
        let mut term = Terminal::<4, 2>::new();
        fmt::Write::write_str(&mut term, "a\nb\nc").unwrap();
        assert_eq!(row_text(&term, 0), "b   ");
        assert_eq!(row_text(&term, 1), "c   ");
    }

    #[test]
    fn erase_blanks_from_the_cursor() {
        let mut term = Terminal::<4, 1>::new();
        fmt::Write::write_str(&mut term, "abcd").unwrap();
        term.set_cursor(2, 0);
        term.erase_to_end_of_line();
        assert_eq!(row_text(&term, 0), "ab  ");
    }

    #[test]
    fn control_characters_are_ignored() {
        let mut term = Terminal::<4, 1>::new();
        fmt::Write::write_str(&mut term, "a\u{7}b").unwrap();
        assert_eq!(row_text(&term, 0), "ab  ");
    }

    #[test]
    fn resize_clamps_to_capacity_and_to_one() {
        let mut term = Terminal::<4, 3>::new();
        term.resize(Size::new(10, 10));
        assert_eq!(term.size(), Size::new(4, 3));
        term.resize(Size::new(0, 0));
        assert_eq!(term.size(), Size::new(1, 1));
        assert!(term.cursor().0 <= 1 && term.cursor().1 == 0);
    }

    struct Grid {
        chars: [[char; 4]; 2],
        inverse: [[bool; 4]; 2],
        presents: usize,
    }

    impl Grid {
        fn new() -> Self {
            Self {
                chars: [['?'; 4]; 2],
                inverse: [[false; 4]; 2],
                presents: 0,
            }
        }
    }

    impl Display for Grid {
        fn size(&self) -> Size {
            Size::new(4, 2)
        }

        fn put(&mut self, col: u16, row: u16, cell: Cell) {
            self.chars[row as usize][col as usize] = cell.ch;
            self.inverse[row as usize][col as usize] = cell.style.inverse;
        }

        fn present(&mut self) {
            self.presents += 1;
        }
    }

    #[test]
    fn scrolling_blanks_the_new_bottom_row() {
        let mut term = Terminal::<4, 2>::new();
        fmt::Write::write_str(&mut term, "abcd\ne\n").unwrap();
        assert_eq!(row_text(&term, 0), "e   ");
        assert_eq!(row_text(&term, 1), "    ");
    }

    #[test]
    fn shrinking_discards_what_no_longer_fits() {
        let mut term = Terminal::<4, 3>::new();
        fmt::Write::write_str(&mut term, "abcd\nefgh\nijkl").unwrap();
        term.resize(Size::new(2, 2));
        term.resize(Size::new(4, 3));
        assert_eq!(row_text(&term, 0), "ab  ");
        assert_eq!(row_text(&term, 1), "ef  ");
        assert_eq!(row_text(&term, 2), "    ");
    }

    #[test]
    fn leaving_a_row_redraws_it_without_the_cursor() {
        let mut term = Terminal::<4, 2>::new();
        let mut grid = Grid::new();
        term.render(&mut grid);
        assert!(grid.inverse[0][0]);
        term.newline();
        term.render(&mut grid);
        assert!(!grid.inverse[0][0]);
        assert!(grid.inverse[1][0]);
    }

    #[test]
    fn resizing_redraws_everything() {
        let mut term = Terminal::<4, 2>::new();
        let mut grid = Grid::new();
        term.render(&mut grid);
        term.resize(Size::new(3, 2));
        assert!(term.render(&mut grid));
    }

    #[test]
    fn render_redraws_only_rows_changed_since_the_last_render() {
        let mut term = Terminal::<4, 2>::new();
        let mut grid = Grid::new();
        assert!(term.render(&mut grid));
        assert_eq!(grid.chars, [[' '; 4]; 2]);
        assert!(!term.render(&mut grid));
        assert_eq!(grid.presents, 1);

        grid.chars[1] = ['?'; 4];
        term.put_char('x');
        assert!(term.render(&mut grid));
        assert_eq!(grid.chars[0], ['x', ' ', ' ', ' ']);
        assert_eq!(grid.chars[1], ['?'; 4]);
        assert_eq!(grid.presents, 2);
    }

    #[test]
    fn clear_blanks_and_homes() {
        let mut term = Terminal::<4, 2>::new();
        fmt::Write::write_str(&mut term, "ab\ncd").unwrap();
        term.clear();
        assert_eq!(term.cursor(), (0, 0));
        assert_eq!(row_text(&term, 0), "    ");
        assert_eq!(row_text(&term, 1), "    ");
    }
}
