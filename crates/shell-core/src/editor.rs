//! A single-line editor over a fixed buffer.
//!
//! Input is printable ASCII only. That matches the US-layout keyboard the
//! kernel decodes and the console font Bedrock ships, and it keeps the buffer a
//! plain byte array that is always valid UTF-8.

/// An editable line of at most `N` bytes with a cursor.
#[derive(Debug)]
pub struct LineEditor<const N: usize> {
    buf: [u8; N],
    len: usize,
    cursor: usize,
}

impl<const N: usize> LineEditor<N> {
    /// An empty line.
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
            cursor: 0,
        }
    }

    /// The line so far.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    /// Characters in the line.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the line is empty.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The cursor position, from zero to the length.
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Empty the line.
    pub const fn clear(&mut self) {
        self.len = 0;
        self.cursor = 0;
    }

    /// Insert `ch` at the cursor. Refuses, returning `false`, if `ch` is not
    /// printable ASCII or the line already holds `limit` characters (or `N`,
    /// whichever is smaller).
    pub fn insert(&mut self, ch: char, limit: usize) -> bool {
        if !(ch.is_ascii_graphic() || ch == ' ') || self.len >= N.min(limit) {
            return false;
        }
        self.buf.copy_within(self.cursor..self.len, self.cursor + 1);
        self.buf[self.cursor] = ch as u8;
        self.len += 1;
        self.cursor += 1;
        true
    }

    /// Remove the character before the cursor.
    pub fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.buf.copy_within(self.cursor..self.len, self.cursor - 1);
        self.len -= 1;
        self.cursor -= 1;
        true
    }

    /// Remove the character under the cursor.
    pub fn delete(&mut self) -> bool {
        if self.cursor >= self.len {
            return false;
        }
        self.buf.copy_within(self.cursor + 1..self.len, self.cursor);
        self.len -= 1;
        true
    }

    /// Move the cursor left one place.
    pub const fn left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    /// Move the cursor right one place.
    pub const fn right(&mut self) -> bool {
        if self.cursor >= self.len {
            return false;
        }
        self.cursor += 1;
        true
    }

    /// Move the cursor to the start.
    pub const fn home(&mut self) -> bool {
        let moved = self.cursor != 0;
        self.cursor = 0;
        moved
    }

    /// Move the cursor to the end.
    pub const fn end(&mut self) -> bool {
        let moved = self.cursor != self.len;
        self.cursor = self.len;
        moved
    }
}

impl<const N: usize> Default for LineEditor<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed<const N: usize>(text: &str) -> LineEditor<N> {
        let mut editor = LineEditor::new();
        for ch in text.chars() {
            editor.insert(ch, N);
        }
        editor
    }

    #[test]
    fn insert_in_the_middle_shifts_the_tail() {
        let mut editor = typed::<16>("hllo");
        editor.home();
        editor.right();
        assert!(editor.insert('e', 16));
        assert_eq!(editor.as_str(), "hello");
        assert_eq!(editor.cursor(), 2);
    }

    #[test]
    fn backspace_and_delete_remove_the_right_character() {
        let mut editor = typed::<16>("abcd");
        editor.left();
        assert!(editor.backspace());
        assert_eq!(editor.as_str(), "abd");
        assert!(editor.delete());
        assert_eq!(editor.as_str(), "ab");
    }

    #[test]
    fn edges_report_no_change() {
        let mut editor = typed::<16>("ab");
        assert!(!editor.delete());
        assert!(!editor.right());
        assert!(editor.home());
        assert!(!editor.backspace());
        assert!(!editor.left());
        assert!(!editor.home());
        assert!(editor.end());
        assert!(!editor.end());
    }

    #[test]
    fn the_limit_and_the_capacity_both_stop_insertion() {
        let mut editor = LineEditor::<4>::new();
        assert!(editor.insert('a', 2));
        assert!(editor.insert('b', 2));
        assert!(!editor.insert('c', 2));
        let mut full = typed::<3>("abcdef");
        assert_eq!(full.as_str(), "abc");
        assert!(!full.insert('z', usize::MAX));
    }

    #[test]
    fn only_printable_ascii_is_accepted() {
        let mut editor = LineEditor::<8>::new();
        assert!(!editor.insert('\u{e9}', 8));
        assert!(!editor.insert('\n', 8));
        assert!(!editor.insert('\u{7f}', 8));
        assert!(editor.insert(' ', 8));
        assert!(editor.insert('~', 8));
        assert_eq!(editor.as_str(), " ~");
    }
}
