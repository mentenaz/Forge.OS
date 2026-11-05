//! The shell, independent of the machine it runs on.
//!
//! `Shell<C, R, N>` owns a `C` by `R` character grid and an `N`-byte line
//! editor, so a backend chooses the capacity and can place the whole shell in
//! a static. There is no heap and no `unsafe`.
//!
//! The shell is driven with [`Shell::feed`] and drawn with [`Shell::render`].
//! [`Shell::run`] is a thin loop over those two for any [`Platform`].

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod commands;
mod editor;
mod terminal;

use core::fmt::Write;

pub use commands::{COMMANDS, Flow};
pub use editor::LineEditor;
use platform::{Display, Event, Key, KeyEvent, KeyState, Modifiers, Platform, Size};
pub use terminal::Terminal;

const PROMPT: &str = "> ";
const BANNER: &str = "shell ready, try 'help'";

/// An interactive shell on a `C` by `R` grid with an `N`-byte input line.
///
/// The input line never wraps: it stops accepting characters one column short
/// of the right edge. The grid needs at least four columns to show a prompt.
#[derive(Debug)]
pub struct Shell<const C: usize, const R: usize, const N: usize> {
    term: Terminal<C, R>,
    editor: LineEditor<N>,
    input_row: usize,
}

impl<const C: usize, const R: usize, const N: usize> Shell<C, R, N> {
    /// A shell that has not started. Call [`Shell::begin`] before use.
    pub const fn new() -> Self {
        Self {
            term: Terminal::new(),
            editor: LineEditor::new(),
            input_row: 0,
        }
    }

    /// Size the grid to the display, then show the banner and the first prompt.
    pub fn begin(&mut self, size: Size) {
        self.term.resize(size);
        self.term.clear();
        let _ = writeln!(self.term, "{BANNER}");
        self.start_prompt();
    }

    /// The line being edited.
    pub fn line(&self) -> &str {
        self.editor.as_str()
    }

    /// The shell's grid, for inspection.
    pub const fn terminal(&self) -> &Terminal<C, R> {
        &self.term
    }

    /// Handle one event. `now_ns` is the platform's uptime, for `uptime`.
    pub fn feed(&mut self, event: Event, now_ns: u64) -> Flow {
        match event {
            Event::Resize(size) => {
                self.term.resize(size);
                self.input_row = self.input_row.min(self.term.size().rows as usize - 1);
                self.redraw_input();
                Flow::Continue
            }
            Event::Key(KeyEvent {
                key,
                state: KeyState::Down,
                modifiers,
            }) => self.key(key, modifiers, now_ns),
            Event::Key(_) => Flow::Continue,
        }
    }

    /// Draw whatever changed. Returns whether anything was drawn.
    pub fn render<D: Display + ?Sized>(&mut self, display: &mut D) -> bool {
        self.term.render(display)
    }

    /// Run until the user exits: draw, take one event, repeat, waiting when
    /// there is none.
    pub fn run<P: Platform>(&mut self, platform: &mut P) {
        self.begin(platform.size());
        loop {
            self.render(platform);
            match platform.poll() {
                Some(event) => {
                    if self.feed(event, platform.uptime_ns()) == Flow::Exit {
                        self.render(platform);
                        return;
                    }
                }
                None => platform.wait(),
            }
        }
    }

    fn key(&mut self, key: Key, modifiers: Modifiers, now_ns: u64) -> Flow {
        if modifiers.alt {
            return Flow::Continue;
        }
        if modifiers.ctrl {
            match key {
                Key::Char('l' | 'L') => {
                    self.term.clear();
                    self.input_row = 0;
                    self.redraw_input();
                }
                Key::Char('c' | 'C') => self.start_prompt(),
                _ => {}
            }
            return Flow::Continue;
        }
        let changed = match key {
            Key::Char(ch) => {
                let limit = self.input_limit();
                self.editor.insert(ch, limit)
            }
            Key::Backspace => self.editor.backspace(),
            Key::Delete => self.editor.delete(),
            Key::Left => self.editor.left(),
            Key::Right => self.editor.right(),
            Key::Home => self.editor.home(),
            Key::End => self.editor.end(),
            Key::Enter => return self.enter(now_ns),
            Key::Tab | Key::Escape | Key::Up | Key::Down => false,
        };
        if changed {
            self.redraw_input();
        }
        Flow::Continue
    }

    fn enter(&mut self, now_ns: u64) -> Flow {
        self.term.newline();
        let flow = commands::execute(self.editor.as_str(), &mut self.term, now_ns);
        if flow == Flow::Continue {
            self.start_prompt();
        }
        flow
    }

    fn start_prompt(&mut self) {
        if self.term.cursor().0 != 0 {
            self.term.newline();
        }
        self.editor.clear();
        self.input_row = self.term.cursor().1;
        self.redraw_input();
    }

    fn input_limit(&self) -> usize {
        (self.term.size().cols as usize).saturating_sub(PROMPT.len() + 1)
    }

    fn redraw_input(&mut self) {
        if (self.term.size().cols as usize) < PROMPT.len() + 2 {
            return;
        }
        self.term.set_cursor(0, self.input_row);
        let _ = self.term.write_str(PROMPT);
        let _ = self.term.write_str(self.editor.as_str());
        self.term.erase_to_end_of_line();
        self.term
            .set_cursor(PROMPT.len() + self.editor.cursor(), self.input_row);
    }
}

impl<const C: usize, const R: usize, const N: usize> Default for Shell<C, R, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::string::String;
    use std::vec::Vec;

    use platform::{Cell, Clock, Input};

    use super::*;

    type Small = Shell<40, 8, 32>;

    const SECOND: u64 = 1_000_000_000;

    struct Screen<const C: usize, const R: usize> {
        cells: [[Cell; C]; R],
        puts: usize,
        presents: usize,
    }

    impl<const C: usize, const R: usize> Screen<C, R> {
        fn new() -> Self {
            Self {
                cells: [[Cell::BLANK; C]; R],
                puts: 0,
                presents: 0,
            }
        }

        fn text(&self, row: usize) -> String {
            self.cells[row]
                .iter()
                .map(|cell| cell.ch)
                .collect::<String>()
                .trim_end()
                .to_string()
        }
    }

    impl<const C: usize, const R: usize> Display for Screen<C, R> {
        fn size(&self) -> Size {
            Size::new(C as u16, R as u16)
        }

        fn put(&mut self, col: u16, row: u16, cell: Cell) {
            if (col as usize) < C && (row as usize) < R {
                self.cells[row as usize][col as usize] = cell;
                self.puts += 1;
            }
        }

        fn present(&mut self) {
            self.presents += 1;
        }
    }

    struct Fake {
        screen: Screen<40, 8>,
        events: VecDeque<Event>,
        arriving: VecDeque<Event>,
        polls: usize,
        waits: usize,
        now: u64,
    }

    impl Fake {
        fn new(now: u64) -> Self {
            Self {
                screen: Screen::new(),
                events: VecDeque::new(),
                arriving: VecDeque::new(),
                polls: 0,
                waits: 0,
                now,
            }
        }
    }

    fn script(lines: &[&str]) -> VecDeque<Event> {
        let mut events = VecDeque::new();
        for line in lines {
            events.extend(line.chars().map(|ch| press(Key::Char(ch))));
            events.push_back(press(Key::Enter));
        }
        events
    }

    impl Display for Fake {
        fn size(&self) -> Size {
            self.screen.size()
        }

        fn put(&mut self, col: u16, row: u16, cell: Cell) {
            self.screen.put(col, row, cell);
        }

        fn present(&mut self) {
            self.screen.present();
        }
    }

    impl Input for Fake {
        fn poll(&mut self) -> Option<Event> {
            self.polls += 1;
            assert!(
                self.polls < 1000,
                "the shell is spinning instead of waiting"
            );
            self.events.pop_front()
        }

        fn wait(&mut self) {
            self.waits += 1;
            let event = self
                .arriving
                .pop_front()
                .expect("the shell waited with nothing left to arrive");
            self.events.push_back(event);
        }
    }

    impl Clock for Fake {
        fn uptime_ns(&self) -> u64 {
            self.now
        }
    }

    fn event(key: Key, modifiers: Modifiers, state: KeyState) -> Event {
        Event::Key(KeyEvent {
            key,
            state,
            modifiers,
        })
    }

    fn press(key: Key) -> Event {
        event(key, Modifiers::default(), KeyState::Down)
    }

    fn ctrl(ch: char) -> Event {
        let modifiers = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        event(Key::Char(ch), modifiers, KeyState::Down)
    }

    fn type_str<const C: usize, const R: usize, const N: usize>(
        shell: &mut Shell<C, R, N>,
        text: &str,
    ) {
        for ch in text.chars() {
            shell.feed(press(Key::Char(ch)), 0);
        }
    }

    fn submit_at<const C: usize, const R: usize, const N: usize>(
        shell: &mut Shell<C, R, N>,
        text: &str,
        now_ns: u64,
    ) -> Flow {
        type_str(shell, text);
        shell.feed(press(Key::Enter), now_ns)
    }

    fn submit(shell: &mut Small, text: &str) -> Flow {
        submit_at(shell, text, 0)
    }

    fn started() -> Small {
        let mut shell = Small::new();
        shell.begin(Size::new(40, 8));
        shell
    }

    fn rows<const C: usize, const R: usize, const N: usize>(shell: &Shell<C, R, N>) -> Vec<String> {
        (0..R)
            .map(|row| {
                (0..C)
                    .filter_map(|col| shell.terminal().char_at(col, row))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn begins_with_the_banner_and_a_prompt() {
        let shell = started();
        let rows = rows(&shell);
        assert_eq!(rows[0], BANNER);
        assert_eq!(rows[1], ">");
        assert_eq!(shell.terminal().cursor(), (2, 1));
    }

    #[test]
    fn echo_prints_its_arguments_joined_by_single_spaces() {
        let mut shell = started();
        submit(&mut shell, "echo hello   world");
        let rows = rows(&shell);
        assert_eq!(rows[1], "> echo hello   world");
        assert_eq!(rows[2], "hello world");
        assert_eq!(rows[3], ">");
    }

    #[test]
    fn an_unknown_command_is_reported() {
        let mut shell = started();
        submit(&mut shell, "frobnicate now");
        assert_eq!(rows(&shell)[2], "frobnicate: command not found");
    }

    #[test]
    fn a_blank_line_just_shows_a_new_prompt() {
        let mut shell = started();
        submit(&mut shell, "   ");
        let rows = rows(&shell);
        assert_eq!(rows[2], ">");
    }

    #[test]
    fn help_lists_every_command() {
        let mut shell = started();
        submit(&mut shell, "help");
        let rows = rows(&shell);
        for (name, about) in COMMANDS {
            assert!(
                rows.iter()
                    .any(|row| row.starts_with(name) && row.ends_with(about)),
                "help does not list {name}"
            );
        }
    }

    #[test]
    fn clear_leaves_only_a_prompt_on_the_first_row() {
        let mut shell = started();
        submit(&mut shell, "echo x");
        submit(&mut shell, "clear");
        let rows = rows(&shell);
        assert_eq!(rows[0], ">");
        assert!(rows[1..].iter().all(String::is_empty));
        assert_eq!(shell.terminal().cursor(), (2, 0));
    }

    #[test]
    fn uptime_is_split_into_hours_minutes_and_seconds() {
        let mut shell = started();
        submit_at(&mut shell, "uptime", 3725 * SECOND);
        assert_eq!(rows(&shell)[2], "up 1h 2m 5s");
    }

    #[test]
    fn exit_ends_the_shell() {
        let mut shell = started();
        assert_eq!(submit(&mut shell, "exit"), Flow::Exit);
    }

    #[test]
    fn backspace_fixes_a_typo() {
        let mut shell = started();
        type_str(&mut shell, "ecx");
        shell.feed(press(Key::Backspace), 0);
        submit(&mut shell, "ho hi");
        assert_eq!(rows(&shell)[2], "hi");
    }

    #[test]
    fn home_right_and_delete_edit_in_place() {
        let mut shell = started();
        type_str(&mut shell, "echoo hi");
        shell.feed(press(Key::Home), 0);
        for _ in 0..4 {
            shell.feed(press(Key::Right), 0);
        }
        shell.feed(press(Key::Delete), 0);
        assert_eq!(shell.line(), "echo hi");
        shell.feed(press(Key::Enter), 0);
        assert_eq!(rows(&shell)[2], "hi");
    }

    #[test]
    fn typing_stops_one_column_short_of_the_edge() {
        let mut shell = Shell::<12, 4, 64>::new();
        shell.begin(Size::new(12, 4));
        type_str(&mut shell, &"a".repeat(30));
        assert_eq!(shell.line().len(), 9);
    }

    #[test]
    fn non_ascii_key_releases_and_modified_keys_do_not_type() {
        let mut shell = started();
        type_str(&mut shell, "\u{e9}");
        shell.feed(event(Key::Char('a'), Modifiers::default(), KeyState::Up), 0);
        shell.feed(ctrl('a'), 0);
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        shell.feed(event(Key::Char('a'), alt, KeyState::Down), 0);
        assert_eq!(shell.line(), "");
    }

    #[test]
    fn up_down_tab_and_escape_are_ignored() {
        let mut shell = started();
        type_str(&mut shell, "ab");
        for key in [Key::Up, Key::Down, Key::Tab, Key::Escape] {
            shell.feed(press(key), 0);
        }
        assert_eq!(shell.line(), "ab");
    }

    #[test]
    fn ctrl_c_abandons_the_line() {
        let mut shell = started();
        type_str(&mut shell, "abc");
        shell.feed(ctrl('c'), 0);
        assert_eq!(shell.line(), "");
        let rows = rows(&shell);
        assert_eq!(rows[1], "> abc");
        assert_eq!(rows[2], ">");
    }

    #[test]
    fn ctrl_l_clears_the_screen_but_keeps_the_line() {
        let mut shell = started();
        submit(&mut shell, "echo x");
        type_str(&mut shell, "ab");
        shell.feed(ctrl('l'), 0);
        let rows = rows(&shell);
        assert_eq!(rows[0], "> ab");
        assert!(rows[1..].iter().all(String::is_empty));
        assert_eq!(shell.line(), "ab");
    }

    #[test]
    fn output_scrolls_when_the_grid_is_full() {
        let mut shell = Shell::<40, 4, 32>::new();
        shell.begin(Size::new(40, 4));
        for _ in 0..5 {
            submit_at(&mut shell, "echo a", 0);
        }
        let rows = rows(&shell);
        assert_eq!(rows[1], "> echo a");
        assert_eq!(rows[2], "a");
        assert_eq!(rows[3], ">");
    }

    #[test]
    fn resizing_never_panics_even_to_nothing() {
        let mut shell = started();
        shell.feed(Event::Resize(Size::new(10, 3)), 0);
        assert_eq!(shell.terminal().size(), Size::new(10, 3));
        shell.feed(Event::Resize(Size::new(0, 0)), 0);
        type_str(&mut shell, "abc");
        shell.feed(press(Key::Enter), 0);
        shell.feed(Event::Resize(Size::new(40, 8)), 0);
        type_str(&mut shell, "echo ok");
        shell.feed(press(Key::Enter), 0);
        assert!(rows(&shell).iter().any(|row| row == "ok"));
    }

    #[test]
    fn render_draws_the_cursor_inverted_and_only_changed_rows() {
        let mut shell = started();
        let mut screen = Screen::<40, 8>::new();
        assert!(shell.render(&mut screen));
        assert_eq!(screen.text(0), BANNER);
        assert!(screen.cells[1][2].style.inverse);
        assert!(!screen.cells[1][3].style.inverse);
        assert_eq!(screen.presents, 1);

        let puts = screen.puts;
        assert!(!shell.render(&mut screen));
        assert_eq!((screen.puts, screen.presents), (puts, 1));

        type_str(&mut shell, "x");
        assert!(shell.render(&mut screen));
        assert_eq!(screen.puts, puts + 40);
        assert_eq!(screen.text(1), "> x");
    }

    #[test]
    fn run_drives_the_shell_until_exit() {
        let mut fake = Fake::new(5 * SECOND);
        fake.events = script(&["uptime", "exit"]);
        let mut shell = Small::new();
        shell.run(&mut fake);
        assert_eq!(fake.screen.text(2), "up 0h 0m 5s");
        assert_eq!(fake.screen.text(3), "> exit");
        assert!(fake.screen.cells[4][0].style.inverse);
    }

    #[test]
    fn run_waits_instead_of_spinning_when_idle() {
        let mut fake = Fake::new(0);
        fake.arriving = script(&["echo hi", "exit"]);
        let expected_waits = fake.arriving.len();
        let mut shell = Small::new();
        shell.run(&mut fake);
        assert_eq!(fake.waits, expected_waits);
        assert_eq!(fake.screen.text(2), "hi");
    }

    #[test]
    fn home_and_end_move_the_cursor_on_screen() {
        let mut shell = started();
        type_str(&mut shell, "ab");
        shell.feed(press(Key::Home), 0);
        assert_eq!(shell.terminal().cursor(), (2, 1));
        shell.feed(press(Key::End), 0);
        assert_eq!(shell.terminal().cursor(), (4, 1));
    }

    #[test]
    fn a_resize_keeps_one_copy_of_the_line_being_typed() {
        let mut shell = started();
        submit(&mut shell, "echo a");
        submit(&mut shell, "echo a");
        type_str(&mut shell, "x");
        shell.feed(Event::Resize(Size::new(40, 3)), 0);
        shell.feed(Event::Resize(Size::new(40, 8)), 0);
        let copies = rows(&shell).iter().filter(|row| *row == "> x").count();
        assert_eq!(copies, 1);
    }
}
