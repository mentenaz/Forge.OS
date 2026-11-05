//! The seam between the shell and the machine it runs on.
//!
//! The shell never touches a screen, a keyboard or a clock directly. It is
//! generic over [`Platform`], and each backend (a host terminal today, a
//! Bedrock ring-3 program later) supplies the three capabilities below.
//! Everything here is `no_std` and allocation-free so the same shell core can
//! be linked into a ring-3 program.
//!
//! Input is polled rather than blocking: Bedrock's native ABI is a
//! submission/completion ring, and a blocking `read_line` would hide that.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// A grid size in character cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    /// Columns.
    pub cols: u16,
    /// Rows.
    pub rows: u16,
}

impl Size {
    /// A grid of `cols` by `rows` cells.
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    /// The number of cells in the grid.
    pub const fn area(self) -> usize {
        self.cols as usize * self.rows as usize
    }
}

/// How a cell is drawn. Colour is deliberately absent until a backend needs it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    /// Heavier weight or brighter, at the backend's discretion.
    pub bold: bool,
    /// Foreground and background swapped; the shell draws its cursor this way.
    pub inverse: bool,
}

impl Style {
    /// No emphasis.
    pub const PLAIN: Style = Style {
        bold: false,
        inverse: false,
    };
}

/// One character position on the display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The character shown.
    pub ch: char,
    /// How it is drawn.
    pub style: Style,
}

impl Cell {
    /// An empty, unstyled cell.
    pub const BLANK: Cell = Cell {
        ch: ' ',
        style: Style::PLAIN,
    };

    /// A cell showing `ch` in `style`.
    pub const fn new(ch: char, style: Style) -> Self {
        Self { ch, style }
    }

    /// A cell showing `ch` with no emphasis.
    pub const fn plain(ch: char) -> Self {
        Self::new(ch, Style::PLAIN)
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::BLANK
    }
}

/// A grid of character cells the shell can draw on.
pub trait Display {
    /// The current grid size.
    fn size(&self) -> Size;

    /// Write one cell. Coordinates outside [`Display::size`] are ignored.
    fn put(&mut self, col: u16, row: u16, cell: Cell);

    /// Make everything written since the last call visible.
    fn present(&mut self);

    /// Blank every cell. Backends with a cheaper way should override this.
    fn clear(&mut self) {
        let Size { cols, rows } = self.size();
        for row in 0..rows {
            for col in 0..cols {
                self.put(col, row, Cell::BLANK);
            }
        }
    }
}

/// Which modifier keys were held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Control.
    pub ctrl: bool,
    /// Alt.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
}

/// A key, already decoded by the platform. The shell never sees scancodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A key that produces a character.
    Char(char),
    /// Return or Enter.
    Enter,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// Cursor left.
    Left,
    /// Cursor right.
    Right,
    /// Cursor up.
    Up,
    /// Cursor down.
    Down,
    /// Home.
    Home,
    /// End.
    End,
}

/// Whether a key went down or came up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    /// The key was pressed.
    Down,
    /// The key was released.
    Up,
}

/// One key transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// Which key.
    pub key: Key,
    /// Pressed or released.
    pub state: KeyState,
    /// Modifiers held at the time.
    pub modifiers: Modifiers,
}

/// Something the platform wants the shell to know about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A key went down or came up.
    Key(KeyEvent),
    /// The display changed size.
    Resize(Size),
}

/// A source of [`Event`]s.
pub trait Input {
    /// The next pending event, or `None` if there is none right now.
    fn poll(&mut self) -> Option<Event>;

    /// Wait until an event may be ready. It can return early, so callers loop
    /// on [`Input::poll`] rather than trusting a single wake-up.
    fn wait(&mut self);
}

/// A monotonic clock.
pub trait Clock {
    /// Nanoseconds since the platform started.
    fn uptime_ns(&self) -> u64;
}

/// Everything the shell needs from the machine under it.
pub trait Platform: Display + Input + Clock {}

impl<T: Display + Input + Clock> Platform for T {}

#[cfg(test)]
mod tests {
    use super::*;

    struct Grid {
        cells: [Cell; 6],
        writes: usize,
    }

    impl Display for Grid {
        fn size(&self) -> Size {
            Size::new(3, 2)
        }

        fn put(&mut self, col: u16, row: u16, cell: Cell) {
            if col < 3 && row < 2 {
                self.cells[row as usize * 3 + col as usize] = cell;
                self.writes += 1;
            }
        }

        fn present(&mut self) {}
    }

    #[test]
    fn clear_blanks_every_cell() {
        let mut grid = Grid {
            cells: [Cell::plain('x'); 6],
            writes: 0,
        };
        grid.clear();
        assert!(grid.cells.iter().all(|cell| *cell == Cell::BLANK));
        assert_eq!(grid.writes, Size::new(3, 2).area());
    }

    #[test]
    fn area_does_not_overflow_u16() {
        assert_eq!(Size::new(u16::MAX, u16::MAX).area(), 65535 * 65535);
    }
}
